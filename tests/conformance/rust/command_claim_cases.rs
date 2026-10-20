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
    // Same lexical exclusions as RelativePath::parse, plus this profile's
    // ToS/ prefix. Borrowed scans only: no RelativePath/String allocation.
    b.work(
        path.len()
            .checked_mul(4)
            .ok_or(ClaimReferenceError::Limit)?,
        16,
    )?;
    if !path.starts_with("ToS/")
        || path.bytes().any(|byte| byte < 32 || byte == b'\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".." | ".git"))
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
    for path in [
        "ToS/.git/x",
        "ToS/a/.git",
        "ToS/a\0b",
        "ToS/a\u{1f}b",
        "ToS/a\nb",
        "ToS/./x",
        "ToS/../x",
        "ToS//x",
        "ToS/a\\b",
        "/ToS/x",
        "ToS/",
    ] {
        b.work(0, 8)?;
        assert_eq!(
            claim_reference_path(path, b),
            Err(ClaimReferenceError::InvalidInput)
        );
    }
    // .git is a component exclusion, not a prefix ban on ordinary filenames.
    claim_reference_path("ToS/a.json", b)?;
    claim_reference_path("ToS/.gitignore", b)?;
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
    let number = |v: &JsonValue, b: &mut ClaimReferenceBudget<'_>| -> ClaimReferenceResult<usize> {
        match v {
            JsonValue::Number(v) => {
                b.work(v.lexeme.len(), 8)?;
                v.lexeme
                    .parse::<usize>()
                    .map_err(|_| ClaimReferenceError::InvalidInput)
            }
            _ => Err(ClaimReferenceError::InvalidInput),
        }
    };
    let rows = claim_reference_field(inventory, "files", b)?
        .and_then(JsonValue::as_array)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let count = number(
        claim_reference_field(inventory, "entry_count", b)?
            .ok_or(ClaimReferenceError::InvalidInput)?,
        b,
    )?;
    let total = number(
        claim_reference_field(inventory, "total_bytes", b)?
            .ok_or(ClaimReferenceError::InvalidInput)?,
        b,
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
            b,
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
            "uid" => number(original, b)? == 1000 && value.as_u64().is_some(),
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

// Rich comparisons qualify the historical INPUT domain, never a SUT answer.
const CLAIM_RICH_ROWS: [usize; 6] = [65_536, 458_752, 2_621_440, 3_145_728, 1_048_576, 1_048_576];
const RICH_ORIGINAL: usize = 1;
const RICH_ENTITY: usize = 2;
const RICH_RELATION: usize = 3;
const RICH_CONTEXT: usize = 4;
const RICH_RAW: usize = 5;
struct RichClaimRemaining {
    state: usize,
    visits: usize,
    bytes: usize,
    rows: [usize; 6],
    fixed: usize,
}
struct RichClaimBudget<'a> {
    base: ClaimReferenceBudget<'a>,
    rows: [usize; 6],
    fixed: usize,
}
impl<'a> RichClaimBudget<'a> {
    fn enter(
        deadline: Instant,
        cancelled: &'a AtomicBool,
        old: Option<RichClaimRemaining>,
    ) -> ClaimReferenceResult<Self> {
        let (base, rows, fixed) = if let Some(old) = old {
            (
                ClaimReferenceBudget {
                    state: old.state,
                    visits: old.visits,
                    byte_work: old.bytes,
                    deadline,
                    cancelled,
                },
                old.rows,
                old.fixed,
            )
        } else {
            let mut base = ClaimReferenceBudget::new(deadline, cancelled);
            base.work(0, 128)?;
            let sum = CLAIM_RICH_ROWS
                .iter()
                .try_fold(0usize, |s, v| s.checked_add(*v))
                .ok_or(ClaimReferenceError::Limit)?;
            let frame = std::mem::size_of::<(
                &JsonValue,
                &JsonValue,
                usize,
                usize,
                usize,
                usize,
                usize,
                usize,
                Option<&str>,
                ClaimReferenceResult<usize>,
                ClaimReferenceResult<()>,
            )>();
            let outside = frame
                .checked_mul(65)
                .and_then(|n| n.checked_add(std::mem::size_of::<RichBatch<'_>>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<[Option<&str>; 65]>()))
                .and_then(|n| n.checked_add(32 * std::mem::size_of::<usize>()))
                .and_then(|n| n.checked_add(16 * std::mem::size_of::<Digest256>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<RichClaimRemaining>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<Option<RichRegistryInputs>>()))
                .ok_or(ClaimReferenceError::Limit)?;
            if outside > 16_384 {
                return Err(ClaimReferenceError::Limit);
            }
            if sum != base.state
                || std::mem::size_of::<Self>()
                    .checked_add(std::mem::size_of::<RichClaimRemaining>())
                    .and_then(|v| v.checked_add(std::mem::size_of::<Option<RichRegistryInputs>>()))
                    .and_then(|v| v.checked_add(16_384))
                    .is_none_or(|v| v > CLAIM_RICH_ROWS[0])
            {
                return Err(ClaimReferenceError::Limit);
            }
            base.own(CLAIM_RICH_ROWS[0])?;
            let mut rows = CLAIM_RICH_ROWS;
            rows[0] = 0;
            let fixed = CLAIM_RICH_ROWS[0]
                .checked_sub(16_384)
                .and_then(|v| v.checked_sub(std::mem::size_of::<Self>()))
                .and_then(|v| v.checked_sub(std::mem::size_of::<RichClaimRemaining>()))
                .and_then(|v| v.checked_sub(std::mem::size_of::<Option<RichRegistryInputs>>()))
                .ok_or(ClaimReferenceError::Limit)?;
            (base, rows, fixed)
        };
        base.active()?;
        Ok(Self { base, rows, fixed })
    }
    fn leave(self) -> RichClaimRemaining {
        RichClaimRemaining {
            state: self.base.state,
            visits: self.base.visits,
            bytes: self.base.byte_work,
            rows: self.rows,
            fixed: self.fixed,
        }
    }
    fn own(&mut self, row: usize, bytes: usize) -> ClaimReferenceResult<()> {
        self.base.work(0, 16)?;
        let left = self
            .rows
            .get(row)
            .and_then(|n| n.checked_sub(bytes))
            .ok_or(ClaimReferenceError::Limit)?;
        if bytes > self.base.state {
            return Err(ClaimReferenceError::Limit);
        }
        self.base.own(bytes)?;
        self.rows[row] = left;
        Ok(())
    }
    fn workspace(&mut self, bytes: usize) -> ClaimReferenceResult<()> {
        self.base.work(0, 16)?;
        self.fixed = self
            .fixed
            .checked_sub(bytes)
            .ok_or(ClaimReferenceError::Limit)?;
        Ok(())
    }
    fn parse(
        &mut self,
        raw: &[u8],
        row: usize,
        reservation: usize,
    ) -> ClaimReferenceResult<JsonValue> {
        self.codec(Some(raw), None, row, reservation).map(|v| v.0)
    }
    fn canonical(
        &mut self,
        value: &JsonValue,
        row: usize,
        reservation: usize,
    ) -> ClaimReferenceResult<Vec<u8>> {
        self.codec(None, Some(value), row, reservation).map(|v| v.1)
    }
    fn codec(
        &mut self,
        raw: Option<&[u8]>,
        value: Option<&JsonValue>,
        row: usize,
        reservation: usize,
    ) -> ClaimReferenceResult<(JsonValue, Vec<u8>)> {
        use std::cell::Cell;
        use tos_foundation::{FoundationError, FoundationErrorCode};
        self.base.work(raw.map_or(0, <[u8]>::len), 64)?;
        let state = reservation
            .checked_sub(16_384)
            .ok_or(ClaimReferenceError::Limit)?;
        if state == 0
            || raw.is_some() == value.is_some()
            || raw.is_some_and(|v| v.len() > JsonLimits::default().max_bytes)
        {
            return Err(ClaimReferenceError::Limit);
        }
        self.own(row, reservation)?;
        let limits = JsonLimits {
            max_bytes: if raw.is_none() {
                state.min(JsonLimits::default().max_bytes)
            } else {
                JsonLimits::default().max_bytes
            },
            max_visits: self.base.visits,
            ..JsonLimits::default()
        };
        let first = Cell::new(None);
        let deadline = self.base.deadline;
        let cancelled = self.base.cancelled;
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
            self.base.work(bytes, visits).map_err(|error| {
                if first.get().is_none() {
                    first.set(Some(error));
                }
                FoundationError {
                    code: FoundationErrorCode::BudgetExceeded,
                    byte_offset: None,
                    detail: String::new(),
                }
            })
        };
        let result = match (raw, value) {
            (Some(raw), None) => tos_foundation::parse_json_with_state_budget_and_admission(
                raw,
                JsonMode::PublishedStrict,
                limits,
                state,
                &mut check,
                &mut admit,
            )
            .map(|d| (d.into_root(), Vec::new())),
            (None, Some(value)) => {
                tos_foundation::canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                    value,
                    CanonicalProfile::SourceCommandInputV1,
                    limits,
                    state,
                    &mut check,
                    &mut admit,
                )
                .map(|v| (JsonValue::Null, v.0))
            }
            _ => unreachable!(),
        };
        if let Some(error) = first.get() {
            return Err(error);
        }
        let result = result.map_err(|e| {
            if e.code == FoundationErrorCode::BudgetExceeded {
                ClaimReferenceError::Limit
            } else {
                ClaimReferenceError::InvalidInput
            }
        })?;
        self.base.work(0, 32)?;
        let retained = if raw.is_some() {
            rich_retained(&result.0, self, 0)?
        } else {
            result.1.capacity()
        };
        if retained > state {
            return Err(ClaimReferenceError::Limit);
        }
        Ok(result)
    }
    fn digest(&mut self, value: &JsonValue) -> ClaimReferenceResult<Digest256> {
        let raw = self.canonical(value, RICH_RAW, 24_576)?;
        self.base.work(raw.len(), 8)?;
        Ok(Digest256::of_bytes(&raw))
    }
}
fn rich_field<'a>(
    v: &'a JsonValue,
    key: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a JsonValue> {
    claim_reference_field(v, key, &mut b.base)?.ok_or(ClaimReferenceError::MissingSource)
}
fn rich_text<'a>(
    v: &'a JsonValue,
    key: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a str> {
    rich_field(v, key, b)?
        .as_str()
        .ok_or(ClaimReferenceError::InvalidInput)
}
fn rich_number(v: &JsonValue, b: &mut RichClaimBudget<'_>) -> ClaimReferenceResult<u64> {
    b.base.work(0, 8)?;
    let JsonValue::Number(n) = v else {
        return Err(ClaimReferenceError::InvalidInput);
    };
    b.base.work(n.lexeme.len(), 8)?;
    n.lexeme
        .parse()
        .map_err(|_| ClaimReferenceError::InvalidInput)
}
fn rich_hex(text: &str, b: &mut RichClaimBudget<'_>) -> ClaimReferenceResult<Digest256> {
    b.base.work(text.len(), 64)?;
    let text = text.strip_prefix("sha256:").unwrap_or(text);
    if text.len() != 64 {
        return Err(ClaimReferenceError::InvalidInput);
    }
    let mut bytes = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let hex = |c| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            _ => Err(ClaimReferenceError::InvalidInput),
        };
        bytes[i] = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Ok(Digest256::from_bytes(bytes))
}
fn rich_same(
    a: &JsonValue,
    c: &JsonValue,
    b: &mut RichClaimBudget<'_>,
    depth: usize,
) -> ClaimReferenceResult<()> {
    b.base.work(0, 16)?;
    if depth > 64 {
        return Err(ClaimReferenceError::Limit);
    }
    let equal = match (a, c) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(c)) => a == c,
        (JsonValue::Number(a), JsonValue::Number(c)) => b
            .base
            .same_bytes(a.lexeme.as_bytes(), c.lexeme.as_bytes())?,
        (JsonValue::String(a), JsonValue::String(c)) => {
            b.base.work(
                a.units()
                    .len()
                    .checked_add(c.units().len())
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(ClaimReferenceError::Limit)?,
                8,
            )?;
            a.units() == c.units()
        }
        (JsonValue::Array(a), JsonValue::Array(c)) if a.len() == c.len() => {
            for (a, c) in a.iter().zip(c) {
                rich_same(a, c, b, depth + 1)?;
            }
            true
        }
        (JsonValue::Object(a), JsonValue::Object(other_fields))
            if a.len() == other_fields.len() =>
        {
            for (key, value) in a {
                let key = key
                    .as_str()
                    .ok_or(ClaimReferenceError::UnsupportedProfile)?;
                rich_same(value, rich_field(c, key, b)?, b, depth + 1)?;
            }
            true
        }
        _ => false,
    };
    if equal {
        Ok(())
    } else {
        Err(ClaimReferenceError::WitnessMismatch)
    }
}
fn rich_input_controls(
    inputs: &DeclaredRichClaimInputs<'_>,
    registry: &RichRegistryInputs,
    mask: u8,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.workspace(
        std::mem::size_of::<[DeclaredClaimObject<'_>; 2]>()
            .checked_add(
                std::mem::size_of::<DeclaredRichClaimInputs<'_>>()
                    .checked_mul(2)
                    .ok_or(ClaimReferenceError::Limit)?,
            )
            .and_then(|n| n.checked_add(256))
            .ok_or(ClaimReferenceError::Limit)?,
    )?;
    let first = inputs
        .records
        .first()
        .ok_or(ClaimReferenceError::MissingSource)?;
    let duplicate = [*first, *first];
    let bad = DeclaredRichClaimInputs {
        selected: inputs.selected,
        software: inputs.software,
        configuration: inputs.configuration,
        request: inputs.request,
        claims: inputs.claims,
        records: &duplicate,
        evidence: inputs.evidence,
        catalogs: inputs.catalogs,
        original_input_projection: inputs.original_input_projection,
    };
    match rich_declared_record(&bad, first.id, b) {
        Err(ClaimReferenceError::AmbiguousIdentity) => {}
        Err(e) => return Err(e),
        Ok(_) => return Err(ClaimReferenceError::WitnessMismatch),
    }
    let missing = DeclaredRichClaimInputs {
        records: &[],
        ..bad
    };
    match rich_declared_record(&missing, first.id, b) {
        Err(ClaimReferenceError::MissingSource) => {}
        Err(e) => return Err(e),
        Ok(_) => return Err(ClaimReferenceError::WitnessMismatch),
    }
    match rich_original_raw(mask, first.path, b"{}", b) {
        Err(ClaimReferenceError::WitnessMismatch) => {}
        Err(e) => return Err(e),
        Ok(()) => return Err(ClaimReferenceError::WitnessMismatch),
    }
    let old = rich_array(
        rich_field(&registry.original, "full_relevant_original_rows", b)?,
        "entity_type_registry",
        b,
    )?;
    let native = rich_unique_row(
        old,
        "type_id",
        if mask == 32 {
            "tos.entity.collection"
        } else {
            "tos.entity.agent"
        },
        b,
    )?;
    b.base.work(0, 16)?;
    if std::mem::size_of::<JsonValue>()
        .checked_add(64)
        .is_none_or(|n| n > 256)
    {
        return Err(ClaimReferenceError::Limit);
    }
    let changed = JsonValue::String(tos_foundation::JsonString::from_utf8("literal"));
    match rich_same(rich_field(native, "object_role", b)?, &changed, b, 0) {
        Err(ClaimReferenceError::WitnessMismatch) => Ok(()),
        Err(e) => Err(e),
        Ok(()) => Err(ClaimReferenceError::WitnessMismatch),
    }
}
fn rich_witness_path(path: &str, b: &mut RichClaimBudget<'_>) -> ClaimReferenceResult<()> {
    b.base.work(
        path.len()
            .checked_mul(4)
            .ok_or(ClaimReferenceError::Limit)?,
        16,
    )?;
    if path.bytes().any(|v| v < 32 || v == b'\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | ".." | ".git"))
    {
        return Err(ClaimReferenceError::InvalidInput);
    }
    Ok(())
}
struct RichOriginalMember {
    mask: u8,
    path: &'static str,
    bytes: usize,
    sha: &'static str,
}
struct RichRegistryInputs {
    original: JsonValue,
    entity: JsonValue,
    relation: JsonValue,
    entity_hash: Digest256,
    relation_hash: Digest256,
}
const RICH_ENTITY_PATH: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RICH_RELATION_PATH: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
fn rich_selected<'a>(
    files: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a [u8]> {
    claim_reference_path(path, &mut b.base)?;
    b.base.work(path.len(), 16)?;
    files
        .get(path)
        .map(Vec::as_slice)
        .ok_or(ClaimReferenceError::MissingSource)
}
fn rich_hash(raw: &[u8], b: &mut RichClaimBudget<'_>) -> ClaimReferenceResult<Digest256> {
    b.base.work(raw.len(), 16)?;
    Ok(Digest256::of_bytes(raw))
}
fn rich_original_raw(
    mask: u8,
    path: &str,
    raw: &[u8],
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let mut found = None;
    for m in RICH_ORIGINAL_MEMBERS {
        b.base.work(
            path.len()
                .checked_add(m.path.len())
                .ok_or(ClaimReferenceError::Limit)?,
            2,
        )?;
        if m.mask & mask != 0 && m.path == path {
            if found.replace(m).is_some() {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    let m = found.ok_or(ClaimReferenceError::MissingSource)?;
    if raw.len() != m.bytes || rich_hash(raw, b)? != rich_hex(m.sha, b)? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    Ok(())
}
fn rich_array<'a>(
    v: &'a JsonValue,
    key: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a [JsonValue]> {
    rich_field(v, key, b)?
        .as_array()
        .ok_or(ClaimReferenceError::InvalidInput)
}
fn rich_unique_row<'a>(
    rows: &'a [JsonValue],
    key: &str,
    id: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a JsonValue> {
    let mut found = None;
    for row in rows {
        b.base.work(0, 8)?;
        let candidate = rich_text(row, key, b)?;
        if b.base.same_bytes(candidate.as_bytes(), id.as_bytes())? && found.replace(row).is_some() {
            return Err(ClaimReferenceError::AmbiguousIdentity);
        }
    }
    found.ok_or(ClaimReferenceError::MissingSource)
}
fn rich_registry_ids(
    rows: &[JsonValue],
    key: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.base.work(0, 32)?;
    if rows.len() > 4096 {
        return Err(ClaimReferenceError::Limit);
    }
    let bytes = rows
        .len()
        .checked_mul(std::mem::size_of::<&str>())
        .ok_or(ClaimReferenceError::Limit)?;
    b.workspace(bytes)?;
    let mut ids: Vec<&str> = Vec::new();
    ids.try_reserve_exact(rows.len())
        .map_err(|_| ClaimReferenceError::Limit)?;
    if ids
        .capacity()
        .checked_mul(std::mem::size_of::<&str>())
        .is_none_or(|n| n > bytes)
    {
        return Err(ClaimReferenceError::Limit);
    }
    for row in rows {
        let id = rich_text(row, key, b)?;
        let (mut lo, mut hi) = (0usize, ids.len());
        while lo < hi {
            b.base.work(0, 16)?;
            let mid = lo
                .checked_add((hi - lo) / 2)
                .ok_or(ClaimReferenceError::Limit)?;
            b.base.work(
                id.len()
                    .checked_add(ids[mid].len())
                    .ok_or(ClaimReferenceError::Limit)?,
                8,
            )?;
            match id.cmp(ids[mid]) {
                std::cmp::Ordering::Less => hi = mid,
                std::cmp::Ordering::Greater => {
                    lo = mid.checked_add(1).ok_or(ClaimReferenceError::Limit)?
                }
                std::cmp::Ordering::Equal => return Err(ClaimReferenceError::AmbiguousIdentity),
            }
        }
        let moved = ids
            .len()
            .checked_sub(lo)
            .and_then(|n| n.checked_mul(std::mem::size_of::<&str>()))
            .ok_or(ClaimReferenceError::Limit)?;
        b.base.work(moved, 16)?;
        ids.insert(lo, id);
    }
    Ok(())
}
fn rich_registry_projection(
    current: &JsonValue,
    historical: &[JsonValue],
    rows_key: &str,
    id_key: &str,
    parents: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let rows = rich_array(current, rows_key, b)?;
    rich_registry_ids(rows, id_key, b)?;
    for old in historical {
        let id = rich_text(old, id_key, b)?;
        rich_same(rich_unique_row(rows, id_key, id, b)?, old, b, 0)?;
        for parent in rich_array(old, parents, b)? {
            let id = parent.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
            rich_unique_row(historical, id_key, id, b)?;
        }
    }
    // Full relevant rows fix ancestry; additionally reject a cycle in that input.
    fn walk<'r>(
        row: &'r JsonValue,
        all: &'r [JsonValue],
        id_key: &str,
        parents: &str,
        stack: &mut [Option<&'r str>; 65],
        depth: usize,
        b: &mut RichClaimBudget<'_>,
    ) -> ClaimReferenceResult<()> {
        b.base.work(0, 32)?;
        if depth >= 64 {
            return Err(ClaimReferenceError::Limit);
        }
        let id = rich_text(row, id_key, b)?;
        for prior in &stack[..depth] {
            b.base.work(id.len(), 4)?;
            if *prior == Some(id) {
                return Err(ClaimReferenceError::InvalidInput);
            }
        }
        stack[depth] = Some(id);
        for p in rich_array(row, parents, b)? {
            let p = p.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
            let next = rich_unique_row(all, id_key, p, b)?;
            // Acyclic original full rows have a bounded strictly shorter chain.
            walk(next, all, id_key, parents, stack, depth + 1, b)?;
        }
        stack[depth] = None;
        Ok(())
    }
    for old in historical {
        walk(old, historical, id_key, parents, &mut [None; 65], 0, b)?;
    }
    if rows_key == "types" {
        for row in rows {
            if let Some(profile) = claim_reference_field(row, "source_record_profile", &mut b.base)?
            {
                for (field, values) in [
                    ("record_type", ["agent", "collection", "work"]),
                    ("id_prefix", ["tos.agent.", "tos.collection.", "tos.work."]),
                    (
                        "source_basename",
                        ["agent.json", "collection.json", "work.json"],
                    ),
                    (
                        "catalog_filename",
                        ["agents.jsonl", "collections.jsonl", "works.jsonl"],
                    ),
                ] {
                    if let Some(value) = claim_reference_field(profile, field, &mut b.base)?
                        .and_then(JsonValue::as_str)
                    {
                        for reserved in values {
                            b.base.work(value.len(), 8)?;
                            if value == reserved {
                                return Err(ClaimReferenceError::AmbiguousIdentity);
                            }
                        }
                    }
                }
            }
        }
    }
    for old in historical {
        let old_id = rich_text(old, id_key, b)?;
        for profile_key in ["source_record_profile", "source_claim_profile"] {
            let Some(profile) = claim_reference_field(old, profile_key, &mut b.base)? else {
                continue;
            };
            let old_schemas = rich_array(profile, "schemas", b)?;
            for row in rows {
                if rich_text(row, id_key, b)? == old_id {
                    continue;
                }
                let Some(other) = claim_reference_field(row, profile_key, &mut b.base)? else {
                    continue;
                };
                for schema in rich_array(other, "schemas", b)? {
                    for original in old_schemas {
                        let version = rich_text(schema, "schema_version", b)?;
                        let path = rich_text(schema, "schema_ref", b)?;
                        b.base.work(
                            version
                                .len()
                                .checked_add(path.len())
                                .ok_or(ClaimReferenceError::Limit)?,
                            16,
                        )?;
                        if version == rich_text(original, "schema_version", b)?
                            || path == rich_text(original, "schema_ref", b)?
                        {
                            return Err(ClaimReferenceError::AmbiguousIdentity);
                        }
                    }
                }
            }
        }
    }
    // Every used source mapping must resolve to one and the same full row.
    for old in historical {
        let id = rich_text(old, id_key, b)?;
        let Some(mappings) = claim_reference_field(old, "source_mappings", &mut b.base)? else {
            continue;
        };
        for mapping in mappings
            .as_array()
            .ok_or(ClaimReferenceError::InvalidInput)?
        {
            let graph = rich_text(mapping, "source_graph", b)?;
            let mut matches = 0usize;
            for row in rows {
                let Some(maps) = claim_reference_field(row, "source_mappings", &mut b.base)? else {
                    continue;
                };
                for m in maps.as_array().ok_or(ClaimReferenceError::InvalidInput)? {
                    b.base.work(0, 16)?;
                    if rich_text(m, "source_graph", b)? != graph {
                        continue;
                    }
                    let fields = mapping
                        .as_object()
                        .ok_or(ClaimReferenceError::InvalidInput)?;
                    let mut same = true;
                    for (k, v) in fields {
                        let k = k.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
                        let Some(other) = claim_reference_field(m, k, &mut b.base)? else {
                            same = false;
                            break;
                        };
                        match rich_same(v, other, b, 0) {
                            Ok(()) => {}
                            Err(ClaimReferenceError::WitnessMismatch) => {
                                same = false;
                                break;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    if same {
                        if rich_text(row, id_key, b)? != id {
                            return Err(ClaimReferenceError::AmbiguousIdentity);
                        }
                        matches = matches.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
                    }
                }
            }
            if matches != 1 {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    Ok(())
}
fn rich_registry_inputs(
    files: &BTreeMap<String, Vec<u8>>,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<RichRegistryInputs> {
    if rich_hash(RICH_ORIGINAL_INPUT.as_bytes(), b)? != rich_hex(RICH_ORIGINAL_INPUT_SHA, b)? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let original = b.parse(RICH_ORIGINAL_INPUT.as_bytes(), RICH_ORIGINAL, 458_752)?;
    let eraw = rich_selected(files, RICH_ENTITY_PATH, b)?;
    let rraw = rich_selected(files, RICH_RELATION_PATH, b)?;
    let entity_hash = rich_hash(eraw, b)?;
    let relation_hash = rich_hash(rraw, b)?;
    let entity = b.parse(eraw, RICH_ENTITY, 2_621_440)?;
    let relation = b.parse(rraw, RICH_RELATION, 3_145_728)?;
    let old = rich_field(&original, "full_relevant_original_rows", b)?;
    rich_registry_projection(
        &entity,
        rich_array(old, "entity_type_registry", b)?,
        "types",
        "type_id",
        "parent_type_ids",
        b,
    )?;
    rich_registry_projection(
        &relation,
        rich_array(old, "relation_type_registry", b)?,
        "relations",
        "relation_type_id",
        "parent_relation_type_ids",
        b,
    )?;
    Ok(RichRegistryInputs {
        original,
        entity,
        relation,
        entity_hash,
        relation_hash,
    })
}
fn rich_schema_inputs(
    inputs: &RichRegistryInputs,
    files: &BTreeMap<String, Vec<u8>>,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let closure = rich_field(&inputs.original, "schema_raw_closure", b)?
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?;
    for (path, commitment) in closure {
        let path = path.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
        let raw = rich_selected(files, path, b)?;
        let size = rich_number(rich_field(commitment, "bytes", b)?, b)?;
        if u64::try_from(raw.len()).map_err(|_| ClaimReferenceError::Limit)? != size
            || rich_hash(raw, b)? != rich_hex(rich_text(commitment, "sha256", b)?, b)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    if rich_hash(rich_selected(files, RICH_ENTITY_PATH, b)?, b)? != inputs.entity_hash
        || rich_hash(rich_selected(files, RICH_RELATION_PATH, b)?, b)? != inputs.relation_hash
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    Ok(())
}
struct DeclaredRawClaimInput<'a> {
    path: &'a str,
    raw: &'a [u8],
}
struct DeclaredRichClaimInputs<'a> {
    selected: &'a BTreeMap<String, Vec<u8>>,
    software: &'a BTreeMap<String, Vec<u8>>,
    configuration: &'a Value,
    request: &'a Value,
    claims: &'a [&'a Value],
    records: &'a [DeclaredClaimObject<'a>],
    evidence: &'a [DeclaredRepoEvidence<'a>],
    catalogs: &'a [DeclaredRawClaimInput<'a>],
    original_input_projection: &'a [u8],
}
fn rich_context(
    inputs: &DeclaredRichClaimInputs<'_>,
    ctx: &tos_command::source_command::CommandContext,
    base: SourceRevision,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.base.work(0, 64)?;
    let n = inputs
        .selected
        .len()
        .checked_add(inputs.software.len())
        .ok_or(ClaimReferenceError::Limit)?;
    if n > 4096
        || ctx.files.len() != n
        || ctx.base_revision != base
        || inputs.configuration.get("uid").and_then(Value::as_u64) != Some(ctx.effective_uid)
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    b.workspace(
        std::mem::size_of::<
            std::iter::Peekable<std::collections::btree_map::Iter<'_, String, Vec<u8>>>,
        >()
        .checked_mul(2)
        .ok_or(ClaimReferenceError::Limit)?,
    )?;
    let mut authored = inputs.selected.iter().peekable();
    let mut software = inputs.software.iter().peekable();
    let mut previous = None;
    for member in &ctx.files {
        let path = member.path.as_str();
        b.base.work(path.len(), 32)?;
        if previous.is_some_and(|old| old >= path) {
            return Err(ClaimReferenceError::AmbiguousIdentity);
        }
        previous = Some(path);
        let take_authored = match (authored.peek(), software.peek()) {
            (Some((a, _)), Some((c, _))) => {
                b.base.work(
                    a.len()
                        .checked_add(c.len())
                        .ok_or(ClaimReferenceError::Limit)?,
                    8,
                )?;
                match a.cmp(c) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Greater => false,
                    std::cmp::Ordering::Equal => {
                        return Err(ClaimReferenceError::AmbiguousIdentity);
                    }
                }
            }
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => return Err(ClaimReferenceError::MissingSource),
        };
        let (declared, raw) = if take_authored {
            authored.next()
        } else {
            software.next()
        }
        .ok_or(ClaimReferenceError::MissingSource)?;
        if !b.base.same_bytes(path.as_bytes(), declared.as_bytes())?
            || raw.len() > 8_388_608
            || !b.base.same_bytes(raw, &member.raw)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    b.base.work(0, 16)?;
    if authored.next().is_some() || software.next().is_some() {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    for (raw, expected) in [
        (&ctx.configuration_raw, inputs.configuration),
        (&ctx.request_raw, inputs.request),
    ] {
        let parsed = b.parse(raw, RICH_CONTEXT, 65_536)?;
        if !claim_reference_same(&parsed, expected, &mut b.base, 0)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        let canonical = b.canonical(&parsed, RICH_CONTEXT, 65_536)?;
        let _ = rich_hash(&canonical, b)?;
    }
    Ok(())
}
fn rich_historical_case<'a>(
    registry: &'a RichRegistryInputs,
    id: &str,
    inputs: &DeclaredRichClaimInputs<'_>,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a JsonValue> {
    if !b.base.same_bytes(
        inputs.original_input_projection,
        RICH_ORIGINAL_INPUT.as_bytes(),
    )? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let old = rich_unique_row(
        rich_array(&registry.original, "original_input_witness_bindings", b)?,
        "oracle_id",
        id,
        b,
    )?;
    let raw = match id {
        "claim-revision-identity-history-v1" => include_bytes!(
            "legacy-python-oracles-v1/claim-revision-identity-history-v1.provenance.json"
        )
        .as_slice(),
        "claim-revision-identity-history-v2" => include_bytes!(
            "legacy-python-oracles-v1/claim-revision-identity-history-v2.provenance.json"
        )
        .as_slice(),
        "claim-revision-catalog-history" => include_bytes!(
            "legacy-python-oracles-v1/claim-revision-catalog-history.provenance.json"
        )
        .as_slice(),
        _ => return Err(ClaimReferenceError::UnsupportedProfile),
    };
    let commitment = rich_field(old, "provenance_raw", b)?;
    if u64::try_from(raw.len()).map_err(|_| ClaimReferenceError::Limit)?
        != rich_number(rich_field(commitment, "bytes", b)?, b)?
        || rich_hash(raw, b)? != rich_hex(rich_text(commitment, "sha256", b)?, b)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let mask = match id {
        "claim-revision-identity-history-v1" => 8,
        "claim-revision-identity-history-v2" => 16,
        "claim-revision-catalog-history" => 32,
        _ => return Err(ClaimReferenceError::UnsupportedProfile),
    };
    let mut previous_path = None;
    let mut count = 0usize;
    let mut total = 0usize;
    for member in RICH_ORIGINAL_MEMBERS {
        b.base.work(
            member
                .path
                .len()
                .checked_add(member.sha.len())
                .ok_or(ClaimReferenceError::Limit)?,
            16,
        )?;
        if member.mask & mask == 0 {
            continue;
        }
        rich_witness_path(member.path, b)?;
        if previous_path.is_some_and(|p| p >= member.path) {
            return Err(ClaimReferenceError::AmbiguousIdentity);
        }
        previous_path = Some(member.path);
        let _ = rich_hex(member.sha, b)?;
        count = count.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
        total = total
            .checked_add(member.bytes)
            .ok_or(ClaimReferenceError::Limit)?;
    }
    let commitment = rich_field(old, "selected_source_commitment", b)?;
    if u64::try_from(count).map_err(|_| ClaimReferenceError::Limit)?
        != rich_number(rich_field(commitment, "entry_count", b)?, b)?
        || u64::try_from(total).map_err(|_| ClaimReferenceError::Limit)?
            != rich_number(rich_field(commitment, "total_bytes", b)?, b)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let original = rich_field(old, "configuration", b)?;
    let current = inputs
        .configuration
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?;
    if original
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?
        .len()
        != current.len()
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    for (key, value) in current {
        b.base.work(key.len(), 16)?;
        let old = rich_field(original, key, b)?;
        let same = match key.as_str() {
            "source_root" => {
                old.as_str() == Some("<isolated-source-root>")
                    && value.as_str().is_some_and(|v| Path::new(v).is_absolute())
            }
            "uid" => rich_number(old, b)? == 1000 && value.as_u64().is_some(),
            _ => claim_reference_same(old, value, &mut b.base, 0)?,
        };
        if !same {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    if !claim_reference_same(
        rich_field(old, "request", b)?,
        inputs.request,
        &mut b.base,
        0,
    )? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    Ok(old)
}
fn rich_read_catalog(
    root: &Path,
    name: &'static str,
    mask: u8,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<Vec<u8>> {
    use std::io::Read;
    claim_reference_path(name, &mut b.base)?;
    let cap = root
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .checked_add(name.len())
        .and_then(|n| n.checked_add(2))
        .ok_or(ClaimReferenceError::Limit)?;
    b.own(RICH_RAW, cap)?;
    let mut path = std::path::PathBuf::new();
    path.try_reserve_exact(cap)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if path.capacity() > cap {
        return Err(ClaimReferenceError::Limit);
    }
    path.push(root);
    path.push(name);
    b.base.active()?;
    let mut file = fs::File::open(&path).map_err(|_| ClaimReferenceError::MissingSource)?;
    let size = usize::try_from(
        file.metadata()
            .map_err(|_| ClaimReferenceError::InvalidInput)?
            .len(),
    )
    .map_err(|_| ClaimReferenceError::Limit)?;
    if size > 1_048_576 {
        return Err(ClaimReferenceError::Limit);
    }
    let reserve = size.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
    b.own(RICH_RAW, reserve)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(reserve)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if raw.capacity() > reserve {
        return Err(ClaimReferenceError::Limit);
    }
    raw.resize(size, 0);
    for chunk in raw.chunks_mut(4096) {
        b.base.work(chunk.len(), 8)?;
        file.read_exact(chunk)
            .map_err(|_| ClaimReferenceError::InvalidInput)?;
    }
    let mut end = [0u8; 1];
    b.base.work(1, 8)?;
    if file
        .read(&mut end)
        .map_err(|_| ClaimReferenceError::InvalidInput)?
        != 0
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    rich_original_raw(mask, name, &raw, b)?;
    Ok(raw)
}
struct RichPiece<'a> {
    path: &'a str,
    line: usize,
    raw: &'a [u8],
}
struct RichBatch<'a> {
    pieces: Vec<RichPiece<'a>>,
    parsed: JsonValue,
}
fn rich_add_piece<'a>(
    pieces: &mut Vec<RichPiece<'a>>,
    path: &'a str,
    line: usize,
    raw: &'a [u8],
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.base.work(path.len(), 16)?;
    for p in pieces.iter() {
        b.base.work(path.len(), 4)?;
        if p.path == path && p.line == line {
            if !b.base.same_bytes(p.raw, raw)? {
                return Err(ClaimReferenceError::WitnessMismatch);
            }
            return Ok(());
        }
    }
    if pieces.len() == pieces.capacity() {
        return Err(ClaimReferenceError::Limit);
    }
    pieces.push(RichPiece { path, line, raw });
    Ok(())
}
fn rich_batch<'a>(
    inputs: &'a DeclaredRichClaimInputs<'a>,
    mask: u8,
    collection: bool,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<RichBatch<'a>> {
    let slots = 128usize
        .checked_mul(std::mem::size_of::<RichPiece<'_>>())
        .ok_or(ClaimReferenceError::Limit)?;
    b.workspace(slots)?;
    let mut pieces = Vec::new();
    pieces
        .try_reserve_exact(128)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if pieces
        .capacity()
        .checked_mul(std::mem::size_of::<RichPiece<'_>>())
        .is_none_or(|v| v > slots)
    {
        return Err(ClaimReferenceError::Limit);
    }
    for d in inputs.records {
        rich_original_raw(mask, d.path, d.raw, b)?;
        let selected = rich_selected(inputs.selected, d.path, b)?;
        if !b.base.same_bytes(d.raw, selected)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        rich_add_piece(&mut pieces, d.path, 0, d.raw, b)?;
    }
    for d in inputs.evidence {
        rich_original_raw(mask, d.path, d.raw, b)?;
        let selected = rich_selected(inputs.selected, d.path, b)?;
        if !b.base.same_bytes(d.raw, selected)? {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    if collection {
        for (path, raw) in inputs.selected {
            b.base.work(path.len(), 8)?;
            if path.starts_with("ToS/source-witnesses/.record-revisions/")
                || (path.starts_with("ToS/source-witnesses/collections/")
                    && (path.ends_with("/source-revision-history.json")
                        || path.ends_with("/membership-claims.jsonl")))
            {
                rich_original_raw(mask, path, raw, b)?;
                if path.ends_with(".jsonl") {
                    for (i, line) in raw.split(|v| *v == b'\n').enumerate() {
                        b.base.work(line.len(), 8)?;
                        if !line.iter().all(u8::is_ascii_whitespace) {
                            rich_add_piece(
                                &mut pieces,
                                path,
                                i.checked_add(1).ok_or(ClaimReferenceError::Limit)?,
                                line,
                                b,
                            )?;
                        }
                    }
                } else {
                    rich_add_piece(&mut pieces, path, 0, raw, b)?;
                }
            } else if path.starts_with("ToS/source-witnesses/.metadata-transactions/") {
                rich_original_raw(mask, path, raw, b)?;
            }
        }
    }
    for d in inputs.catalogs {
        rich_original_raw(mask, d.path, d.raw, b)?;
        for (i, line) in d.raw.split(|v| *v == b'\n').enumerate() {
            b.base.work(line.len(), 8)?;
            if !line.iter().all(u8::is_ascii_whitespace) {
                rich_add_piece(
                    &mut pieces,
                    d.path,
                    i.checked_add(1).ok_or(ClaimReferenceError::Limit)?,
                    line,
                    b,
                )?;
            }
        }
    }
    b.base.work(
        0,
        pieces
            .len()
            .checked_mul(16)
            .and_then(|n| n.checked_add(32))
            .ok_or(ClaimReferenceError::Limit)?,
    )?;
    let capacity = pieces
        .iter()
        .try_fold(2usize, |sum, p| {
            sum.checked_add(p.raw.len()).and_then(|s| s.checked_add(1))
        })
        .ok_or(ClaimReferenceError::Limit)?;
    if capacity > 114_688 {
        return Err(ClaimReferenceError::Limit);
    }
    b.own(RICH_RAW, capacity)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if raw.capacity() > capacity {
        return Err(ClaimReferenceError::Limit);
    }
    raw.push(b'[');
    for (i, p) in pieces.iter().enumerate() {
        b.base.work(p.raw.len(), 8)?;
        if i != 0 {
            raw.push(b',');
        }
        raw.extend_from_slice(p.raw);
    }
    raw.push(b']');
    // One paid strict array frames the original raw JSON values. No renderer or
    // serializer supplies an answer; all original whole-file hashes were checked.
    let parsed = b.parse(&raw, RICH_RAW, if collection { 360_448 } else { 131_072 })?;
    if parsed
        .as_array()
        .ok_or(ClaimReferenceError::InvalidInput)?
        .len()
        != pieces.len()
    {
        return Err(ClaimReferenceError::InvalidInput);
    }
    Ok(RichBatch { pieces, parsed })
}
fn rich_document<'a>(
    batch: &'a RichBatch<'_>,
    path: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a JsonValue> {
    let mut found = None;
    for (p, v) in batch.pieces.iter().zip(
        batch
            .parsed
            .as_array()
            .ok_or(ClaimReferenceError::InvalidInput)?,
    ) {
        b.base.work(path.len(), 8)?;
        if p.path == path && p.line == 0 {
            if found.replace(v).is_some() {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    found.ok_or(ClaimReferenceError::MissingSource)
}
fn rich_record_ref(
    record: &JsonValue,
    reference: &JsonValue,
    identity: &str,
    version: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<Digest256> {
    b.base.work(0, 32)?;
    if reference
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?
        .len()
        != 3
        || rich_text(reference, "id", b)?.is_empty()
        || rich_number(rich_field(reference, "version", b)?, b)? == 0
        || !rich_text(reference, "digest", b)?.starts_with("sha256:")
    {
        return Err(ClaimReferenceError::InvalidInput);
    }
    let digest = b.digest(record)?;
    if rich_text(record, identity, b)? != rich_text(reference, "id", b)?
        || rich_number(rich_field(record, version, b)?, b)?
            != rich_number(rich_field(reference, "version", b)?, b)?
        || digest != rich_hex(rich_text(reference, "digest", b)?, b)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    Ok(digest)
}
fn rich_catalog<'a>(
    batch: &'a RichBatch<'_>,
    path: &str,
    id: &str,
    identity: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<(&'a JsonValue, usize)> {
    let mut found = None;
    for (p, v) in batch.pieces.iter().zip(
        batch
            .parsed
            .as_array()
            .ok_or(ClaimReferenceError::InvalidInput)?,
    ) {
        b.base.work(path.len(), 8)?;
        if p.path == path && p.line != 0 && rich_text(v, identity, b)? == id {
            if found.replace((v, p.line)).is_some() {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    found.ok_or(ClaimReferenceError::MissingSource)
}
fn rich_declared_record<'a>(
    inputs: &'a DeclaredRichClaimInputs<'a>,
    id: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<&'a DeclaredClaimObject<'a>> {
    let mut found = None;
    for record in inputs.records {
        b.base.work(
            record
                .id
                .len()
                .checked_add(id.len())
                .ok_or(ClaimReferenceError::Limit)?,
            8,
        )?;
        if record.id == id {
            if found.replace(record).is_some() {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
    }
    found.ok_or(ClaimReferenceError::MissingSource)
}
fn rich_native_record(
    inputs: &DeclaredRichClaimInputs<'_>,
    batch: &RichBatch<'_>,
    reference: &JsonValue,
    kind: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let d = rich_declared_record(inputs, rich_text(reference, "id", b)?, b)?;
    let record = rich_document(batch, d.path, b)?;
    if d.identity_field != "record_id"
        || rich_text(record, "schema_version", b)? != "tos_corpus_record_v1"
        || rich_text(record, "record_type", b)? != kind
        || !d.path.ends_with(match kind {
            "agent" => "/agent.json",
            "collection" => "/collection.json",
            "work" => "/work.json",
            _ => return Err(ClaimReferenceError::UnsupportedProfile),
        })
    {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    let digest = rich_record_ref(record, reference, "record_id", "record_version", b)?;
    let catalog = match kind {
        "agent" => "ToS/source-witnesses/catalog/agents.jsonl",
        "collection" => "ToS/source-witnesses/catalog/collections.jsonl",
        "work" => "ToS/source-witnesses/catalog/works.jsonl",
        _ => return Err(ClaimReferenceError::UnsupportedProfile),
    };
    let (row, _) = rich_catalog(batch, catalog, d.id, "record_id", b)?;
    if rich_text(row, "source_record_ref", b)? != d.path {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    if digest != rich_hex(rich_text(row, "record_sha256", b)?, b)? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    // The complete committed catalog bytes (not only this projection) already
    // match their original INPUT witness. No actual catalog response is copied.
    Ok(())
}
fn rich_claim_inputs(
    inputs: &DeclaredRichClaimInputs<'_>,
    registry: &RichRegistryInputs,
    batch: &RichBatch<'_>,
    identity: bool,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    b.base.work(0, 64)?;
    if inputs.claims.len() != 1 {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    let claim = inputs.claims[0];
    let object = claim
        .get("object")
        .ok_or(ClaimReferenceError::MissingSource)?;
    let allowed = inputs
        .configuration
        .get("allowed_object_values")
        .and_then(Value::as_array)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    if allowed.len() != 1 {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    claim_reference_equal(object, &allowed[0], &mut b.base, 0)?;
    for d in inputs.evidence {
        let mut count = 0usize;
        for key in ["evidence_refs", "counterevidence_refs"] {
            if let Some(refs) = claim.get(key).and_then(Value::as_array) {
                for r in refs {
                    b.base.work(d.reference.len(), 8)?;
                    if r.as_str() == Some(d.reference) {
                        count = count.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
                    }
                }
            }
        }
        if count == 0 || d.path != d.reference {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    let old = rich_field(&registry.original, "full_relevant_original_rows", b)?;
    let old_relations = rich_array(old, "relation_type_registry", b)?;
    let predicate = claim
        .get("predicate")
        .and_then(Value::as_str)
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let wanted = match predicate {
        "identity_transition_proposal" => "tos.relation.identity-transition-proposal",
        "subject_identity_transition_proposal" => {
            "tos.relation.subject-identity-transition-proposal"
        }
        "collection_member_order" => "tos.relation.collection-member-order",
        _ => return Err(ClaimReferenceError::UnsupportedProfile),
    };
    let route = rich_unique_row(old_relations, "relation_type_id", wanted, b)?;
    let profile = rich_field(route, "source_claim_profile", b)?;
    let source_case = rich_unique_row(
        rich_array(&registry.original, "original_input_witness_bindings", b)?,
        "oracle_id",
        match predicate {
            "identity_transition_proposal" => "claim-revision-identity-history-v1",
            "subject_identity_transition_proposal" => "claim-revision-identity-history-v2",
            _ => "claim-revision-catalog-history",
        },
        b,
    )?;
    let source_value = rich_array(
        rich_field(source_case, "configuration", b)?,
        "allowed_object_values",
        b,
    )?
    .first()
    .ok_or(ClaimReferenceError::MissingSource)?;
    let _ = b.digest(source_value)?;
    let reader = rich_text(profile, "reader", b)?;
    if reader
        != match predicate {
            "identity_transition_proposal" => "identity-transition-v1",
            "subject_identity_transition_proposal" => "identity-transition-v2",
            _ => "structured-reference-value-v1",
        }
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    if identity {
        if inputs.records.len() != 3
            || object.get("supersedes_proposal") != Some(&Value::Null)
            || !inputs
                .configuration
                .get("allowed_related_claim_refs")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        {
            return Err(ClaimReferenceError::UnsupportedProfile);
        }
        for key in ["predecessors", "successors"] {
            for reference in object
                .get(key)
                .and_then(Value::as_array)
                .ok_or(ClaimReferenceError::InvalidInput)?
            {
                b.base.work(0, 16)?;
                // The same full object is authenticated through original config;
                // use its Foundation counterpart, never a converted SUT result.
                let old_cases =
                    rich_array(&registry.original, "original_input_witness_bindings", b)?;
                let case = rich_unique_row(
                    old_cases,
                    "oracle_id",
                    if predicate == "identity_transition_proposal" {
                        "claim-revision-identity-history-v1"
                    } else {
                        "claim-revision-identity-history-v2"
                    },
                    b,
                )?;
                let value = rich_array(
                    rich_field(case, "configuration", b)?,
                    "allowed_object_values",
                    b,
                )?
                .first()
                .ok_or(ClaimReferenceError::MissingSource)?;
                let mut found = None;
                for candidate in rich_array(value, key, b)? {
                    if claim_reference_same(candidate, reference, &mut b.base, 0)? {
                        if found.replace(candidate).is_some() {
                            return Err(ClaimReferenceError::AmbiguousIdentity);
                        }
                    }
                }
                rich_native_record(
                    inputs,
                    batch,
                    found.ok_or(ClaimReferenceError::WitnessMismatch)?,
                    "agent",
                    b,
                )?;
            }
        }
        for d in inputs.records {
            let home = d
                .path
                .strip_suffix("agent.json")
                .ok_or(ClaimReferenceError::UnsupportedProfile)?;
            for path in inputs.selected.keys() {
                b.base.work(path.len(), 8)?;
                if path.strip_prefix(home) == Some("source-revision-history.json") {
                    return Err(ClaimReferenceError::UnsupportedProfile);
                }
            }
        }
    }
    Ok(())
}
fn rich_retained(
    v: &JsonValue,
    b: &mut RichClaimBudget<'_>,
    depth: usize,
) -> ClaimReferenceResult<usize> {
    // Capacity metadata units, not semantic string comparison or parser work:
    // 2 tag/depth, 4 container layout, 4 two-buffer key/string metadata, and
    // one checked accumulator addition. No child byte contents are scanned.
    b.base.work(0, 2)?;
    if depth > 64 {
        return Err(ClaimReferenceError::Limit);
    }
    let add = |a: usize, c: usize| a.checked_add(c).ok_or(ClaimReferenceError::Limit);
    Ok(match v {
        JsonValue::Null | JsonValue::Bool(_) => 0,
        JsonValue::Number(v) => v.lexeme.capacity(),
        JsonValue::String(v) => {
            b.base.work(0, 4)?;
            v.retained_storage_bytes()
                .map_err(|_| ClaimReferenceError::Limit)?
        }
        JsonValue::Array(v) => {
            b.base.work(0, 4)?;
            let mut n = v
                .capacity()
                .checked_mul(std::mem::size_of::<JsonValue>())
                .ok_or(ClaimReferenceError::Limit)?;
            for child in v {
                let child = rich_retained(child, b, depth + 1)?;
                b.base.work(0, 1)?;
                n = add(n, child)?;
            }
            n
        }
        JsonValue::Object(v) => {
            b.base.work(0, 4)?;
            let mut n = v
                .capacity()
                .checked_mul(std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>())
                .ok_or(ClaimReferenceError::Limit)?;
            for (k, child) in v {
                b.base.work(0, 4)?;
                let key = k
                    .retained_storage_bytes()
                    .map_err(|_| ClaimReferenceError::Limit)?;
                b.base.work(0, 1)?;
                n = add(n, key)?;
                let child = rich_retained(child, b, depth + 1)?;
                b.base.work(0, 1)?;
                n = add(n, child)?;
            }
            n
        }
    })
}
fn rich_string(parts: &[&str], b: &mut RichClaimBudget<'_>) -> ClaimReferenceResult<String> {
    b.base.work(0, 32)?;
    let n = parts
        .iter()
        .try_fold(0usize, |s, p| s.checked_add(p.len()))
        .ok_or(ClaimReferenceError::Limit)?;
    b.own(RICH_RAW, n)?;
    let mut s = String::new();
    s.try_reserve_exact(n)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if s.capacity() > n {
        return Err(ClaimReferenceError::Limit);
    }
    for p in parts {
        b.base.work(p.len(), 4)?;
        s.push_str(p);
    }
    Ok(s)
}
fn rich_package(
    inputs: &DeclaredRichClaimInputs<'_>,
    batch: &RichBatch<'_>,
    receipt: &JsonValue,
    source_path: &str,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let previous = rich_field(receipt, "previous_source", b)?;
    let id = rich_text(previous, "id", b)?;
    let id_digest = rich_hash(id.as_bytes(), b)?;
    b.own(RICH_RAW, 64)?;
    let id_hex = id_digest.to_hex();
    let revision = rich_text(receipt, "previous_revision", b)?;
    let bare = revision
        .strip_prefix("sha256:")
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let _ = rich_hex(bare, b)?;
    let prefix = rich_string(
        &[
            "ToS/source-witnesses/.record-revisions/",
            &id_hex,
            "-",
            bare,
        ],
        b,
    )?;
    if prefix != rich_text(receipt, "archive_path", b)? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let manifest_path = rich_string(&[&prefix, "/manifest.json"], b)?;
    let manifest = rich_document(batch, &manifest_path, b)?;
    if manifest
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?
        .len()
        != 6
        || rich_text(manifest, "schema_version", b)? != "tos_source_package_archive_v2"
        || rich_text(manifest, "publication_protocol", b)? != "tos_selected_source_metadata_v1"
        || rich_text(manifest, "source_path", b)? != source_path
        || rich_text(manifest, "revision", b)? != revision
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    rich_same(rich_field(manifest, "source", b)?, previous, b, 0)?;
    let members = rich_field(manifest, "files", b)?
        .as_object()
        .ok_or(ClaimReferenceError::InvalidInput)?;
    if members.is_empty() || members.len() > 64 {
        return Err(ClaimReferenceError::Limit);
    }
    b.base.work(0, 64)?;
    if std::mem::size_of::<JsonValue>()
        .checked_add(std::mem::size_of::<
            Vec<(tos_foundation::JsonString, JsonValue)>,
        >())
        .is_none_or(|n| n > 1024)
    {
        return Err(ClaimReferenceError::Limit);
    }
    // Preflight every copied member before allocating the fingerprint. Fresh
    // UTF-16 growth plus its UTF-8 copy is bounded by 6*n+32; scalar Clone
    // capacities are bounded by their retained source buffers. Headers and
    // short fixed keys have a separate 256-byte per-member allowance.
    let mut allocation = 1024usize;
    for (name, binding) in members {
        let name = name.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
        b.base.work(name.len(), 128)?;
        if name.is_empty()
            || name.len() > 256
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
            || name == ".git"
            || name.bytes().any(|v| v < 32)
            || binding
                .as_object()
                .ok_or(ClaimReferenceError::InvalidInput)?
                .len()
                != 3
        {
            return Err(ClaimReferenceError::InvalidInput);
        }
        let sha = rich_field(binding, "sha256", b)?;
        let digest = sha.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
        if digest.len() != 71 || !digest.starts_with("sha256:") {
            return Err(ClaimReferenceError::InvalidInput);
        }
        let _ = rich_hex(digest, b)?;
        let bytes = rich_field(binding, "bytes", b)?;
        if rich_number(bytes, b)? > 2_097_152 {
            return Err(ClaimReferenceError::Limit);
        }
        let sha_heap = rich_retained(sha, b, 0)?;
        let bytes_heap = rich_retained(bytes, b, 0)?;
        let member = std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>()
            .checked_mul(3)
            .and_then(|n| name.len().checked_mul(6).and_then(|v| n.checked_add(v)))
            .and_then(|n| n.checked_add(32))
            .and_then(|n| n.checked_add(sha_heap))
            .and_then(|n| n.checked_add(bytes_heap))
            .and_then(|n| n.checked_add(256))
            .ok_or(ClaimReferenceError::Limit)?;
        allocation = allocation
            .checked_add(member)
            .ok_or(ClaimReferenceError::Limit)?;
    }
    b.own(RICH_RAW, allocation)?;
    let mut fingerprint = Vec::new();
    fingerprint
        .try_reserve_exact(members.len())
        .map_err(|_| ClaimReferenceError::Limit)?;
    if fingerprint.capacity() > members.len() {
        return Err(ClaimReferenceError::Limit);
    }
    let mut saw_record = false;
    let mut package_bytes = 0u64;
    for (name, binding) in members {
        let name = name.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
        b.base.work(name.len(), 64)?;
        if name.len() > 256
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
            || name == ".git"
            || name.bytes().any(|v| v < 32)
            || binding
                .as_object()
                .ok_or(ClaimReferenceError::InvalidInput)?
                .len()
                != 3
        {
            return Err(ClaimReferenceError::InvalidInput);
        }
        let blob = rich_text(binding, "blob", b)?;
        let digest = rich_text(binding, "sha256", b)?;
        let size = rich_number(rich_field(binding, "bytes", b)?, b)?;
        package_bytes = package_bytes
            .checked_add(size)
            .ok_or(ClaimReferenceError::Limit)?;
        if package_bytes > 8_388_608 {
            return Err(ClaimReferenceError::Limit);
        }

        if size > 2_097_152 || blob.strip_suffix(".blob") != digest.strip_prefix("sha256:") {
            return Err(ClaimReferenceError::InvalidInput);
        }
        let path = rich_string(&[&prefix, "/", blob], b)?;
        let raw = rich_selected(inputs.selected, &path, b)?;
        if u64::try_from(raw.len()).map_err(|_| ClaimReferenceError::Limit)? != size
            || rich_hash(raw, b)? != rich_hex(digest, b)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        if name == "collection.json" {
            saw_record = true;
            rich_record_ref(
                rich_document(batch, &path, b)?,
                previous,
                "record_id",
                "record_version",
                b,
            )?;
        }
        // Closed primitive copies: two scalar bindings and short schema keys.
        // The complete preflight reservation precedes every Vec/string clone.
        let sha = rich_field(binding, "sha256", b)?;
        let bytes = rich_field(binding, "bytes", b)?;
        let (JsonValue::String(sha_text), JsonValue::Number(byte_number)) = (sha, bytes) else {
            return Err(ClaimReferenceError::InvalidInput);
        };
        // Metadata/state admission is not copy work. Pay source-buffer read and
        // destination writes before either scalar Clone or UTF-16 conversion.
        b.base.work(0, 64)?;
        let copy_work = sha_text
            .units()
            .len()
            .checked_mul(2)
            .and_then(|n| sha_text.as_str().and_then(|s| n.checked_add(s.len())))
            .and_then(|n| n.checked_add(byte_number.lexeme.len()))
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| name.len().checked_mul(6).and_then(|v| n.checked_add(v)))
            .and_then(|n| n.checked_add(66)) // six units per byte of fixed keys
            .ok_or(ClaimReferenceError::Limit)?;
        b.base.work(copy_work, 16)?;
        fingerprint.push((
            tos_foundation::JsonString::from_utf8(name),
            JsonValue::Object(vec![
                (tos_foundation::JsonString::from_utf8("sha256"), sha.clone()),
                (
                    tos_foundation::JsonString::from_utf8("bytes"),
                    bytes.clone(),
                ),
            ]),
        ));
    }
    if !saw_record {
        return Err(ClaimReferenceError::MissingSource);
    }
    let fingerprint = JsonValue::Object(fingerprint);
    if rich_retained(&fingerprint, b, 0)?
        .checked_add(std::mem::size_of::<JsonValue>())
        .is_none_or(|n| n > allocation)
        || b.digest(&fingerprint)? != rich_hex(revision, b)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let receipt_request = rich_field(receipt, "request", b)?;
    if b.digest(receipt_request)? != rich_hex(rich_text(receipt, "request_digest", b)?, b)? {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let publication = rich_field(receipt, "publication", b)?;
    let tx = rich_text(publication, "transaction_id", b)?
        .strip_prefix("sha256:")
        .ok_or(ClaimReferenceError::InvalidInput)?;
    let _ = rich_hex(tx, b)?;
    let transaction = rich_string(
        &["ToS/source-witnesses/.metadata-transactions/", tx, "/"],
        b,
    )?;
    let mut count = 0usize;
    let mut manifest = false;
    let mut completion = false;
    for m in RICH_ORIGINAL_MEMBERS {
        b.base.work(m.path.len(), 4)?;
        if m.mask & 32 != 0 && m.path.starts_with(&transaction) {
            rich_original_raw(32, m.path, rich_selected(inputs.selected, m.path, b)?, b)?;
            manifest |= m.path.strip_prefix(&transaction) == Some("manifest.json");
            completion |= m.path.strip_prefix(&transaction) == Some("completion.json");
            count = count.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
        }
    }
    if count == 0 || !manifest || !completion {
        return Err(ClaimReferenceError::MissingSource);
    }
    Ok(())
}
fn check_identity_claim_inputs(
    inputs: &DeclaredRichClaimInputs<'_>,
    version: u8,
    context: &tos_command::source_command::CommandContext,
    base: SourceRevision,
    registry: &RichRegistryInputs,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    let (id, mask) = match version {
        1 => ("claim-revision-identity-history-v1", 8),
        2 => ("claim-revision-identity-history-v2", 16),
        _ => return Err(ClaimReferenceError::UnsupportedProfile),
    };
    rich_context(inputs, context, base, b)?;
    rich_schema_inputs(registry, inputs.selected, b)?;
    rich_historical_case(registry, id, inputs, b)?;
    rich_input_controls(inputs, registry, mask, b)?;
    let batch = rich_batch(inputs, mask, false, b)?;
    rich_claim_inputs(inputs, registry, &batch, true, b)?;
    Ok(())
}
fn check_collection_claim_inputs(
    inputs: &DeclaredRichClaimInputs<'_>,
    context: &tos_command::source_command::CommandContext,
    base: SourceRevision,
    registry: &RichRegistryInputs,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<()> {
    rich_context(inputs, context, base, b)?;
    rich_schema_inputs(registry, inputs.selected, b)?;
    let case = rich_historical_case(registry, "claim-revision-catalog-history", inputs, b)?;
    let value = rich_array(
        rich_field(case, "configuration", b)?,
        "allowed_object_values",
        b,
    )?
    .first()
    .ok_or(ClaimReferenceError::MissingSource)?;
    rich_input_controls(inputs, registry, 32, b)?;
    let batch = rich_batch(inputs, 32, true, b)?;
    rich_claim_inputs(inputs, registry, &batch, false, b)?;
    let requested = rich_field(value, "collection_version", b)?;
    let subject = rich_text(requested, "id", b)?;
    let primary = rich_declared_record(inputs, subject, b)?;
    let current = rich_document(&batch, primary.path, b)?;
    let home = primary
        .path
        .strip_suffix("collection.json")
        .ok_or(ClaimReferenceError::UnsupportedProfile)?;
    let history_path = rich_string(&[home, "source-revision-history.json"], b)?;
    let history = rich_document(&batch, &history_path, b)?;
    if rich_text(history, "schema_version", b)? != "tos_source_revision_history_v2"
        || rich_text(history, "record_id", b)? != subject
        || history
            .as_object()
            .ok_or(ClaimReferenceError::InvalidInput)?
            .len()
            != 3
    {
        return Err(ClaimReferenceError::UnsupportedProfile);
    }
    let receipts = rich_array(history, "receipts", b)?;
    if receipts.is_empty() || receipts.len() > 128 {
        return Err(ClaimReferenceError::Limit);
    }
    let mut selected = 0usize;
    let mut selected_record = None;
    let mut previous = None;
    for receipt in receipts {
        if let Some(old) = previous {
            rich_same(old, rich_field(receipt, "previous_source", b)?, b, 0)?;
        }
        let from = rich_field(receipt, "previous_source", b)?;
        let to = rich_field(receipt, "source", b)?;
        if rich_text(from, "id", b)? != subject
            || rich_text(to, "id", b)? != subject
            || rich_number(rich_field(to, "version", b)?, b)?
                != rich_number(rich_field(from, "version", b)?, b)?
                    .checked_add(1)
                    .ok_or(ClaimReferenceError::Limit)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        let is_requested = match rich_same(from, requested, b, 0) {
            Ok(()) => {
                selected = selected.checked_add(1).ok_or(ClaimReferenceError::Limit)?;
                true
            }
            Err(ClaimReferenceError::WitnessMismatch) => false,
            Err(e) => return Err(e),
        };
        rich_package(inputs, &batch, receipt, primary.path, b)?;
        if is_requested {
            let manifest_path = rich_string(
                &[rich_text(receipt, "archive_path", b)?, "/manifest.json"],
                b,
            )?;
            let manifest = rich_document(&batch, &manifest_path, b)?;
            let binding = rich_field(rich_field(manifest, "files", b)?, "collection.json", b)?;
            let archive_path = rich_string(
                &[
                    rich_text(receipt, "archive_path", b)?,
                    "/",
                    rich_text(binding, "blob", b)?,
                ],
                b,
            )?;
            selected_record = Some(rich_document(&batch, &archive_path, b)?);
        }

        previous = Some(to);
    }
    if selected != 1 {
        return Err(ClaimReferenceError::AmbiguousIdentity);
    }
    let current_ref = previous.ok_or(ClaimReferenceError::MissingSource)?;
    rich_native_record(inputs, &batch, current_ref, "collection", b)?;
    if rich_number(rich_field(current, "record_version", b)?, b)?
        <= rich_number(rich_field(requested, "version", b)?, b)?
    {
        return Err(ClaimReferenceError::WitnessMismatch);
    }
    let declared = rich_array(value, "members", b)?;
    let memberships = rich_array(value, "membership_versions", b)?;
    if declared.len() != memberships.len() || memberships.is_empty() || memberships.len() > 128 {
        return Err(ClaimReferenceError::InvalidInput);
    }
    let stream_path = rich_string(&[home, "membership-claims.jsonl"], b)?;
    b.workspace(
        std::mem::size_of::<[Option<&str>; 128]>()
            .checked_mul(2)
            .ok_or(ClaimReferenceError::Limit)?,
    )?;
    let mut seen_ids: [Option<&str>; 128] = [None; 128];
    let mut seen_members: [Option<&str>; 128] = [None; 128];
    for (i, reference) in memberships.iter().enumerate() {
        let id = rich_text(reference, "id", b)?;
        let (row, _) = rich_catalog(
            &batch,
            "ToS/source-witnesses/catalog/claims.jsonl",
            id,
            "claim_id",
            b,
        )?;
        let mut found = None;
        for (piece, record) in batch.pieces.iter().zip(
            batch
                .parsed
                .as_array()
                .ok_or(ClaimReferenceError::InvalidInput)?,
        ) {
            b.base.work(piece.path.len(), 8)?;
            if piece.path == stream_path
                && piece.line != 0
                && rich_text(record, "claim_id", b)? == id
            {
                if found.replace((piece, record)).is_some() {
                    return Err(ClaimReferenceError::AmbiguousIdentity);
                }
            }
        }
        let (piece, record) = found.ok_or(ClaimReferenceError::MissingSource)?;
        if rich_text(record, "schema_version", b)? != "tos_claim_packet_v1"
            || rich_text(record, "claim_type", b)? != "bibliographic"
            || rich_text(record, "subject_ref", b)? != subject
            || rich_text(record, "predicate", b)? != "contains_work"
            || !["bibliographic_assertion", "scholarly_report"].contains(&rich_text(
                record,
                "assertion_layer",
                b,
            )?)
        {
            return Err(ClaimReferenceError::UnsupportedProfile);
        }
        if let Some(polarity) = claim_reference_field(record, "polarity", &mut b.base)? {
            if polarity.as_str() != Some("positive") {
                return Err(ClaimReferenceError::WitnessMismatch);
            }
        }
        rich_record_ref(record, reference, "claim_id", "claim_version", b)?;
        if rich_text(row, "source_claim_file_ref", b)? != stream_path
            || rich_number(rich_field(row, "source_claim_line", b)?, b)?
                != u64::try_from(piece.line).map_err(|_| ClaimReferenceError::Limit)?
            || rich_hex(rich_text(row, "claim_sha256", b)?, b)?
                != rich_hex(rich_text(reference, "digest", b)?, b)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        let mut declared_membership_count = 0usize;
        let archived = selected_record.ok_or(ClaimReferenceError::MissingSource)?;
        for declared_ref in rich_array(archived, "membership_claim_refs", b)? {
            b.base.work(id.len(), 8)?;
            if declared_ref.as_str() == Some(id) {
                declared_membership_count = declared_membership_count
                    .checked_add(1)
                    .ok_or(ClaimReferenceError::Limit)?;
            }
        }
        if declared_membership_count != 1 {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        let member = rich_text(record, "object", b)?;
        for j in 0..i {
            b.base.work(
                id.len()
                    .checked_add(member.len())
                    .ok_or(ClaimReferenceError::Limit)?,
                8,
            )?;
            if seen_ids[j] == Some(id) || seen_members[j] == Some(member) {
                return Err(ClaimReferenceError::AmbiguousIdentity);
            }
        }
        let mut member_count = 0usize;
        for declared_member in declared {
            b.base.work(member.len(), 8)?;
            if declared_member.as_str() == Some(member) {
                member_count = member_count
                    .checked_add(1)
                    .ok_or(ClaimReferenceError::Limit)?;
            }
        }
        if member_count != 1 {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
        seen_ids[i] = Some(id);
        seen_members[i] = Some(member);
        let d = rich_declared_record(inputs, member, b)?;
        let work = rich_document(&batch, d.path, b)?;
        let (catalog, _) = rich_catalog(
            &batch,
            "ToS/source-witnesses/catalog/works.jsonl",
            member,
            "record_id",
            b,
        )?;
        if rich_text(work, "record_id", b)? != member
            || rich_text(work, "record_type", b)? != "work"
            || rich_text(catalog, "source_record_ref", b)? != d.path
            || b.digest(work)? != rich_hex(rich_text(catalog, "record_sha256", b)?, b)?
        {
            return Err(ClaimReferenceError::WitnessMismatch);
        }
    }
    // Legacy current membership is an adapter scope, NOT absence of a history file.
    // No fabricated predecessor or correction-chain evidence is constructed.
    Ok(())
}
fn rich_evidence_declarations<'a>(
    claim: &'a Value,
    files: &'a BTreeMap<String, Vec<u8>>,
    b: &mut RichClaimBudget<'_>,
) -> ClaimReferenceResult<Vec<DeclaredRepoEvidence<'a>>> {
    let mut count = 0usize;
    for key in ["evidence_refs", "counterevidence_refs"] {
        b.base.work(key.len(), 16)?;
        if let Some(a) = claim.get(key).and_then(Value::as_array) {
            count = count
                .checked_add(a.len())
                .ok_or(ClaimReferenceError::Limit)?;
        }
    }
    if count > 128 {
        return Err(ClaimReferenceError::Limit);
    }
    let bytes = count
        .checked_mul(std::mem::size_of::<DeclaredRepoEvidence<'_>>())
        .ok_or(ClaimReferenceError::Limit)?;
    b.workspace(bytes)?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| ClaimReferenceError::Limit)?;
    if out
        .capacity()
        .checked_mul(std::mem::size_of::<DeclaredRepoEvidence<'_>>())
        .is_none_or(|n| n > bytes)
    {
        return Err(ClaimReferenceError::Limit);
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(a) = claim.get(key).and_then(Value::as_array) {
            for value in a {
                let path = value.as_str().ok_or(ClaimReferenceError::InvalidInput)?;
                b.base.work(path.len(), 16)?;
                for old in &out {
                    let old: &DeclaredRepoEvidence<'_> = old;
                    b.base.work(path.len(), 8)?;
                    if old.reference == path {
                        return Err(ClaimReferenceError::AmbiguousIdentity);
                    }
                }
                let raw = rich_selected(files, path, b)?;
                out.push(DeclaredRepoEvidence {
                    reference: path,
                    path,
                    raw,
                });
            }
        }
    }
    Ok(out)
}

// Reviewed original INPUT declarations; no response fields or SUT output.
const RICH_ORIGINAL_INPUT_SHA: &str =
    "b07b7a3e0ecb22df20ecfbc1afdb4bf727461115899ebf62e42a3b8bf65813e8";
#[rustfmt::skip]
const RICH_ORIGINAL_INPUT: &str = r####"{
  "authored_input_commit": "c2a32e9a2ea9d85d7d27e0a5964fdd5e6c807bca",
  "current_source_pin": "05b65a4d7a3b1cf172324d234ce29518ebca32b6",
  "full_relevant_original_rows": {
    "entity_type_registry": [
      {
        "abstract": false,
        "definition": "A persistent person-like responsibility bearer. Authorship, translation and editing are expressed through typed responsibility relations.",
        "labels": {
          "default": "Agent",
          "en": "Agent",
          "ru": "Действующее лицо"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/contracts/corpus-record.schema.json",
        "parent_type_ids": [
          "tos.entity.identity"
        ],
        "source_mappings": [
          {
            "source_graph": "source-navigation",
            "source_kind_id": "agent"
          },
          {
            "source_graph": "source-claims",
            "source_kind_id": "agent"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.agent"
      },
      {
        "abstract": false,
        "definition": "An aggregate publication or container with evidence-bearing membership claims.",
        "labels": {
          "default": "Collection",
          "en": "Collection",
          "ru": "Сборник"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.manifestation"
        ],
        "source_mappings": [
          {
            "source_graph": "source-navigation",
            "source_kind_id": "collection"
          },
          {
            "source_graph": "source-claims",
            "source_kind_id": "collection"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.collection"
      },
      {
        "abstract": false,
        "definition": "A scoped ordering of Works linked by exact contains_work Claim versions in an exact Collection version. Membership, coverage and historical chronology retain their own evidence; alternative orderings remain independent Claims.",
        "labels": {
          "default": "Attributed Collection member order",
          "en": "Attributed Collection member order",
          "ru": "Приписанный порядок членов коллекции"
        },
        "lifecycle": "stable",
        "object_role": "literal",
        "owner_ref": "ToS/contracts/scoped-member-structure.schema.json",
        "parent_type_ids": [
          "tos.entity.literal"
        ],
        "source_mappings": [
          {
            "source_graph": "source-claims",
            "source_kind_id": "collection-member-order"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.collection-member-order"
      },
      {
        "abstract": true,
        "definition": "An object whose ToS identity persists while versioned descriptions and assertions may change.",
        "labels": {
          "default": "Persistent identity",
          "en": "Persistent identity",
          "ru": "Устойчивая идентичность"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.thing"
        ],
        "source_mappings": [],
        "supersedes_type_id": null,
        "type_id": "tos.entity.identity"
      },
      {
        "abstract": false,
        "definition": "The complete, unexecuted merge or split plan inside one exact Claim. Execution and reference migration require a separately authorized transition.",
        "labels": {
          "default": "Identity transition proposal",
          "en": "Identity transition proposal",
          "ru": "Предложение перехода идентичности"
        },
        "lifecycle": "stable",
        "object_role": "literal",
        "owner_ref": "ToS/doctrine/semantic-interchange/README.md",
        "parent_type_ids": [
          "tos.entity.literal"
        ],
        "source_mappings": [
          {
            "source_graph": "source-claims",
            "source_kind_id": "identity-transition-proposal"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.identity-transition-proposal"
      },
      {
        "abstract": true,
        "definition": "The family of persistent intellectual creations, communicative documents, realizations and aggregates, with the bibliographic ladder as one declared profile.",
        "labels": {
          "default": "Intellectual object",
          "en": "Intellectual object",
          "ru": "Интеллектуальный объект"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.identity"
        ],
        "source_mappings": [],
        "supersedes_type_id": null,
        "type_id": "tos.entity.intellectual-object"
      },
      {
        "abstract": false,
        "definition": "A value whose scope and reference are owned by its Claim.",
        "labels": {
          "default": "Claim-scoped literal",
          "en": "Claim-scoped literal",
          "ru": "Литерал утверждения"
        },
        "lifecycle": "stable",
        "object_role": "literal",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.thing"
        ],
        "source_mappings": [
          {
            "source_graph": "source-claims",
            "source_kind_id": "literal"
          },
          {
            "source_graph": "source-navigation",
            "source_kind_id": "literal"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.literal"
      },
      {
        "abstract": true,
        "definition": "Root for edition and collection identities that embody or aggregate Expressions and Works.",
        "labels": {
          "default": "Manifestation",
          "en": "Manifestation",
          "ru": "Публикационное воплощение"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.intellectual-object"
        ],
        "source_mappings": [],
        "supersedes_type_id": null,
        "type_id": "tos.entity.manifestation"
      },
      {
        "abstract": true,
        "definition": "The family of authored ToS node classes, with each instance retaining its declared candidate or canonical status.",
        "labels": {
          "default": "Authored semantic object",
          "en": "Authored semantic object",
          "ru": "Авторский смысловой объект"
        },
        "lifecycle": "stable",
        "object_role": "semantic",
        "owner_ref": "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "parent_type_ids": [
          "tos.entity.thing"
        ],
        "source_mappings": [],
        "supersedes_type_id": null,
        "type_id": "tos.entity.semantic-object"
      },
      {
        "abstract": true,
        "definition": "The root for declared ToS entity families in the semantic interchange hierarchy.",
        "labels": {
          "default": "Thing",
          "en": "Thing",
          "ru": "Объект"
        },
        "lifecycle": "stable",
        "object_role": "root",
        "owner_ref": "ToS/doctrine/semantic-interchange/README.md",
        "parent_type_ids": [],
        "source_mappings": [],
        "supersedes_type_id": null,
        "type_id": "tos.entity.thing"
      },
      {
        "abstract": false,
        "definition": "An abstract intellectual or artistic creation with separately identified realizations, editions, acquired items and files.",
        "labels": {
          "default": "Work",
          "en": "Work",
          "ru": "Произведение"
        },
        "lifecycle": "stable",
        "object_role": "identity",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_type_ids": [
          "tos.entity.intellectual-object"
        ],
        "source_mappings": [
          {
            "source_graph": "source-navigation",
            "source_kind_id": "work"
          },
          {
            "source_graph": "source-claims",
            "source_kind_id": "work"
          },
          {
            "source_graph": "philosophy",
            "source_kind_id": "work"
          }
        ],
        "supersedes_type_id": null,
        "type_id": "tos.entity.work"
      }
    ],
    "relation_type_registry": [
      {
        "abstract": true,
        "assertion_mode": "reified-claim",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "Root for evidence-bearing Work, Expression, Edition, Collection, Item and File topology predicates.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.thing"
        ],
        "evidence_required": true,
        "inverse_labels": null,
        "inverse_relation_type_id": null,
        "labels": {
          "default": "bibliographically related to",
          "en": "bibliographically related to",
          "ru": "библиографически связано с"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_relation_type_ids": [
          "tos.relation.related"
        ],
        "range_type_ids": [
          "tos.entity.thing"
        ],
        "relation_type_id": "tos.relation.bibliographic-topology",
        "review_requirement": "recorded",
        "source_mappings": [],
        "supersedes_relation_type_id": null,
        "transitive": false
      },
      {
        "abstract": false,
        "assertion_mode": "reified-claim",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "Attributes order to a scoped subset of Works bound to an exact Collection version and exact contains_work Claim versions. Membership, chronology and later revisions retain their own evidence and identity bindings.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.collection"
        ],
        "evidence_required": true,
        "inverse_labels": {
          "default": "Member order attributed to",
          "en": "Member order attributed to",
          "ru": "Порядок членов приписан"
        },
        "inverse_relation_type_id": null,
        "labels": {
          "default": "Has attributed member order",
          "en": "Has attributed member order",
          "ru": "Имеет приписанный порядок членов"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/contracts/source-member-structure-claim.schema.json",
        "parent_relation_type_ids": [],
        "range_type_ids": [
          "tos.entity.collection-member-order"
        ],
        "relation_type_id": "tos.relation.collection-member-order",
        "review_requirement": "recorded",
        "source_claim_profile": {
          "assertion_layers": [
            "scholarly_report",
            "textual_observation",
            "bibliographic_assertion"
          ],
          "object_reference_set": {
            "basis_adapter": "collection-membership-versions-v1",
            "max_items": 128,
            "member_type_ids": [
              "tos.entity.work"
            ],
            "min_items": 1,
            "structure_adapter": "scoped-members-v1",
            "subject_is_member": false
          },
          "profile_version": 1,
          "reader": "structured-reference-value-v1",
          "schemas": [
            {
              "schema_dependencies": [
                "ToS/contracts/source-structured-value.schema.json",
                "ToS/contracts/corpus-record.schema.json",
                "ToS/contracts/scoped-member-structure.schema.json",
                "ToS/contracts/knowledge-assessment.schema.json"
              ],
              "schema_ref": "ToS/contracts/source-member-structure-claim.schema.json",
              "schema_version": "tos_source_member_structure_claim_v1"
            }
          ],
          "value_kind": "collection-member-order"
        },
        "source_mappings": [
          {
            "scope": "claim-predicate",
            "source_graph": "source-claims",
            "source_predicate_id": "collection_member_order"
          }
        ],
        "supersedes_relation_type_id": null,
        "transitive": false
      },
      {
        "abstract": false,
        "assertion_mode": "reified-claim",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "Relates a Collection to a member Work through an evidence-bearing membership claim.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.collection"
        ],
        "evidence_required": true,
        "inverse_labels": {
          "default": "is contained in collection",
          "en": "is contained in collection",
          "ru": "содержится в сборнике"
        },
        "inverse_relation_type_id": null,
        "labels": {
          "default": "contains work",
          "en": "contains work",
          "ru": "содержит произведение"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/doctrine/CORPUS_FOUNDATION.md",
        "parent_relation_type_ids": [
          "tos.relation.bibliographic-topology"
        ],
        "range_type_ids": [
          "tos.entity.work"
        ],
        "relation_type_id": "tos.relation.contains-work",
        "review_requirement": "recorded",
        "source_claim_profile": {
          "assertion_layers": [
            "scholarly_report",
            "bibliographic_assertion"
          ],
          "profile_version": 1,
          "reader": "identity-relation-v1",
          "schemas": [
            {
              "schema_dependencies": [],
              "schema_ref": "ToS/contracts/source-relation-claim.schema.json",
              "schema_version": "tos_source_relation_claim_v1"
            }
          ]
        },
        "source_mappings": [
          {
            "scope": "claim-predicate",
            "source_graph": "source-claims",
            "source_predicate_id": "contains_work"
          }
        ],
        "supersedes_relation_type_id": null,
        "transitive": false
      },
      {
        "abstract": false,
        "assertion_mode": "reified-claim",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "Links a predecessor to a complete source-bound merge or split proposal over the exact concrete source identities supported by the v1 profile. The proposal preserves independently readable participants and awaits its separately authorized transition.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.identity"
        ],
        "evidence_required": true,
        "inverse_labels": {
          "default": "proposes an identity transition for",
          "en": "proposes an identity transition for",
          "ru": "предлагает переход идентичности для"
        },
        "inverse_relation_type_id": null,
        "labels": {
          "default": "has an identity transition proposal",
          "en": "has an identity transition proposal",
          "ru": "имеет предложение перехода идентичности"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/doctrine/semantic-interchange/README.md",
        "parent_relation_type_ids": [
          "tos.relation.related"
        ],
        "range_type_ids": [
          "tos.entity.identity-transition-proposal"
        ],
        "relation_type_id": "tos.relation.identity-transition-proposal",
        "review_requirement": "recorded",
        "source_claim_profile": {
          "assertion_layers": [
            "identity_assertion"
          ],
          "profile_version": 1,
          "reader": "identity-transition-v1",
          "schemas": [
            {
              "schema_dependencies": [
                "ToS/contracts/source-structured-value.schema.json",
                "ToS/contracts/corpus-record.schema.json"
              ],
              "schema_ref": "ToS/contracts/source-identity-transition-claim.schema.json",
              "schema_version": "tos_source_identity_transition_claim_v1"
            }
          ],
          "value_kind": "identity-transition-proposal"
        },
        "source_mappings": [
          {
            "scope": "claim-predicate",
            "source_graph": "source-claims",
            "source_predicate_id": "identity_transition_proposal"
          }
        ],
        "supersedes_relation_type_id": null,
        "transitive": false
      },
      {
        "abstract": true,
        "assertion_mode": "direct",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "The root of the semantic interchange relation hierarchy, retaining each mapped source-native predicate.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.thing"
        ],
        "evidence_required": false,
        "inverse_labels": null,
        "inverse_relation_type_id": null,
        "labels": {
          "default": "related to",
          "en": "related to",
          "ru": "связано с"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/doctrine/semantic-interchange/README.md",
        "parent_relation_type_ids": [],
        "range_type_ids": [
          "tos.entity.thing"
        ],
        "relation_type_id": "tos.relation.related",
        "review_requirement": "none",
        "source_mappings": [],
        "supersedes_relation_type_id": null,
        "transitive": false
      },
      {
        "abstract": false,
        "assertion_mode": "reified-claim",
        "cardinality": {
          "per_object_max": null,
          "per_object_min": null,
          "per_subject_max": null,
          "per_subject_min": null
        },
        "definition": "Links a predecessor to a complete, unexecuted merge or split proposal over exact source identities or explicitly opted-in semantic subject metadata. Each participant retains its identity and readable references. The concrete versioned source profile defines eligible subjects and the separately authorized execution route.",
        "directionality": "directed",
        "domain_type_ids": [
          "tos.entity.identity",
          "tos.entity.semantic-object"
        ],
        "evidence_required": true,
        "inverse_labels": {
          "default": "proposes an identity transition for this subject",
          "en": "proposes an identity transition for this subject",
          "ru": "предлагает переход идентичности этого предмета"
        },
        "inverse_relation_type_id": null,
        "labels": {
          "default": "has an exact subject identity proposal",
          "en": "has an exact subject identity proposal",
          "ru": "имеет предложение перехода идентичности предмета"
        },
        "lifecycle": "stable",
        "owner_ref": "ToS/doctrine/semantic-interchange/README.md",
        "parent_relation_type_ids": [
          "tos.relation.related"
        ],
        "range_type_ids": [
          "tos.entity.identity-transition-proposal"
        ],
        "relation_type_id": "tos.relation.subject-identity-transition-proposal",
        "review_requirement": "recorded",
        "source_claim_profile": {
          "assertion_layers": [
            "identity_assertion"
          ],
          "profile_version": 1,
          "reader": "identity-transition-v2",
          "schemas": [
            {
              "schema_dependencies": [
                "ToS/contracts/source-structured-value.schema.json",
                "ToS/contracts/corpus-record.schema.json"
              ],
              "schema_ref": "ToS/contracts/subject-identity-transition-claim.schema.json",
              "schema_version": "tos_subject_identity_transition_claim_v1"
            }
          ],
          "value_kind": "identity-transition-proposal"
        },
        "source_mappings": [
          {
            "scope": "claim-predicate",
            "source_graph": "source-claims",
            "source_predicate_id": "subject_identity_transition_proposal"
          }
        ],
        "supersedes_relation_type_id": null,
        "transitive": false
      }
    ]
  },
  "original_input_witness_bindings": [
    {
      "configuration": {
        "allowed_evidence_refs": [
          "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
        ],
        "allowed_fields": [
          "qualifiers"
        ],
        "allowed_form_field_ids": [
          "claim.statement",
          "claim.name",
          "claim.caption",
          "claim.hover"
        ],
        "allowed_form_ids": [
          "tos.form.nietzsche-letter-705.addressee.statement-ru",
          "tos.form.nietzsche-letter-705.addressee-name",
          "tos.form.nietzsche-letter-705.addressee-caption",
          "tos.form.nietzsche-letter-705.addressee-hover"
        ],
        "allowed_operations": [
          "claim.revise"
        ],
        "authority_ref": "synthetic:claim-revision-conformance-no-admission",
        "claim_id": "tos.claim.nietzsche-letter-705.addressee",
        "expires_at": "2099-01-01T00:00:00Z",
        "principal_id": "agent:cmd2-claim-oracle",
        "schema_version": "tos_local_claim_revision_owner_v1",
        "source_path": "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-revision-initial",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-revision-initial.provenance.json",
      "provenance_raw": {
        "bytes": 62342,
        "sha256": "a797b7071e9e56621b5dc1f206fd811e5b18ed67ca62ee9fc05153b676ca01c4"
      },
      "request": {
        "fields": {
          "qualifiers": {
            "statement": "В eKGWB адресатом письма Ницше № 705 указан Constantin Georg Naumann. Это идентификация адресата по сохранённой заметке о чтении источника, а не утверждение о получении или чтении письма; человек не отождествляется с одноимённой фирмой. [command fixture successor]"
          }
        },
        "forms": [
          {
            "field_id": "claim.statement",
            "form_id": "tos.form.nietzsche-letter-705.addressee.statement-ru"
          },
          {
            "field_id": "claim.name",
            "form_id": "tos.form.nietzsche-letter-705.addressee-name"
          },
          {
            "field_id": "claim.caption",
            "form_id": "tos.form.nietzsche-letter-705.addressee-caption"
          },
          {
            "field_id": "claim.hover",
            "form_id": "tos.form.nietzsche-letter-705.addressee-hover"
          }
        ],
        "operation": "prepare-revise",
        "reason": "Synthetic source-copy Claim successor conformance; no assessment.",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 194,
        "total_bytes": 2325795
      }
    },
    {
      "configuration": {
        "allowed_claim_ids": [
          "tos.claim.nietzsche-letter-705.addressee"
        ],
        "allowed_evidence_refs": [
          "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
        ],
        "allowed_object_refs": [
          "tos.agent.constantin-georg-naumann"
        ],
        "allowed_operations": [
          "claims.create"
        ],
        "allowed_predicates": [
          "correspondence_addressee"
        ],
        "allowed_subject_refs": [
          "tos.letter.nietzsche-naumann-1886-705"
        ],
        "authority_ref": "synthetic-test-only:claim-creation-not-assessment",
        "expires_at": "2099-01-01T00:00:00Z",
        "maker_type": "model",
        "principal_id": "model:codex",
        "provenance_event_id": "tos.event.source-creation.nietzsche-letter-705-addressee.2026-09-07",
        "schema_version": "tos_local_claim_create_owner_v1",
        "source_path": "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-creation",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-creation.provenance.json",
      "provenance_raw": {
        "bytes": 68593,
        "sha256": "2af0d018dfc089a1545d7cc24f22ca66569fe2da3ea7563b91e886636bc06146"
      },
      "request": {
        "claims": [
          {
            "assertion_layer": "bibliographic_assertion",
            "claim_id": "tos.claim.nietzsche-letter-705.addressee",
            "claim_type": "relation",
            "claim_version": 1,
            "epistemic_status": "reported",
            "evidence_refs": [
              "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
            ],
            "maker": {
              "agent_ref": "model:codex",
              "maker_type": "model"
            },
            "object": "tos.agent.constantin-georg-naumann",
            "predicate": "correspondence_addressee",
            "provenance_event_ref": "tos.event.source-creation.nietzsche-letter-705-addressee.2026-09-07",
            "qualifiers": {
              "attribution": "eKGWB BVN-1886,705: editorial correspondence identification, as reported in the retained source-reading note",
              "display_fields": {
                "caption": {
                  "language": "ru",
                  "script": "Cyrl",
                  "text": "eKGWB указывает Constantin Georg Naumann; получение и чтение письма этим не доказаны."
                },
                "hover": {
                  "language": "ru",
                  "script": "Cyrl",
                  "text": "Идентификация адресата опирается на сохранённую заметку об eKGWB. Получение и чтение не установлены этой записью; человек не отождествляется с фирмой."
                },
                "name": {
                  "language": "ru",
                  "script": "Cyrl",
                  "text": "Атрибуция адресата письма № 705"
                },
                "schema_version": "tos_claim_display_fields_v1"
              },
              "does_not_establish": [
                "delivery",
                "actual_reading",
                "agreement",
                "causal_influence",
                "person_company_equivalence"
              ],
              "evidence_posture": "Retained source-reading note and public locator; no new independent source capture or reviewer admission.",
              "provenance_scope": "Serialization only; prior source reading is not independently execution-attested.",
              "source_url": "https://www.nietzschesource.org/texts/eKGWB/BVN-1886",
              "statement": "В eKGWB адресатом письма Ницше № 705 указан Constantin Georg Naumann. Это идентификация адресата по сохранённой заметке о чтении источника, а не утверждение о получении или чтении письма; человек не отождествляется с одноимённой фирмой.",
              "statement_language": "ru",
              "statement_script": null
            },
            "review_status": "unreviewed",
            "schema_version": "tos_source_relation_claim_v1",
            "subject_ref": "tos.letter.nietzsche-naumann-1886-705",
            "visibility": "public_metadata_only"
          }
        ],
        "operation": "prepare-create",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 214,
        "total_bytes": 3561968
      }
    },
    {
      "configuration": {
        "allowed_evidence_refs": [
          "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
        ],
        "allowed_fields": [
          "qualifiers"
        ],
        "allowed_form_field_ids": [
          "claim.statement",
          "claim.name",
          "claim.caption",
          "claim.hover"
        ],
        "allowed_form_ids": [
          "tos.form.nietzsche-letter-705.addressee.statement-ru",
          "tos.form.nietzsche-letter-705.addressee-name",
          "tos.form.nietzsche-letter-705.addressee-caption",
          "tos.form.nietzsche-letter-705.addressee-hover"
        ],
        "allowed_operations": [
          "claim.revise"
        ],
        "authority_ref": "synthetic:claim-revision-conformance-no-admission",
        "claim_id": "tos.claim.nietzsche-letter-705.addressee",
        "expires_at": "2099-01-01T00:00:00Z",
        "principal_id": "agent:cmd2-claim-oracle",
        "schema_version": "tos_local_claim_revision_owner_v1",
        "source_path": "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-revision-selected-history",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-revision-selected-history.provenance.json",
      "provenance_raw": {
        "bytes": 63289,
        "sha256": "2a494a37590cc6e2c0bc4c33c4a148d533d8a7291dfbff7dbc8dc8130170e538"
      },
      "request": {
        "fields": {
          "qualifiers": {
            "statement": "В eKGWB адресатом письма Ницше № 705 указан Constantin Georg Naumann. Это идентификация адресата по сохранённой заметке о чтении источника, а не утверждение о получении или чтении письма; человек не отождествляется с одноимённой фирмой. [retained correction; no assessment]"
          }
        },
        "forms": [
          {
            "field_id": "claim.statement",
            "form_id": "tos.form.nietzsche-letter-705.addressee.statement-ru"
          }
        ],
        "operation": "prepare-revise",
        "reason": "Retained source-copy correction in an isolated Claim corpus; no assessment.",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 199,
        "total_bytes": 2345870
      }
    },
    {
      "configuration": {
        "allowed_evidence_refs": [
          "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
        ],
        "allowed_fields": [
          "qualifiers"
        ],
        "allowed_form_field_ids": [
          "claim.statement"
        ],
        "allowed_form_ids": [
          "tos.form.synthetic-identity-v1"
        ],
        "allowed_object_refs": [
          "tos.agent.constantin-georg-naumann",
          "tos.agent.command-identity-second",
          "tos.agent.command-identity-third"
        ],
        "allowed_object_values": [
          {
            "counterreading": "The three endpoints may remain distinct.",
            "grounds": "Synthetic exact metadata versions; no identity admission.",
            "kind": "identity-transition-proposal",
            "mapping": [
              {
                "predecessor": "tos.agent.constantin-georg-naumann",
                "successor": "tos.agent.command-identity-second"
              },
              {
                "predecessor": "tos.agent.constantin-georg-naumann",
                "successor": "tos.agent.command-identity-third"
              }
            ],
            "members": [
              "tos.agent.constantin-georg-naumann",
              "tos.agent.command-identity-second",
              "tos.agent.command-identity-third"
            ],
            "operation": "split",
            "predecessors": [
              {
                "digest": "sha256:71052c1c44bb57a2ad04c40cbfe26ffbd14ed12921c577d9c234043b807be83b",
                "id": "tos.agent.constantin-georg-naumann",
                "version": 1
              }
            ],
            "scope": "Proposal for source metadata comparison only.",
            "source_wording": {
              "language": "en",
              "script": "Latn",
              "text": "Synthetic source wording proposes one provisional Agent identity splitting into two; no transition is performed."
            },
            "successors": [
              {
                "digest": "sha256:b4bb861b238411e2c28aac7799c0b6f4250a1d61ce8fd7e4658454346407d8b3",
                "id": "tos.agent.command-identity-second",
                "version": 1
              },
              {
                "digest": "sha256:d2c9839039deb8977372bffb63e3c0d231f59eaaec62bcda4102cc1ba2b59e73",
                "id": "tos.agent.command-identity-third",
                "version": 1
              }
            ],
            "supersedes_proposal": null,
            "unresolved_links": []
          }
        ],
        "allowed_operations": [
          "claim.revise"
        ],
        "allowed_related_claim_refs": [],
        "authority_ref": "synthetic-test-only:identity-correction-no-admission",
        "claim_id": "tos.claim.synthetic-identity-v1",
        "expires_at": "2099-01-01T00:00:00Z",
        "principal_id": "model:codex",
        "schema_version": "tos_local_identity_proposal_revision_owner_v1",
        "source_path": "ToS/source-witnesses/relations/command-identity-v1/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-revision-identity-history-v1",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-revision-identity-history-v1.provenance.json",
      "provenance_raw": {
        "bytes": 67491,
        "sha256": "788a697f8e4117fc4bf55c1b67554ae467422a5c5ef2243bd03b20aa6dc2de18"
      },
      "request": {
        "fields": {
          "qualifiers": {
            "statement": "The selected metadata supports only a proposed split. [retained identity wording correction; no admission]"
          }
        },
        "forms": [
          {
            "field_id": "claim.statement",
            "form_id": "tos.form.synthetic-identity-v1"
          }
        ],
        "operation": "prepare-revise",
        "reason": "Retain an isolated identity proposal wording correction.",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 213,
        "total_bytes": 2368220
      }
    },
    {
      "configuration": {
        "allowed_evidence_refs": [
          "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md"
        ],
        "allowed_fields": [
          "qualifiers"
        ],
        "allowed_form_field_ids": [
          "claim.statement"
        ],
        "allowed_form_ids": [
          "tos.form.synthetic-identity-v2"
        ],
        "allowed_object_refs": [
          "tos.agent.constantin-georg-naumann",
          "tos.agent.command-identity-second",
          "tos.agent.command-identity-third"
        ],
        "allowed_object_values": [
          {
            "counterreading": "The three endpoints may remain distinct.",
            "grounds": "Synthetic exact metadata versions; no identity admission.",
            "kind": "identity-transition-proposal",
            "mapping": [
              {
                "predecessor": "tos.agent.constantin-georg-naumann",
                "successor": "tos.agent.command-identity-second"
              },
              {
                "predecessor": "tos.agent.constantin-georg-naumann",
                "successor": "tos.agent.command-identity-third"
              }
            ],
            "members": [
              "tos.agent.constantin-georg-naumann",
              "tos.agent.command-identity-second",
              "tos.agent.command-identity-third"
            ],
            "operation": "split",
            "predecessors": [
              {
                "digest": "sha256:71052c1c44bb57a2ad04c40cbfe26ffbd14ed12921c577d9c234043b807be83b",
                "id": "tos.agent.constantin-georg-naumann",
                "version": 1
              }
            ],
            "scope": "Proposal for source metadata comparison only.",
            "source_wording": {
              "language": "en",
              "script": "Latn",
              "text": "Synthetic source wording proposes one provisional Agent identity splitting into two; no transition is performed."
            },
            "successors": [
              {
                "digest": "sha256:b4bb861b238411e2c28aac7799c0b6f4250a1d61ce8fd7e4658454346407d8b3",
                "id": "tos.agent.command-identity-second",
                "version": 1
              },
              {
                "digest": "sha256:d2c9839039deb8977372bffb63e3c0d231f59eaaec62bcda4102cc1ba2b59e73",
                "id": "tos.agent.command-identity-third",
                "version": 1
              }
            ],
            "supersedes_proposal": null,
            "unresolved_links": []
          }
        ],
        "allowed_operations": [
          "claim.revise"
        ],
        "allowed_related_claim_refs": [],
        "authority_ref": "synthetic-test-only:identity-correction-no-admission",
        "claim_id": "tos.claim.synthetic-identity-v2",
        "expires_at": "2099-01-01T00:00:00Z",
        "principal_id": "model:codex",
        "schema_version": "tos_local_identity_proposal_revision_owner_v2",
        "source_path": "ToS/source-witnesses/relations/command-identity-v2/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-revision-identity-history-v2",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-revision-identity-history-v2.provenance.json",
      "provenance_raw": {
        "bytes": 67491,
        "sha256": "f96e9e6c297bbcaf90eb660bd60d756eb378b30235212078ca497643e44e05df"
      },
      "request": {
        "fields": {
          "qualifiers": {
            "statement": "The selected metadata supports only a proposed split. [retained identity wording correction; no admission]"
          }
        },
        "forms": [
          {
            "field_id": "claim.statement",
            "form_id": "tos.form.synthetic-identity-v2"
          }
        ],
        "operation": "prepare-revise",
        "reason": "Retain an isolated identity proposal wording correction.",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 213,
        "total_bytes": 2368248
      }
    },
    {
      "configuration": {
        "allowed_evidence_refs": [
          "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json",
          "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/membership-claims.jsonl",
          "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json",
          "ToS/review-ledger/2026-09-10-mysl-collection-order-source-reading.md"
        ],
        "allowed_fields": [
          "qualifiers"
        ],
        "allowed_form_field_ids": [
          "claim.statement"
        ],
        "allowed_form_ids": [
          "tos.form.synthetic-collection-order"
        ],
        "allowed_object_refs": [
          "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
          "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
          "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
          "tos.work.friedrich-nietzsche.der-fall-wagner",
          "tos.work.friedrich-nietzsche.goetzen-daemmerung",
          "tos.work.friedrich-nietzsche.der-antichrist",
          "tos.work.friedrich-nietzsche.ecce-homo"
        ],
        "allowed_object_values": [
          {
            "collection_version": {
              "digest": "sha256:f14299bd238043c88f48b33e7116a12724d0e1285d5bb0861c69c78e1e3ace80",
              "id": "tos.collection.friedrich-nietzsche.works-in-two-volumes-volume-2-mysl-1996",
              "version": 4
            },
            "coverage": "exhaustive-within-scope",
            "kind": "collection-member-order",
            "limitations": "Metadata-map reading only; all map member boundaries are inferred and unreviewed. No fresh PDF inspection, source-text or translation admission, rights clearance, publication permission or canon. Non-member apparatus remains separate.",
            "members": [
              "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
              "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
              "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
              "tos.work.friedrich-nietzsche.der-fall-wagner",
              "tos.work.friedrich-nietzsche.goetzen-daemmerung",
              "tos.work.friedrich-nietzsche.der-antichrist",
              "tos.work.friedrich-nietzsche.ecce-homo"
            ],
            "membership_basis": "Existing exact Collection metadata and seven contains_work Claim versions named by the retained map; this ordering does not create membership.",
            "membership_versions": [
              {
                "digest": "sha256:ae67847fbdefc874dda30bd1a920d0ea5db8f69bd7559ed28a29bfe92a54a234",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-also-sprach-zarathustra",
                "version": 2
              },
              {
                "digest": "sha256:2812ac81509f8b331129692ef9ca33bb9f9345fe49c955dc6c244778d2ba936d",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-jenseits-von-gut-und-boese",
                "version": 1
              },
              {
                "digest": "sha256:ed1168f3ead31e119194a4f34d7fcc31f70282bbe98ef8b45dff0aad8a5a04b5",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-zur-genealogie-der-moral",
                "version": 1
              },
              {
                "digest": "sha256:1e09d881a035a40decda459fdab2c61ab26439b7bd3c27fbc016b3de1d181425",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-der-fall-wagner",
                "version": 1
              },
              {
                "digest": "sha256:7ba572d59194ad01ff7e83810a2e74807e3fa7a3c0dbec8cf1682a08213104fb",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-goetzen-daemmerung",
                "version": 1
              },
              {
                "digest": "sha256:d8b9711d2c0b4dd3e8e6c6da23839e846cbd218e7740f845679eb45c0be2ecec",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-der-antichrist",
                "version": 1
              },
              {
                "digest": "sha256:2a751df1878e4123c28927ff49f45f6135b7c6656874b31edeb450e6e6621925",
                "id": "tos.claim.collection.mysl-1996-volume-2.contains-ecce-homo",
                "version": 1
              }
            ],
            "ordering": {
              "basis": "Explicit map sequence 1–7 and reported container-page progression; editorial order, not composition, publication or historical chronology.",
              "mode": "total",
              "precedes": [
                [
                  "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
                  "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese"
                ],
                [
                  "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
                  "tos.work.friedrich-nietzsche.zur-genealogie-der-moral"
                ],
                [
                  "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
                  "tos.work.friedrich-nietzsche.der-fall-wagner"
                ],
                [
                  "tos.work.friedrich-nietzsche.der-fall-wagner",
                  "tos.work.friedrich-nietzsche.goetzen-daemmerung"
                ],
                [
                  "tos.work.friedrich-nietzsche.goetzen-daemmerung",
                  "tos.work.friedrich-nietzsche.der-antichrist"
                ],
                [
                  "tos.work.friedrich-nietzsche.der-antichrist",
                  "tos.work.friedrich-nietzsche.ecce-homo"
                ]
              ]
            },
            "source_scope": "The seven Work entries of retained work-boundary map version 1 for the exact Mysl 1996 volume 2 Item/File; not all intellectual components of the publication.",
            "source_wording": {
              "language": "en",
              "script": "Latn",
              "text": "The retained unreviewed work-boundary map for Mysl 1996 volume 2 reports the sequence Also sprach Zarathustra; Jenseits von Gut und Böse; Zur Genealogie der Moral; Der Fall Wagner; Götzen-Dämmerung; Der Antichrist; Ecce homo. This is a source-attributed editorial sequence of seven already linked Works, not a new membership assertion, historical chronology or accepted PDF inspection."
            }
          }
        ],
        "allowed_operations": [
          "claim.revise"
        ],
        "authority_ref": "synthetic-test-only:collection-order-correction-no-membership-grant",
        "claim_id": "tos.claim.synthetic-collection-order",
        "expires_at": "2099-01-01T00:00:00Z",
        "principal_id": "agent:codex-tos-foundation",
        "schema_version": "tos_local_claim_revision_owner_v4",
        "source_path": "ToS/source-witnesses/relations/command-collection-order/source-claims.jsonl",
        "source_root": "<isolated-source-root>",
        "uid": 1000
      },
      "oracle_id": "claim-revision-catalog-history",
      "provenance_path": "tests/conformance/rust/legacy-python-oracles-v1/claim-revision-catalog-history.provenance.json",
      "provenance_raw": {
        "bytes": 82440,
        "sha256": "edda030c6ded5e9a4738ceea9b32dd7bad20ebe8e5332464210a33695c609d20"
      },
      "request": {
        "fields": {
          "qualifiers": {
            "statement": "The retained unreviewed work-boundary map for Mysl 1996 volume 2 reports the sequence Also sprach Zarathustra; Jenseits von Gut und Böse; Zur Genealogie der Moral; Der Fall Wagner; Götzen-Dämmerung; Der Antichrist; Ecce homo. This is a source-attributed editorial sequence of seven already linked Works, not a new membership assertion, historical chronology or accepted PDF inspection. [retained order wording correction; no membership grant]"
          }
        },
        "forms": [
          {
            "field_id": "claim.statement",
            "form_id": "tos.form.synthetic-collection-order"
          }
        ],
        "operation": "prepare-revise",
        "reason": "Retain an isolated attributed order wording correction.",
        "schema_version": "tos_local_source_command_v1"
      },
      "selected_source_commitment": {
        "entry_count": 248,
        "total_bytes": 2609248
      }
    }
  ],
  "registry_original_raw": [
    {
      "path": "ToS/doctrine/semantic-interchange/entity-types.v1.json",
      "raw": {
        "bytes": 264272,
        "sha256": "cad0331eea790d8d63ab25c5263b81da3dcb4366f5416a94d88b7b9fbbbce465"
      }
    },
    {
      "path": "ToS/doctrine/semantic-interchange/relation-types.v1.json",
      "raw": {
        "bytes": 335626,
        "sha256": "462d75fe3a22567e88c6c0f14b40fae5a83db244333c66acd406e9dca9c5ab4c"
      }
    }
  ],
  "schema": "pr267_B_rich_original_INPUT_projection_candidate_v1",
  "schema_raw_closure": {
    "ToS/contracts/claim-packet.schema.json": {
      "bytes": 5632,
      "sha256": "7daa1aa9831000a6ad56a0dfab38cfbb06cfd7a6d2cdfaa1aff264f2c33999cd"
    },
    "ToS/contracts/corpus-record.schema.json": {
      "bytes": 8744,
      "sha256": "a0947108d13950656ece81642d47ab4f1166f5c7ebb14c6ef74b097c9175c851"
    },
    "ToS/contracts/knowledge-assessment.schema.json": {
      "bytes": 5043,
      "sha256": "71cd4aa22c4b89f4077fde98b09e5baa375525162ff499066845d7af811e0298"
    },
    "ToS/contracts/scoped-member-structure.schema.json": {
      "bytes": 3509,
      "sha256": "e6ffd025d1905de1f19acbda575aa82b60050e6754b0433cd0148f1ec81ef2b3"
    },
    "ToS/contracts/source-claim-record.schema.json": {
      "bytes": 4297,
      "sha256": "fb69069171350fb7c0a7bf744201a05855a67d33e4b8ff4f9d2784dcb7eb47c7"
    },
    "ToS/contracts/source-identity-transition-claim.schema.json": {
      "bytes": 3670,
      "sha256": "4fda26cff3b9373647c090ffed3ee48b8ca81b9a25851fbc2a73647be75c7b94"
    },
    "ToS/contracts/source-member-structure-claim.schema.json": {
      "bytes": 2070,
      "sha256": "a24a423ffbdb36ea18474edd38ba267e91e98f868e72b031b334b996639ef123"
    },
    "ToS/contracts/source-relation-claim.schema.json": {
      "bytes": 604,
      "sha256": "14a7bbf923b30ced37953649cfced1b10069adb2f63266c56aea7f115318defe"
    },
    "ToS/contracts/source-structured-value.schema.json": {
      "bytes": 1289,
      "sha256": "d0392d43c8e45671894174fc09bc930fa696766d1a803494096ad04cd6f2fcf8"
    },
    "ToS/contracts/subject-identity-transition-claim.schema.json": {
      "bytes": 3736,
      "sha256": "024bb9459f2ecba2986552dfba2cd2fdef6a3a95a20e63c6e16a2174b7fbcc33"
    }
  },
  "source_scope": "Original authored INPUT rows, complete original configuration/request and raw commitments only. No semantic answer, source_bindings, preview, descriptor or normalized result was read. The full original INPUT inventories remain immutable in the six original provenance files. Counts are historical, never current-cut totals.",
  "status": "DOCUMENTARY_CANDIDATE_NO_SOURCE_OR_PROFILE_ENACTMENT"
}
"####;
#[rustfmt::skip]
const RICH_ORIGINAL_MEMBERS: &[RichOriginalMember] = &[
    RichOriginalMember { mask: 63, path: "ToS/contracts/access-request.schema.json", bytes: 9346, sha: "885c0be76e1ef44a6809945eda395e3830fd429ef81ca0eb7a9ab5712ae2bc4d" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/acquisition-batch.schema.json", bytes: 6800, sha: "43445c580399ec5aa3013254fdc04143c945666ba8c874d2c137f24e6b8799a9" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/acquisition-provenance-delta.schema.json", bytes: 2400, sha: "9a8ab57ceb87c299ecae69e8275613625d19684c30070f1c83a57eab1b4bda7a" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/artifact-source-witness-v2.schema.json", bytes: 11257, sha: "bdedd892c9cce4120dd77fb78d4bb953127919149ba2a7583b7934dec7f15215" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/artifact-source-witness.schema.json", bytes: 11046, sha: "1b5a1a49851093daf3b08182b08fc78658d05de4ca1f838bc795cc9c6fb0ce3f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/artifact-visual-representation.schema.json", bytes: 6972, sha: "2317c4671aee44730c9b4176511853a01e4729714fbfedf1fcb0c01b9cc3c506" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/authored-route-evidence-bridge-v1.schema.json", bytes: 8828, sha: "358fb2effe1393e77cbaabca5cd3da659c006bbf78870548890661b9d07545ad" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/bounded-translation-research-input.schema.json", bytes: 13020, sha: "25c93ac6f1f1c557e720742a1aa11bf961c937e92b8e2d69445404b16592a522" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/claim-display-fields.schema.json", bytes: 2445, sha: "b1cec0d2104822e74a522847eae9bc65b4ce89db61d8d339de1aed6c973a80f7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/claim-packet.schema.json", bytes: 5632, sha: "7daa1aa9831000a6ad56a0dfab38cfbb06cfd7a6d2cdfaa1aff264f2c33999cd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/collection-work-boundary-map.schema.json", bytes: 6591, sha: "edfdd60bdc77dbf75e59bc2b420912d2b62363a46c8e42f3c95a4e5db08d4e38" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/corpus-record.schema.json", bytes: 8744, sha: "a0947108d13950656ece81642d47ab4f1166f5c7ebb14c6ef74b097c9175c851" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/critical-edition-citation-witness-decision.schema.json", bytes: 11874, sha: "e203e6c10eab31ff7d6d87cae56b6accc77f31b1fd66f6a5c2b0933ea5beae70" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/critical-edition-witness-admission.schema.json", bytes: 14037, sha: "976489b0088a0d6e943781a9e00a8dfe6bc7c3da34fd6244e1686e30bb0b18e7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/document-catalogue-claim.schema.json", bytes: 7527, sha: "1e3d42da65403eaefff1da668c31843dbe8b7f0c3420cda0a82356e7de085be6" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/document-record.schema.json", bytes: 1602, sha: "f1da16a065ecf024ac7b9dc707a9b0bab7f8ea9d405954ed9ee3f8821e5e05ef" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/edition-reading-admission.schema.json", bytes: 8871, sha: "a62d3f5e9182d557c35070d9d18b32a8c1685b998c4d837feb2da34a291d8ade" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/epistemic-evidence-projection.schema.json", bytes: 4883, sha: "0baf90f7966714d4ce4141d61552416186050f97d7a9b391b077c399b8c43459" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/experimental-translation-candidate.schema.json", bytes: 9533, sha: "a9c8a1c7d6266515ad5cdbc70376f08e7adcdfa8919ebc4e69dfe6c20b34caa0" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/experimental-translation-episode.schema.json", bytes: 32739, sha: "388a5008add385eb528cea843c72250e7148cc54e88f248f392df83fa93ccc7b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/expression-derivation.schema.json", bytes: 1621, sha: "6f78998c37b54beef3822688fcd9985186c21621e8e61ef1a23b1e92abfc237e" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/first-publication-chronology.schema.json", bytes: 4580, sha: "84ad73a011a1bc951d31401d0872da17d6f031270f4a8a047b675e32bd3812e8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/formation-relation-claim.schema.json", bytes: 2468, sha: "85a67de9cae49ba5771efde9d8569b675c0aa0cbfa2f5c101316135571282ba1" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/german-assisted-source-review.schema.json", bytes: 8856, sha: "491c85ffc0837c2c1d1248ae6ca9973b1e1d888f8e43725016ef55ffa4bfd529" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/german-source-triangulation.schema.json", bytes: 16586, sha: "522e94f24eacc4404c0ca26cf506e80894b312b468921f1317dda9263aa0455f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/golden-kernel-transfer-plan.schema.json", bytes: 25666, sha: "7ef08d7840c40ffe7342c6356b1df8dd19c577a5d4eb95845bc8ab49441c81ca" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/graph-query-plan.schema.json", bytes: 3445, sha: "2ee33b77c496e732e22932e8fc24b1c0fe4087745eac434946d9580edf9e3afd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/hierarchical-numbered-unit-label-correspondence.schema.json", bytes: 6117, sha: "9173ee5c2bb9349be004ae39f57204c34840d604f96e387d04f09d9778e11e17" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/hierarchical-source-numbered-unit-page-map.schema.json", bytes: 12796, sha: "126dd8d448581b976062ff58bd2064b3de3b9d8c505b387bd2fbf90712b0d1ae" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/hierarchical-target-numbered-unit-page-map.schema.json", bytes: 11506, sha: "db229136190729a8f6648c12c3a5ed3a1c00c79b460593e73d80c8fc1a5ef877" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/historical-claim.schema.json", bytes: 10780, sha: "b3e9a2163e5b8b46f342643d7d0f84949017106fa50e942f83d2306abb887750" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/historical-context-claim.schema.json", bytes: 3187, sha: "15e5823b6b812494853076223025250afa9922636ff9509ec20129d8972f894d" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/historical-context-record.schema.json", bytes: 10879, sha: "6f025746a87ffc2a7be8d7a745bcd6cd9438fa9d04e0add43fb1a9854d44efef" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/historical-record.schema.json", bytes: 3705, sha: "252cffe61616fd258254b71f42872ee2c8b9446901bd4383e47d21bf4e1376c2" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/human-form-set.schema.json", bytes: 2486, sha: "e2d5ea3dcb3c779466adcca2940165ec8def74f0bc2dd783666e5d08bf52aff2" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/human-form-template.schema.json", bytes: 1527, sha: "558527b3c0de451a7210fe2c62503bc5ce9592bbf2d2390aba9a1b5f8ab4e91e" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/human-form.schema.json", bytes: 16342, sha: "1af83793ef571fc3b8cbc1f03f9c8ad8e54fe07a8b17c5337195f7fcb39d3980" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/intellectual-formation-record.schema.json", bytes: 4412, sha: "8228e76df5b471e334c952918bb23928f4eede10b3e57657e2456d056b61aa09" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-assessment-authority.schema.json", bytes: 2011, sha: "8ae72f8e24ffba3bec8c7f4ff0d3c56d90e9e4f33b4795aabe217df3dd5e50cd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-assessment-batch.schema.json", bytes: 3566, sha: "66144d009da2268001b356ba4ef39da09dc408e43b1edceee9c6a3c17ea924ea" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-assessment-competence.schema.json", bytes: 1586, sha: "b0f6466b7eb175b9c9b4f1e5f4df0f93c853d0ebc9acb5f3606fe052ede19cb1" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-assessment-policy.schema.json", bytes: 2094, sha: "77d4956a4d9f6f5ea8a78a338a655d39957c9cbe0a5d425bab77ca3ad301b511" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-assessment.schema.json", bytes: 5043, sha: "71cd4aa22c4b89f4077fde98b09e5baa375525162ff499066845d7af811e0298" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/knowledge-query-vocabulary.schema.json", bytes: 6064, sha: "aac63ec709e34679539d96c523bc86df3d77a8d2f4315155586e8a8fc4d466df" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/laboratory-sample-plan.schema.json", bytes: 5659, sha: "0c7019f06ee5159610d83244543cb89f6ae55f4eea3738db57c23b97619d17af" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-comparison-claim.schema.json", bytes: 3976, sha: "0720a6a93c4c9e671ac875100ba6afc0e0f34cf3aef5a4b93b43f6cacb2a9fbe" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-description-record.schema.json", bytes: 5835, sha: "268b1d163386dc408a02ad27c353e0ba46cad22c580caaf8219af9657b264adc" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-index-plan.schema.json", bytes: 10202, sha: "b7348fc25e1954a043bcc8e30579d70ee93949bbd1717bf993b4080600e1a717" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-index-projection.schema.json", bytes: 14424, sha: "80a327913f6ea61c62f303577b5799be396db506089e6fb92ed8e3fdc5ca562c" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-recurrence-plan.schema.json", bytes: 8002, sha: "524d93c61b9d28a2a7281dc97d3f31a4aa0732d442bb7beba75872cac0197982" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-recurrence-projection.schema.json", bytes: 8579, sha: "661a53ddc3153fa98b973fec87676d8f8b04614669f9e7bb707ca1b2d16f3fde" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-usage-context-plan.schema.json", bytes: 12160, sha: "41cb2c56edb259994ef073ef4d4c2965d7e0bf5ecaf313b9a416e03169b3def3" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-usage-context-receipt.schema.json", bytes: 13317, sha: "5120eb7a26263c392d9011182ac3172f42665be4ce9b1bad10c70b421ed30194" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lexical-usage-context-row.schema.json", bytes: 3050, sha: "7e21f763d5826680eb4b0dc2bb2ff5f82ecbbf88211e66eabcf4cc7992f7befb" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/linguistic-description-record.schema.json", bytes: 5764, sha: "c6cb0872541eebdb10c22b80bc15d7037e58d0070f594509167967d767b65d36" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/linguistic-relation-claim.schema.json", bytes: 1003, sha: "0ab386046f113d6c16fc6e627153e37baa38231383cd7bee51ced3f0d3cfd73e" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/lived-witness-packet.schema.json", bytes: 18822, sha: "0f9a9a3aa8f8ddc406a7a11b23dcbc3934b317a6be1201d285efa4ef0f09ce79" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/manual-error-ledger-record.schema.json", bytes: 6574, sha: "9d4dbe5455b705bba2f6f123176eb9b4afe850c47c05da30b78a24122491b162" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/manual-gold-assurance.schema.json", bytes: 14738, sha: "3bcad7751309658ba2c38f020f50558a8c6bd40179b3da31b7b50f65b9baa943" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/manual-gold-status.schema.json", bytes: 4030, sha: "26f56b8e8bbe343c0ca1cdce7357453a7ff2a3b96aaf79073b7ce0d27167b2e7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/material-discovery-record.schema.json", bytes: 12974, sha: "0e8093dc2ccb0410c455a875f746b2202e852051455ac26444ceb10aea948a07" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-census-result-receipt.schema.json", bytes: 21174, sha: "1228c378de944fb361cb0725be56d6cfeebf6f2a83564845e14802c41c003fbd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-contextual-artifact-admission.schema.json", bytes: 10977, sha: "7433b21c9a5eb9d405d54cc472c5671a2d7ce770f829ae0b8c9a9c3519907110" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-contextual-episode-plan.schema.json", bytes: 11816, sha: "80372c7521f13f7026dcb13bd203c6189366b91f6ccda6527654d9f5f2d4d396" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-contextual-episode-receipt.schema.json", bytes: 7287, sha: "16a4b86d8051af2aa62766dfcb6b59a78a6632e94c79e4a571339b5eb256bab7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-contextual-episode-row.schema.json", bytes: 2782, sha: "1a7734a4a20c05268fe8cb1a724405bf88ed138f8d936f7c775dd0dc3123af39" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-contextual-result-receipt.schema.json", bytes: 18462, sha: "328be01e9dc71f3cbe1148c78630d4fcef3dfe42e44a91d28c0fea689a684c2a" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-evaluation-plan.schema.json", bytes: 11648, sha: "7efc8eeb49380cda387387898432666b7add0188b12a9558a70916c3ab1a83d6" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/morphology-input-receipt.schema.json", bytes: 5533, sha: "fa0f95170ad727cee670df2793d2dc34a7b026bd499092a33058b355029ac25d" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-local-text-read.schema.json", bytes: 4034, sha: "fce62db90ff0224e70980b54b4123141c575f21182054f56cc0bfb7c320cb921" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-page-ocr-comparison.schema.json", bytes: 12056, sha: "e3c656b790c00be278ea36e9ebae8d8469529e63470d70a0a4b5cec75312e7a7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-private-text-read.schema.json", bytes: 3589, sha: "b8654549c2a0f4803cb69e0b2807be6ba010df05c3b58022bacaece30c3eda52" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-layer-binding.schema.json", bytes: 1885, sha: "da7fa3c3cb42ba0e83f632034daf024f826dc73926d0c09f1878c13ecc7d629d" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-layer-comparison.schema.json", bytes: 7194, sha: "80e3e14673eb1f782980700d86efa9cc3a4a183f7bd818f963d3d4498140351b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-layer-derivation-comparison.schema.json", bytes: 6984, sha: "c73f2b147da964325d244c5d3feb5c9e60bf1fe4d4d3d320ee6403443b6ac6ae" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-layer-quality-basis.schema.json", bytes: 3828, sha: "152f7b0aee8fee47603b8fc9aa283a7776b8c09f34415226523c68150d95c929" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-unit-assessment-subject.schema.json", bytes: 1781, sha: "36dcdc9b41f91360950a99f1678afef6626a620a314a38108f39353cce64e5b4" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-text-unit-binding.schema.json", bytes: 3255, sha: "957dbe7d4309068b7d4c19ccf0a4c5a65f0ec0c39a829e0aaf3e4ea6732ed3a2" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/native-translation-alignment-record-v1.schema.json", bytes: 10767, sha: "5fa732463078756c9ac511ea9c57e00c02e4ff77948465375b1271f3d30726a4" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/numbered-unit-page-map.schema.json", bytes: 8874, sha: "43ecb437771e8b13b64159630776c232a6b2c778c63b8691043c8f3c70374da0" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/object-link-claim-v2.schema.json", bytes: 1462, sha: "ff0046e2e05dd67c68a75af02429d609baae75dba8db3aa4147ac0297cd50e92" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/object-link-claim.schema.json", bytes: 2303, sha: "fbc77bdc6c756daf5f26c4474b59b3f8c79e8cfb8e8f12b12052ec0802da8de8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/occurrence-description-record.schema.json", bytes: 2418, sha: "a63e24545410c7f6da351cd14af6f219902e2f9fdcb39a2b472105b3b1cdf86a" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/ocr-visual-sample-plan.schema.json", bytes: 9423, sha: "efc5af081de74da05360644e23455527214b8e836ebc37c8aa95a6a38f729324" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/open-work-candidate-queue.schema.json", bytes: 4892, sha: "4dfae12cde9388b9f97a28d1afbf6d753d2a9d119f487c00e033700fe0fcdb36" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/open-work-candidate-receipt.schema.json", bytes: 8288, sha: "c8c7d5f1a7e88d300fa2d5853f19187a1a3d6f5208cbbb1f4f489c68edf438ed" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/open-work-candidate.schema.json", bytes: 6549, sha: "bfccc41d462838019d29a78c8868155441d980025fde13c7f8c092479dc06c8b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/open-work-channel-timing-receipt.schema.json", bytes: 2857, sha: "bed2e70d8b92844aeecdcda37ece41426711dfee1ba5bea896e8afd355a2feaf" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/open-work-readiness-plan.schema.json", bytes: 8484, sha: "12343a5e5db2579b51eec4b0479512fa2cafae7c93ae27c0413f3e3b814fced1" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/owner-local-source-context.schema.json", bytes: 1353, sha: "4261950c0f2f48001498b0bee0cbe1ff4e702af45688090e8123f080b5c17127" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/parallel-numbered-unit-label-map.schema.json", bytes: 7416, sha: "8ced68903506d323be129d84c4e40093ed153ad01f6962063d59e7e767d575e8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/parallel-witness-structure-map.schema.json", bytes: 10847, sha: "19d9847a7ee3091a6869f5907009ceab6662712c6dc3e2984e62cd5d48ba7a02" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/partitioned-projection.schema.json", bytes: 3950, sha: "55969fd4b048f93a73b6a6740ea7392ac7b475baa9520bf75352ed6be283ea3c" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/philosophy-atlas-projection.schema.json", bytes: 13662, sha: "e65ce0cdad3661bbb35a5d8d07134c587e684ef19a83dd920575974e70468373" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/philosophy-graph-projection.schema.json", bytes: 31123, sha: "49a3f59da10cf8823c45446c52c9237f4d2f96d28af4e068a8a36acac9814b19" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/philosophy-graph-views.schema.json", bytes: 9208, sha: "cd5d10561ba2c692b9766294d82ecf8b5e60a9b76428ebe8695e68bb2c43b601" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/philosophy-source-planting.schema.json", bytes: 6106, sha: "749f55fb4b54474e06259b63bf5f99f4074dcb17ba2c3110c49e54f2e09721fd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/private-laboratory-evidence-handoff.schema.json", bytes: 12373, sha: "1b65ff635b5a12c5957eab87c7de336ad68fc0017b5f9ab2b6ea83a525495fec" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/private-transfer-source-visible-review-bundle.schema.json", bytes: 8742, sha: "eecf83a707f6588412032c0dbc570744634e5eb45bb5aad7abe2c3e1116add59" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/provenance-event-v2.schema.json", bytes: 34901, sha: "bc56f527b580e3c4728fcb2ebe8cfa333b75a7db56922226d1eaec6de0557ae0" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/provenance-event.schema.json", bytes: 4253, sha: "d4f9a09f65c91c478dfc93d4b26feca70e8204b2b946c9eee6bac4c8c872d0a2" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/provision-activity.schema.json", bytes: 8607, sha: "27d53ef0f739b4e640311d625b9abd6a6cab0c21e808a9872776d1f660c0451b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/public-laboratory-evidence-derivative.schema.json", bytes: 10806, sha: "b0594f9064b132b2654722919ebea8ed6ef4aafc7a940d8c4779fe41e9fc9bdf" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/public-native-text-authority.schema.json", bytes: 3566, sha: "8ca983cc9347c044f2f1b0b58fdf70e933d73f5d54b168f3b8d7f173c4f66f55" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/public-native-text-create-owner.schema.json", bytes: 10668, sha: "a71a29e30d3279e9d20367ea665acc3d495f37b9097c28cada5dde2709df260b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/reception-record.schema.json", bytes: 7303, sha: "f7bb253ac378449b29eaa46464136cb3c75892bc735a41df9e978a38b0fb8923" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/record-version-view.schema.json", bytes: 7514, sha: "e543b1b235302ec547128564f1e0792ecbc091eec9acca8b1d789c2ea2ff3cf0" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/research-corpus-record.schema.json", bytes: 1841, sha: "a8828699d4b8ad0bc24853c68474ee4a2fa7f061a24228585eeee0ce73a06de8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/retrieval-query-plan.schema.json", bytes: 4193, sha: "bdf3b6ac181da9db91feb7358b9dbf925bdddfb46ae3978bb6e1062e0d87cbf9" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/rights-record.schema.json", bytes: 8264, sha: "a8b4e440a9cbb8053653d310e70a1f5405b621e258277d8e76ac7f6bac7d864f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/root-entry-map.schema.json", bytes: 4333, sha: "476f2135fa4b049480a7282cbe014f61f1c4072268d53f09e9dd72d5316c28f5" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/scholarly-composite-claim.schema.json", bytes: 1844, sha: "bfeb29b1e654fae5bae6bc6d4c73fb1ee3de0da3dd7120ee2d31f2573306935e" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/scholarly-composite-file-representation.schema.json", bytes: 10212, sha: "8bd1149a13cd8f330ed428a55bf5a4f8048d65423c1da8c39dd3b7230bdcc2da" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/scholarly-composite-record.schema.json", bytes: 2417, sha: "7531fe0f32247a87991e42fa8eb6f8d93efe70dda3e4d299b6c339d4dc698a81" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/scholarly-composite-witness.schema.json", bytes: 9755, sha: "d704fac5f02a747578b3ef6ddaf7f7af06c517c29c529d767ceaf5b84af25cd9" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/scoped-member-structure.schema.json", bytes: 3509, sha: "e6ffd025d1905de1f19acbda575aa82b60050e6754b0433cd0148f1ec81ef2b3" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-annotation-packet-v2.schema.json", bytes: 26114, sha: "1a3e8bf71f47c8c655c00cc9b7401cb621b5136ee52567ab3c34c2022c2b65d8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-description-record.schema.json", bytes: 4643, sha: "4fa3fda61509be3740911e7c8422ba2b68117a81d7c5abc61b5f02d1e18b271b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-entity-type-registry.schema.json", bytes: 16948, sha: "a15caf1b79abbe840cb951e80f7ddcdd1021291ceb28c52d162e47949bedac29" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-ladder-packet.schema.json", bytes: 54667, sha: "b4e06f0599ae52c0df4dbf4b13d6a3b3b68d17064957553fce392f626e3ed8b7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-relation-claim.schema.json", bytes: 1997, sha: "8c0f0c80c58c6196d197e36fab79259d09d36ff41b529dfa983af5c35a25856f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/semantic-relation-type-registry.schema.json", bytes: 19804, sha: "6b977aa01ac54172ee7d50618867edc75cefa395f22767cd4450211ca2c39894" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/server-import-contract.schema.json", bytes: 11091, sha: "817e7f1c163f250d37a30f531c1f2de1a52d23cb8f0a6b1e23f91cd39885afe1" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/server-import-receipt.schema.json", bytes: 4296, sha: "efd8946b0eba1a76503e232ed27e3fcec43e1b429e39304d9d02d4251ecadd70" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/sign-annotation.schema.json", bytes: 3924, sha: "823cd06541a8ac4dd79f21428d0351fdff3b4802b93d6621254b39f93efa4427" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/sign-description-record.schema.json", bytes: 4844, sha: "04646f22a48ff306d47513505080e5fe4c03cd52a07d968ac8bf7991af274dfe" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/social-body-record.schema.json", bytes: 4722, sha: "e95527b594aad14214b6a7b73feef54bf7fbb848e83e7d6a1925c4e79642fb9f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/social-relation-claim.schema.json", bytes: 2528, sha: "befabc307e1591c3ed0f7cbe381900fb3b849ca1628b051b8705139df982e500" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-anchor-v2.schema.json", bytes: 14126, sha: "99e60f433a8545495706216bb66dfbae241cadf44086b59100af87464f5d8235" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-anchor.schema.json", bytes: 4864, sha: "6b493f99d06aa76e3254eb2cd3f46907969a3ad43416e100333ead20f6244e8c" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-catalog-projection-v2.schema.json", bytes: 12417, sha: "56831635e4a21ad6e97bfe96855d5b32257b462627b0d6ae18f9c17bbf7d2db8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-claim-record.schema.json", bytes: 4297, sha: "fb69069171350fb7c0a7bf744201a05855a67d33e4b8ff4f9d2784dcb7eb47c7" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-classification-claim.schema.json", bytes: 3136, sha: "8dccdd325290c0ef8b98ad2acb9d3c7d59b3d0e0fe4e9886472417bb716c14ba" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-gated-evaluation-plan.schema.json", bytes: 13172, sha: "10874bb36b6cdb91366f298373d80872fd782ca727fa829a2c52c396675dc0c9" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-gated-llm-evaluation-plan.schema.json", bytes: 20735, sha: "6ffd85230ac3d9e49642d61ae82d59e300ba6c9c5429cd75459d0b41fb6fad83" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-gated-semantic-evaluation-plan.schema.json", bytes: 26657, sha: "a59831e51ddf2a19d79000d81ce276a9bb09734e6470a420283181403a911288" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-identity-transition-claim.schema.json", bytes: 3670, sha: "4fda26cff3b9373647c090ffed3ee48b8ca81b9a25851fbc2a73647be75c7b94" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-item-manifest.schema.json", bytes: 3431, sha: "fc768e6245bbfcc16bc004abffc28bb7dca25239b3eb1b68ed7260fa9e0af18b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-lexical-translatability-claim.schema.json", bytes: 6265, sha: "bb71e78d2b0a5cd8b6baec105d04fe6ab681daa086c99772ddf66a6316e50feb" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-link.schema.json", bytes: 2618, sha: "0c2ca8b65823e69bf80417b029149f392e934b41c02dcbcfae0a198a4f2867b3" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-member-structure-claim.schema.json", bytes: 2070, sha: "a24a423ffbdb36ea18474edd38ba267e91e98f868e72b031b334b996639ef123" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-metadata-record.schema.json", bytes: 2779, sha: "cd63f93ec325d06f53329adf7475731d819a1d98c33db458c5f28274bf1e3ea4" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-occurrence-motif-claim.schema.json", bytes: 4004, sha: "eb79f3b3c9467a21bab13bb9981c8b45782bd75ed2e67d1cb48326a387631ec3" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-payload-rights-review.schema.json", bytes: 4543, sha: "fbc75f9adbf508e0c90770eabd4be58f54ef1b1447576f62c91d42bc772b6e06" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-record-closure-selection.schema.json", bytes: 2096, sha: "26ef7d0fc255018a002905922c09d845b5933d1626d796e3a84aae394ddd6867" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-registry-normalization.schema.json", bytes: 4156, sha: "22778baf9bec309709ecd59c9c50e68b44a0edf52b6cddccad4d7b42e7021eaf" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-relation-claim.schema.json", bytes: 604, sha: "14a7bbf923b30ced37953649cfced1b10069adb2f63266c56aea7f115318defe" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-resource-inventory.schema.json", bytes: 13926, sha: "71b5ae06f827a0bfcbe2063cae808e59a6803265af0a8e5943f1afaa6d137910" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-structured-value.schema.json", bytes: 1289, sha: "d0392d43c8e45671894174fc09bc930fa696766d1a803494096ad04cd6f2fcf8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-temporal-claim.schema.json", bytes: 1820, sha: "b18e447fe2b6a0e3905c29f0705799fc40293c6e0d635d16bc71294c99d711b4" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-text-layer.schema.json", bytes: 23349, sha: "019b223d7f2449979d0671f3e4e46dc443a4c9f71097874db5fca904e05b4b95" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-text-unit-packet-v1.schema.json", bytes: 28409, sha: "6ba96f4298cf5e88ae261c53c46c7d51681d0f0f0faf3bcecce2e309a0e071ab" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-textual-survival-claim.schema.json", bytes: 2653, sha: "4422d5f94055523a34029b26360adcccec320e6ebee1e486db6e595ea2242f38" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-witness-bibliographic-graph.schema.json", bytes: 18615, sha: "e1b3237b9ff29c30504b28ed662959848a1724bc0eb169015324618ccd063b1f" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/source-witness-catalog.schema.json", bytes: 16261, sha: "678c1f2f0ab01085d5e90d3d85fcea307023e3a44ada064a3eaae70fd6a32d40" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/subject-identity-transition-claim.schema.json", bytes: 3736, sha: "024bb9459f2ecba2986552dfba2cd2fdef6a3a95a20e63c6e16a2174b7fbcc33" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/target-numbered-unit-page-map.schema.json", bytes: 11540, sha: "531cc5674180a864d25ba3246110e560503417880731aa8ebd923719a6ea1d7a" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/textual-passage-claim.schema.json", bytes: 1834, sha: "d56a151d3c76ae01200945b3755ae243506213e705be18444ce6ed0f34c8375e" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/textual-passage-record.schema.json", bytes: 3643, sha: "fee443fc946283732e937ba60c603066aa52e65fe5b66eccdd9d7012e8fc5cc8" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/thought-description-record.schema.json", bytes: 5888, sha: "c1071eea9c5a58fb3dc2339094d76d74fb49a22e9c5d48569d16b48ba3f91edd" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/thought-practice-record.schema.json", bytes: 14518, sha: "2d42c171b2f01d9ae4963860d1473724d7958f7c6e67b29012c269716826b702" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/thought-relation-claim.schema.json", bytes: 1046, sha: "fbe86b15696550317f20cf5526ae96af1d514b0cc7c48ebd9ba04b1e3a975016" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/thought-topic-record.schema.json", bytes: 9117, sha: "41291684c19b66e6ab2041e2cd48adb5c42cb02f841d8b5c333754feeb2c2cf4" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/tos-corpus-index.schema.json", bytes: 21026, sha: "2dfd8d0a8076e8c7d7a93c1c23dc727bcbbf599e53641e95c888e3cde492d411" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/tos-node-contract.schema.json", bytes: 11754, sha: "738dac1048745056ac1710d30ad38fa3b636dc8307f0f4c8ce7775c814861daa" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/tos-source-home.schema.json", bytes: 2549, sha: "1d0676db1088240200f4d98323af04197d05153587724971be12502cd6f67dfa" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/tos-tiny-entry-route.schema.json", bytes: 2207, sha: "aafb6f5c79981ee87f4b6dfd2c7866ba51146d5eb8f264bc9b66438b64551507" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-candidate-source-structural-route.schema.json", bytes: 6087, sha: "977998fc42f869a28cbc7dc7011e62c820a8146a88df1720055c46c9de4c8a84" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-candidate-structural-crosswalk.schema.json", bytes: 8229, sha: "adbcfd47146cf1149e5fe5a20ecfa408983083d10c406130ac0344f237cdd922" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-candidate-target-structural-crosswalk.schema.json", bytes: 8366, sha: "d42c611b518390b133efd0bea877791e1c4425a0eb50c09ad17546d416fe4850" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-route-readiness-projection-v2.schema.json", bytes: 9723, sha: "7de355ab3338f84d8380d30f76cafd2aa4d59f0efba2566695a37996d7cd5b18" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-route-readiness-projection.schema.json", bytes: 7936, sha: "6e5ab096f2f901805b93bfc15d5903f9644891a32b6020270450e624377c4a2d" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-source-passage-candidate-set.schema.json", bytes: 18121, sha: "23c616725cd98f7e4477b26fd07703531f7fe86df480af428870ddf6cafbde41" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-source-visible-review-receipt.schema.json", bytes: 12467, sha: "af388db349c40dfc8e5ac5604dfa1db90c3e77839c4332ec9df9e0f48b869f65" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/transfer-target-passage-candidate-set.schema.json", bytes: 10528, sha: "91365cd5f4f75cc7340ff17b5cceb74657d7e3f13e6d6dfeee06cb33d81c7345" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-alignment-packet-v1.schema.json", bytes: 23228, sha: "69be2c70d9eac1cd1ccef165d99b5c7709cd2e52f8ec8022548ce33eb7cca130" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-exposure-aware-plan.schema.json", bytes: 20024, sha: "fc5bbf611f88034d0222d44abea827b566d43b362c7bca8b34b66295f6db1539" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-laboratory-plan.schema.json", bytes: 17079, sha: "930f3282ebb53a340d0432f8c8020f900cdc2788084b0fa975d06091fdc10649" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-packet.schema.json", bytes: 38562, sha: "5ba678a45af75398b64a2411a545d054213848b251c413f84e6f5176f9db1517" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-pre-draft-analysis.schema.json", bytes: 17377, sha: "f3f5d8fd72c74bb8e0d7f132651823f20d02efe97edcac3d058a3616df087d5c" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-reference-register.schema.json", bytes: 13720, sha: "f0cec14021dbb5f84639cabc2bba5ab118c8a150a1ca238aeeb25337aa2f6dbf" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-sample-plan.schema.json", bytes: 5566, sha: "a4f98c74e0c33dd89c0151679608fd1b9f19f3f1fce5843a2676400bb888a157" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/translation-source-review-plan.schema.json", bytes: 8342, sha: "fd79297ab1c4ff0ac60e17206e046d97362af14a8ab93d5c100194dabe500341" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/visual-retrieval-plan.schema.json", bytes: 13550, sha: "24194109f9d6003268fd5ab9542cea56d7765edc9a6a286032ffeb037b07026b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/visual-retrieval-result-receipt.schema.json", bytes: 18851, sha: "932254b4a85327cee032b77b613678fa5c0c138591d5192752e682e323ca3028" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/witness-structure-anchor-set.schema.json", bytes: 5179, sha: "f44e4830dd5f1ed55aac798846f783da4f189eb94f13fb7e9a2a7c5280c7e98b" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/witness-structure-correspondence.schema.json", bytes: 12697, sha: "c34c26c11b45eaa1310f0be3b9044d805a318a340c6032df99ca99485142c2e2" },
    RichOriginalMember { mask: 63, path: "ToS/contracts/witness-text-collation-packet-v1.schema.json", bytes: 20584, sha: "492e0976d2f93d266b84a2916f8269e1e7f5c8dafda2f1921cc0ac9a023687cf" },
    RichOriginalMember { mask: 63, path: "ToS/doctrine/semantic-interchange/entity-types.v1.json", bytes: 264272, sha: "cad0331eea790d8d63ab25c5263b81da3dcb4366f5416a94d88b7b9fbbbce465" },
    RichOriginalMember { mask: 63, path: "ToS/doctrine/semantic-interchange/relation-types.v1.json", bytes: 335626, sha: "462d75fe3a22567e88c6c0f14b40fae5a83db244333c66acd406e9dca9c5ab4c" },
    RichOriginalMember { mask: 63, path: "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md", bytes: 65, sha: "b5a73d3f9a41963a581b8bdf055c6252210ab2980983774fc1fb6adbd9ff94c1" },
    RichOriginalMember { mask: 32, path: "ToS/review-ledger/2026-09-10-mysl-collection-order-source-reading.md", bytes: 2949, sha: "3c1b4866a1afea1a34e231091447f696c42c32825834953c00ac62b4d78ae612" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4.blob", bytes: 9323, sha: "1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018.blob", bytes: 4923, sha: "51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af.blob", bytes: 2591, sha: "acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/cb9f065a4c925bcef4b77011ff87316e3fb254e8b4aa3748c4e4db9b1f172048.blob", bytes: 2416, sha: "cb9f065a4c925bcef4b77011ff87316e3fb254e8b4aa3748c4e4db9b1f172048" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/completion.json", bytes: 527, sha: "362880c566f6ed9166b72f71c1d29960233f5638961bd73e2c03b8a1e5c6db34" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/f5a148e876b20437cc7c2a561a3ab257d8c74c9278399769f5cf76ffdb558dc7.blob", bytes: 4886, sha: "f5a148e876b20437cc7c2a561a3ab257d8c74c9278399769f5cf76ffdb558dc7" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/manifest.json", bytes: 3728, sha: "837acccab94aa9a72d821781eecd064303366888823d9e118b3659f873f0bcac" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4.blob", bytes: 9323, sha: "1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/3be5e7eb09829363d9a746519d3c154b284c1f150ea679495b462f529e839047.blob", bytes: 13770, sha: "3be5e7eb09829363d9a746519d3c154b284c1f150ea679495b462f529e839047" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018.blob", bytes: 4923, sha: "51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af.blob", bytes: 2591, sha: "acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/completion.json", bytes: 528, sha: "770db430929426d399552f30d4f12defb915217da419761d80f0eafd2e05a601" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/e78240b0cabfb726d4bdce5560e1aeab2e0ada492afde33d42985b109d5c0d08.blob", bytes: 2556, sha: "e78240b0cabfb726d4bdce5560e1aeab2e0ada492afde33d42985b109d5c0d08" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/f2270bca627b1c1d2232b02d1f2f5692acfd2685c9a876b0cb6456e1dce7daa4.blob", bytes: 9316, sha: "f2270bca627b1c1d2232b02d1f2f5692acfd2685c9a876b0cb6456e1dce7daa4" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/manifest.json", bytes: 3800, sha: "e7b866bad5a92a6fc5a27afe83f6be21d4018a94bee3a30f590c3b15ab60724f" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4.blob", bytes: 9323, sha: "1f2239e57cab70fd39d45af8277b4e2d675c46213c807223a233ecce3d9a26b4" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018.blob", bytes: 4923, sha: "51e09b868107e5fc41af804a724d214dbc55dbba677d5c4a9de60f2ab659c018" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af.blob", bytes: 2591, sha: "acf3f17bded4bedccd6d950df078fc05d203a1b46ceb3ab1b38f13a89865b0af" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/manifest.json", bytes: 1280, sha: "cd296a01a56f03fe1e0d32c6f25fc2e57ad7fcc62381d413fa3dfc22958343cc" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-fafe3ef84b65b29968511c6a06ff018e46571d11c604ec14fc5ef5018f70c1b2/cb9f065a4c925bcef4b77011ff87316e3fb254e8b4aa3748c4e4db9b1f172048.blob", bytes: 2416, sha: "cb9f065a4c925bcef4b77011ff87316e3fb254e8b4aa3748c4e4db9b1f172048" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-fafe3ef84b65b29968511c6a06ff018e46571d11c604ec14fc5ef5018f70c1b2/f5a148e876b20437cc7c2a561a3ab257d8c74c9278399769f5cf76ffdb558dc7.blob", bytes: 4886, sha: "f5a148e876b20437cc7c2a561a3ab257d8c74c9278399769f5cf76ffdb558dc7" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-fafe3ef84b65b29968511c6a06ff018e46571d11c604ec14fc5ef5018f70c1b2/manifest.json", bytes: 1037, sha: "b08aeb0e18d48d9683ac29a00a2f239b54c2af34ffa7273ad8454190b4e6f39f" },
    RichOriginalMember { mask: 63, path: "ToS/source-witnesses/agents/command-claim-fixture/agent.json", bytes: 415, sha: "71052c1c44bb57a2ad04c40cbfe26ffbd14ed12921c577d9c234043b807be83b" },
    RichOriginalMember { mask: 24, path: "ToS/source-witnesses/agents/command-identity-second/agent.json", bytes: 399, sha: "b4bb861b238411e2c28aac7799c0b6f4250a1d61ce8fd7e4658454346407d8b3" },
    RichOriginalMember { mask: 24, path: "ToS/source-witnesses/agents/command-identity-third/agent.json", bytes: 397, sha: "d2c9839039deb8977372bffb63e3c0d231f59eaaec62bcda4102cc1ba2b59e73" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/catalog/agents.jsonl", bytes: 407, sha: "a3bacf88982cab3bd1764dff99f080ff2b71521f230ce8fd8a393e6ab38985c9" },
    RichOriginalMember { mask: 24, path: "ToS/source-witnesses/catalog/agents.jsonl", bytes: 1190, sha: "06dfa04c2e06a332ef8049f5f58acd81bf67aa7881a35ae819fa1ae74fa91621" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/catalog/catalog.manifest.json", bytes: 1640, sha: "3af8be94b3d43ad540a937467d86b0e051d7c15586e5cda8e549c921f92b47e6" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/catalog/catalog.manifest.json", bytes: 1640, sha: "823b4d8b480be212ace9e2df04021d7d5cadd951f86f21aba665bb7343a893e5" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/catalog/catalog.manifest.json", bytes: 1641, sha: "23eb31771076c5364ffcbdccbc9f570df82e35d984bf94d2573930837aee8c94" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/catalog/claims.jsonl", bytes: 2363, sha: "2e8ffbb2a1bacbe46e0c7aba3c677f4deb79a8da0e5d452a4c3de517cfd0a22a" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/catalog/claims.jsonl", bytes: 2372, sha: "e27403283e56b129c1bea90b72bbdaba269842a47e0ea9d45e422a33b49258ea" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/catalog/claims.jsonl", bytes: 22700, sha: "d2d7d24d89ec35489711e0899e5712fa1599ba9fe6e8d41afb8c8f83defb99d1" },
    RichOriginalMember { mask: 24, path: "ToS/source-witnesses/catalog/collections.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/catalog/collections.jsonl", bytes: 1062, sha: "53cd4ee3142ef759ab7b913a1a0118d7d6d8c47636c46045731ef2415d1e1146" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/editions.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/expressions.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/items.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/letters.jsonl", bytes: 477, sha: "c419ddc8d518b5f6c437edb925bab8a92195466e4be2129105c31ced165395aa" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/links.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/organizations.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 56, path: "ToS/source-witnesses/catalog/places.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 24, path: "ToS/source-witnesses/catalog/works.jsonl", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/catalog/works.jsonl", bytes: 7444, sha: "2b12ebca5b2599c79e7af0aed3dc8d94b43d2db88227879aeae8b8f998ffb297" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.human-forms.json", bytes: 13770, sha: "3be5e7eb09829363d9a746519d3c154b284c1f150ea679495b462f529e839047" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json", bytes: 2556, sha: "e78240b0cabfb726d4bdce5560e1aeab2e0ada492afde33d42985b109d5c0d08" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/membership-claims.jsonl", bytes: 6694, sha: "0cdba2d535721afab470c98daaf4a7a2c69067429039a1415a57bda5eb6512a8" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/responsibility-claims.jsonl", bytes: 6641, sha: "69a71e5fd97304843d71412d614ed0b88494910bc3cb5450a93a97903eeb9303" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/source-revision-history.json", bytes: 9316, sha: "f2270bca627b1c1d2232b02d1f2f5692acfd2685c9a876b0cb6456e1dce7daa4" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/anchors.jsonl", bytes: 9610, sha: "b6ce8b7eb953fe37ea5991d9246382fac83a4413811feaf931b263d5404d735f" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json", bytes: 8788, sha: "e2f66f463fff651a68258dda1dfc6e77bd7d040991f11f2a0965218925d7d60e" },
    RichOriginalMember { mask: 63, path: "ToS/source-witnesses/letters/command-claim-fixture/letter.json", bytes: 457, sha: "4f145c99f738d46be43de2e4d2915697ca03188a89b160cec1291911e918aefd" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/.source-claims.96464a7f6a9140741887773096408e6ab9807ddb875d60a89a82898817373ec2.human-forms.json.writer.lock", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/source-claims.jsonl", bytes: 5252, sha: "9df93498d86a29efd6cb7833166f90ca8f0bb27d00ef81b70dd6afc65527379c" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/source-create-environment.json", bytes: 285, sha: "34359afda69b97c5e022a693134eb4a42d47fdb45667e1ef1a6531775a211816" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/source-create-provenance.jsonl", bytes: 13350, sha: "dc9fc84afe695f04320389f2f31c41c12c3fd8912fc2743133f8232f87e2859a" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/source-create-receipt.json", bytes: 23376, sha: "8ecc29643d22b122fad4809d7eec0b484d2bae2b4838d298b3cd91b728abc0b5" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/relations/command-collection-order/source-create-request.json", bytes: 27652, sha: "a0b01cba0582bed87a39a68b190acb9203ccd1fe5907750f70eac4b2584001ee" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/.source-claims.1e78a8b76ded4530b9d8799bede94260386b8afd38edd247a2ee8243ed64230c.human-forms.json.writer.lock", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/source-claims.jsonl", bytes: 2083, sha: "19852388cff39333e15bdb0d44c69e2d5e7cd337238b41969b47f17b53c47cd6" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/source-create-environment.json", bytes: 285, sha: "99cf80aa7367f524c4ad8abd3e934efae44b1613ffc1c97911c5f17a4295b0e6" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/source-create-provenance.jsonl", bytes: 13372, sha: "868f4465cd8f25ad64e543ea009ebddfb5c6970218ca1d162d6eb02666e04a04" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/source-create-receipt.json", bytes: 10740, sha: "4a229ba0acc45cdf556ef8f5d128e16cb7565dd1feeaa9502ce4c6702acea4e0" },
    RichOriginalMember { mask: 8, path: "ToS/source-witnesses/relations/command-identity-v1/source-create-request.json", bytes: 11878, sha: "95060fbae1a4900ff4d90b1894a1d7f6b9efc320bc321dfbe463dbf2383ed809" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/.source-claims.d4920c0b45d453a738caed31b6b5c5c8993a7b41660608474501ec450b75b057.human-forms.json.writer.lock", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/source-claims.jsonl", bytes: 2092, sha: "fed3b151ae77a34698ae67e47707ee7feb55adf9aa5fd59f8173f3529b5df25d" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/source-create-environment.json", bytes: 285, sha: "99cf80aa7367f524c4ad8abd3e934efae44b1613ffc1c97911c5f17a4295b0e6" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/source-create-provenance.jsonl", bytes: 13372, sha: "23c5bb4e88d52dce3777e7be54a9e86832b419398e132435e981b3da61573636" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/source-create-receipt.json", bytes: 10740, sha: "3b4411fc38bf4ce6cebebe826b2da6ccca1775d3d96827145efe44ce087b14c4" },
    RichOriginalMember { mask: 16, path: "ToS/source-witnesses/relations/command-identity-v2/source-create-request.json", bytes: 11887, sha: "be5f7264b9aef626e42c5c8311233c090b65d83fe54ca5b45831690bb389419c" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/.source-claims.2c4c3a4f5cb2cbf1713ebdaa0b27dfcb0729cf980f33591a6e1e2ea6296b8d25.human-forms.json.writer.lock", bytes: 0, sha: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl", bytes: 2393, sha: "c24d6bad02b05d86e7a319f28794f4dbe3978912cdede21708049ca5c3fea5c2" },
    RichOriginalMember { mask: 1, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-claims.jsonl", bytes: 2399, sha: "a0bad0e44dc62e60a1f27fc1ef47f68dd40447ac0fe8925416405d35561ef623" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-create-environment.json", bytes: 285, sha: "34359afda69b97c5e022a693134eb4a42d47fdb45667e1ef1a6531775a211816" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-create-provenance.jsonl", bytes: 13468, sha: "087b29c1ba6ddb1719d25d0295ab15f2bcfea9140e4b54b1f199290f9b94326b" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-create-receipt.json", bytes: 2449, sha: "0395b31e970f249df24c4a95eda462ca9bcf3eed3b98c52a46d6e384a7158b93" },
    RichOriginalMember { mask: 4, path: "ToS/source-witnesses/relations/nietzsche-letter-705-addressee/source-create-request.json", bytes: 3879, sha: "7e0319639b8295f8854790b0fa43485fe0745393b2f0000faf2a5ee457ab33f0" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json", bytes: 4198, sha: "f2eb1fa908f98c25e2d7bbbeaaaa49ced810a912d49bdc7c6cfa2822ca731b51" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/work.json", bytes: 2700, sha: "25f4efc99b4d31ca193ba587f30b12a0a5c9ef407e9613714cfacf24f5fb1d9b" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/der-fall-wagner/work.json", bytes: 2926, sha: "72bff933b3658421558c6793b942e819ec95398d7482d35d587166f28fa6fb24" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/ecce-homo/work.json", bytes: 2492, sha: "e5d7c1b0c9ae0816fb3a2ba36633414646cc70c93c40c5c7ab4968eea52d2ce1" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/goetzen-daemmerung/work.json", bytes: 2764, sha: "b5e1a068d9bf244aeb34396406dd5b1ebac3d1c40d58cb05b3ff84430e09dd99" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json", bytes: 2613, sha: "032d3acdc89f751943c98a0b450835effdf0bcc8e0c1ddd362bb9e2d90658f3c" },
    RichOriginalMember { mask: 32, path: "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/work.json", bytes: 2533, sha: "d02d59fd4860d2368579e4acda1f1dcdba32a10c42d13e114957060fe0cafaf2" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py", bytes: 123481, sha: "364561c250d15236328f129d9b80aa1b998620500637b766aa738db38e58e4c3" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py", bytes: 38474, sha: "0348b69120a20875582790b10690c462b80b02331581eeba79acce54aac5368a" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_version_reader.py", bytes: 24895, sha: "8dcecb87f4ac91dd7bb94e1f9491d4deff1f3ad26857c4468cd0b00f64784bcd" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py", bytes: 22147, sha: "c0e23044d76b4a2e67015ddef3b4af0beef3fb8e1fdde0a99a50649f6b5ba340" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py", bytes: 27366, sha: "4ff5546d71f6230c13fd39d29bbc3d9de261cdd18dc4d7eac43401f7319600e8" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py", bytes: 41392, sha: "6562e1d16e99d38f6b47035f70650ee3d32dbab3838532db0d196043330f4fb0" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_claim_commands.py", bytes: 35337, sha: "09e2582c47715efe7c985409907ab4f44de784ef21d459f52d9ed6be20c57337" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py", bytes: 5649, sha: "b3a8b38f95371f45b2cc4eacaec2f9c0e37cd5a6677a1724d4a64164f2ff384f" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py", bytes: 135531, sha: "b21d8f9ab8e78d2bfd7bdc9c09aee8af206d2bcfdbf439a6d06f99458ef69ce7" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py", bytes: 23535, sha: "87840e826b3e948aadcae48a5687bc1c7f75328e072b4e3fc51a76bb960d8640" },
    RichOriginalMember { mask: 2, path: "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py", bytes: 31813, sha: "a7821f3fa4260b8b383209e22b083bb4cd773b938942ebc07946b774e4bd231b" },
    RichOriginalMember { mask: 2, path: "rust/crates/tos-command/src/source_claims.rs", bytes: 360793, sha: "1319448fb44a65838cda3d131051fb7d1f631464370a2d2689ec26e313cbb5c8" },
    RichOriginalMember { mask: 2, path: "rust/crates/tos-command/src/source_serialization.rs", bytes: 57341, sha: "fda7a3f9d78f14da40fccd7e577b47c77641f1ed81f1dd25ec2bad247a58cfad" },
    RichOriginalMember { mask: 2, path: "scripts/build_source_witness_catalog.py", bytes: 30121, sha: "cf46896daa378930fc666727606836ceb7aa2a8a7be95b4eb3f22ad1255bacfd" },
    RichOriginalMember { mask: 2, path: "scripts/native_text_binding.py", bytes: 47568, sha: "84113ed2ea0334cb9b5ce44baa940b7199893ab5389cabb78659cdcd1b2d058a" },
    RichOriginalMember { mask: 2, path: "scripts/source_document_catalogue.py", bytes: 1639, sha: "ec109209caf59f299c185ecd7de1089e31610b77eb3cda604f083151bade2670" },
    RichOriginalMember { mask: 2, path: "scripts/source_identity_proposals.py", bytes: 13224, sha: "96539b59a4c86175b868bb8ff6a0ee2d7d9663ff9987ca4e378d0d0a995a813a" },
    RichOriginalMember { mask: 2, path: "scripts/source_owner_context.py", bytes: 11761, sha: "0bb984830b5bc234f97b9e17b17a3f94d5360e17889a32ea1e10a1ffc60e2b2d" },
    RichOriginalMember { mask: 2, path: "scripts/source_record_profiles.py", bytes: 56546, sha: "87703622c296bf1d5b87775f2b4a9ec2c70d7083aca62b2500475dcec341e2a9" },
    RichOriginalMember { mask: 2, path: "scripts/source_witness_bibliographic_graph_common.py", bytes: 103130, sha: "00b09e31a53d3f17d318be353ac7a3a06061c0e6a16cca5810514dbd0ed5bb23" },
    RichOriginalMember { mask: 2, path: "scripts/source_witness_human_forms.py", bytes: 46829, sha: "59f6bb4c09fed4c5b2649b9ff71aa3fd66e5f131a806b2481fd44e84b57c880b" },
];

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
    let mut rich_remaining = None;
    let mut rich_registries = None;
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
        let mut rich =
            RichClaimBudget::enter(deadline, &cancellation, rich_remaining.take()).unwrap();
        if rich_registries.is_none() {
            rich_registries = Some(rich_registry_inputs(&current_files, &mut rich).unwrap());
        }
        let registries = rich_registries.as_ref().unwrap();
        let agent_catalog = rich_read_catalog(
            isolated.path(),
            "ToS/source-witnesses/catalog/agents.jsonl",
            if version == "v1" { 8 } else { 16 },
            &mut rich,
        )
        .unwrap();
        rich.workspace(
            std::mem::size_of::<[DeclaredClaimObject<'_>; 3]>()
                + std::mem::size_of::<[DeclaredRawClaimInput<'_>; 1]>()
                + std::mem::size_of::<DeclaredRichClaimInputs<'_>>(),
        )
        .unwrap();
        let rich_records = [
            DeclaredClaimObject {
                id: refs[0]["id"].as_str().unwrap(),
                path: agent_path,
                raw: &current_files[agent_path],
                identity_field: "record_id",
            },
            DeclaredClaimObject {
                id: refs[1]["id"].as_str().unwrap(),
                path: "ToS/source-witnesses/agents/command-identity-second/agent.json",
                raw: &current_files["ToS/source-witnesses/agents/command-identity-second/agent.json"],
                identity_field: "record_id",
            },
            DeclaredClaimObject {
                id: refs[2]["id"].as_str().unwrap(),
                path: "ToS/source-witnesses/agents/command-identity-third/agent.json",
                raw: &current_files["ToS/source-witnesses/agents/command-identity-third/agent.json"],
                identity_field: "record_id",
            },
        ];
        let rich_evidence = rich_evidence_declarations(&claim, &current_files, &mut rich).unwrap();
        let rich_catalogs = [DeclaredRawClaimInput {
            path: "ToS/source-witnesses/catalog/agents.jsonl",
            raw: &agent_catalog,
        }];
        let rich_claims = [&claim];
        let rich_inputs = DeclaredRichClaimInputs {
            selected: &current_files,
            software: &software_files,
            configuration: &revision_owner,
            request: &correction,
            claims: &rich_claims,
            records: &rich_records,
            evidence: &rich_evidence,
            catalogs: &rich_catalogs,
            original_input_projection: RICH_ORIGINAL_INPUT.as_bytes(),
        };
        check_identity_claim_inputs(
            &rich_inputs,
            if version == "v1" { 1 } else { 2 },
            &revision_context,
            current,
            registries,
            &mut rich,
        )
        .unwrap();
        rich_remaining = Some(rich.leave());
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
    let mut rich = RichClaimBudget::enter(deadline, &cancellation, None).unwrap();
    let rich_registries = rich_registry_inputs(&current_files, &mut rich).unwrap();
    let rich_collections = rich_read_catalog(
        isolated.path(),
        "ToS/source-witnesses/catalog/collections.jsonl",
        32,
        &mut rich,
    )
    .unwrap();
    let rich_claim_catalog = rich_read_catalog(
        isolated.path(),
        "ToS/source-witnesses/catalog/claims.jsonl",
        32,
        &mut rich,
    )
    .unwrap();
    let rich_work_catalog = rich_read_catalog(
        isolated.path(),
        "ToS/source-witnesses/catalog/works.jsonl",
        32,
        &mut rich,
    )
    .unwrap();
    rich.workspace(
        std::mem::size_of::<[DeclaredClaimObject<'_>; 8]>()
            + std::mem::size_of::<[DeclaredRawClaimInput<'_>; 3]>()
            + std::mem::size_of::<DeclaredRichClaimInputs<'_>>(),
    )
    .unwrap();
    let members = claim["object"]["members"].as_array().unwrap();
    let rich_records = [
        DeclaredClaimObject {
            id: claim["subject_ref"].as_str().unwrap(),
            path: "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json",
            raw: &current_files["ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[0].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[1].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[2].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[3].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/der-fall-wagner/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/der-fall-wagner/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[4].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/goetzen-daemmerung/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/goetzen-daemmerung/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[5].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/work.json"],
            identity_field: "record_id",
        },
        DeclaredClaimObject {
            id: members[6].as_str().unwrap(),
            path: "ToS/source-witnesses/works/friedrich-nietzsche/ecce-homo/work.json",
            raw: &current_files["ToS/source-witnesses/works/friedrich-nietzsche/ecce-homo/work.json"],
            identity_field: "record_id",
        },
    ];
    let rich_evidence = rich_evidence_declarations(&claim, &current_files, &mut rich).unwrap();
    let rich_catalogs = [
        DeclaredRawClaimInput {
            path: "ToS/source-witnesses/catalog/collections.jsonl",
            raw: &rich_collections,
        },
        DeclaredRawClaimInput {
            path: "ToS/source-witnesses/catalog/claims.jsonl",
            raw: &rich_claim_catalog,
        },
        DeclaredRawClaimInput {
            path: "ToS/source-witnesses/catalog/works.jsonl",
            raw: &rich_work_catalog,
        },
    ];
    let rich_claims = [&claim];
    let rich_inputs = DeclaredRichClaimInputs {
        selected: &current_files,
        software: &software_files,
        configuration: &revision_owner,
        request: &correction,
        claims: &rich_claims,
        records: &rich_records,
        evidence: &rich_evidence,
        catalogs: &rich_catalogs,
        original_input_projection: RICH_ORIGINAL_INPUT.as_bytes(),
    };
    check_collection_claim_inputs(
        &rich_inputs,
        &revision_context,
        current,
        &rich_registries,
        &mut rich,
    )
    .unwrap();
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
