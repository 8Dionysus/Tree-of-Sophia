//! Prepared, offline schema execution for one immutable resource set.

use std::cell::Cell;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use jsonschema::{Draft, Keyword, Registry, ValidationError, Validator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tos_foundation::{
    Digest256, Digest256Hasher, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind,
    JsonValue, parse_json,
};

use crate::executor::{
    BatchBudget, DIAGNOSTIC_UNIT_HEADER_BYTES,
    schema_diagnostics::{self as diag, Caps, Failure, Issue, Status},
};
use crate::{FormatProfile, SchemaBackendProbe, SchemaProbeError, SchemaResource, published_value};

pub const ENGINE_ID: &str = tos_foundation::IN_PROCESS_SCHEMA_ENGINE_ID;

/// Input decoding is independent of the schema format profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    PublishedStrict,
    /// The existing selected finite JSON profile allows the batch raw-byte limit.
    PublishedStrictSelected,
    /// Preserve the existing scalar route's serde decode and re-encoding.
    SerdeNormalized,
    LegacyPythonObserved,
    /// Historical Python-compatible grammar with an exact selected raw byte limit.
    LegacyPythonObservedBounded {
        max_instance_bytes: usize,
    },
    LegacyPythonObservedSelected(crate::executor::LegacySelectedDiagnosticsLimits),
}

impl InputMode {
    /// SHA-256 of `tos-schema-input-mode-v1\0` and the mode's ASCII tag.
    /// Selected legacy input adds a NUL and its byte limit (u64), visit limit
    /// (u32), parser state (u64), and conversion state (u64), in big-endian order.
    pub fn identity_sha256(self) -> Digest256 {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-schema-input-mode-v1\0");
        match self {
            Self::PublishedStrict => hash.update(b"published-strict"),
            Self::PublishedStrictSelected => hash.update(b"published-strict-selected"),
            Self::SerdeNormalized => hash.update(b"serde-normalized"),
            Self::LegacyPythonObserved => hash.update(b"legacy-python-observed"),
            Self::LegacyPythonObservedBounded { max_instance_bytes } => {
                hash.update(b"legacy-python-observed-bounded\0");
                hash.update(&(max_instance_bytes as u64).to_be_bytes());
            }
            Self::LegacyPythonObservedSelected(limits) => {
                hash.update(b"legacy-python-observed-selected\0");
                hash.update(&(limits.max_instance_bytes as u64).to_be_bytes());
                hash.update(&limits.max_visits.to_be_bytes());
                hash.update(&limits.parser_state_bytes.to_be_bytes());
                hash.update(&limits.conversion_state_bytes.to_be_bytes());
            }
        }
        hash.finalize()
    }
}

/// Evidence refers to the exact input and selected resource set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub engine: &'static str,
    pub schema_set_sha256: Digest256,
    pub profile: FormatProfile,
    pub input_mode: InputMode,
    pub root_uri: String,
    pub input_sha256: Digest256,
    pub caps: Caps,
    pub status: Status,
    pub failure: Failure,
    pub total_issue_count: u64,
    pub truncated: bool,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incomplete {
    Cancelled,
    Deadline,
    BatchBudget,
}

pub struct Unit<'a> {
    pub root_uri: &'a str,
    pub raw: &'a [u8],
    pub input_mode: InputMode,
}

/// The cache is local to one exact schema set and bounded by the batch selector limit.
/// A validator depends on the schema set, the format profile (fixed for the engine)
/// and the root: input modes only decode the instance, so one root compiles once.
pub struct InProcessSchemaEngine {
    probe: SchemaBackendProbe,
    profile: FormatProfile,
    validators: BTreeMap<String, Validator>,
}

/// Per-check ceilings a caller's operation envelope adds to one validation: the
/// thread CPU time the check may still spend and the decoded instance state it
/// may hold (the bounds a schema worker received as CPU and address-space limits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitCeilings {
    pub cpu: Option<Duration>,
    pub max_state_bytes: Option<u64>,
}

// ── Interruption inside one validation ──────────────────────────────────────
// Every subschema the engine compiles carries INTERRUPT_KEYWORD. Each time the
// validator applies a subschema the keyword counts, and every INTERRUPT_STRIDE
// applications it reads the running check's cancellation flag, wall deadline
// and thread CPU ceiling. The engine collects every error of a check before it
// returns any, so a flag alone would let it walk the whole instance: a tripped
// check therefore unwinds out of the engine with InterruptSignal, which
// `collect_interruptible` catches, and its report is discarded as incomplete.
// Under `panic = "abort"` there is no unwinding; the tripped check then fails
// every remaining subschema and is still discarded.
const INTERRUPT_KEYWORD: &str = "x-tos-interrupt";
const INTERRUPT_STRIDE: u32 = 64;

#[derive(Clone, Copy)]
struct Running {
    deadline: Instant,
    cpu_deadline: Option<Duration>,
    cancelled: *const AtomicBool,
    applications: u32,
    tripped: Option<Incomplete>,
}

thread_local! {
    static RUNNING: Cell<Option<Running>> = const { Cell::new(None) };
}

/// The running check on this thread, from its start to the end of its validation.
struct RunningCheck {
    previous: Option<Running>,
}

impl RunningCheck {
    fn enter(deadline: Instant, cpu_deadline: Option<Duration>, cancelled: &AtomicBool) -> Self {
        let previous = RUNNING.with(|cell| {
            cell.replace(Some(Running {
                deadline,
                cpu_deadline,
                cancelled,
                applications: 0,
                tripped: None,
            }))
        });
        Self { previous }
    }

    fn tripped(&self) -> Option<Incomplete> {
        RUNNING.with(|cell| cell.get().and_then(|running| running.tripped))
    }
}

impl Drop for RunningCheck {
    fn drop(&mut self) {
        RUNNING.with(|cell| cell.set(self.previous));
    }
}

/// CPU time this thread has spent, where the platform reports it.
pub(crate) fn thread_cpu_time() -> Option<Duration> {
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    {
        let mut spec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: clock_gettime writes one timespec into the valid local above.
        let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut spec) };
        if status == 0 && spec.tv_sec >= 0 && spec.tv_nsec >= 0 {
            return Some(Duration::new(spec.tv_sec as u64, spec.tv_nsec as u32));
        }
    }
    None
}

fn interrupted() -> bool {
    RUNNING.with(|cell| {
        let Some(mut running) = cell.get() else {
            return false;
        };
        if running.tripped.is_none() {
            running.applications = running.applications.wrapping_add(1);
            if running.applications % INTERRUPT_STRIDE == 0 {
                // SAFETY: RunningCheck stores this pointer from a reference the
                // caller holds for the whole check and restores the previous
                // state before that reference ends; the keyword runs only inside
                // that check, synchronously, on this same thread.
                let cancelled = unsafe { &*running.cancelled };
                running.tripped = if cancelled.load(Ordering::Relaxed) {
                    Some(Incomplete::Cancelled)
                } else if Instant::now() >= running.deadline {
                    Some(Incomplete::Deadline)
                } else if running
                    .cpu_deadline
                    .zip(thread_cpu_time())
                    .is_some_and(|(limit, now)| now >= limit)
                {
                    Some(Incomplete::BatchBudget)
                } else {
                    None
                };
            }
            cell.set(Some(running));
            if running.tripped.is_some() && cfg!(panic = "unwind") {
                // resume_unwind skips the panic hook: this is control flow, not a fault.
                std::panic::resume_unwind(Box::new(InterruptSignal));
            }
        }
        running.tripped.is_some()
    })
}

/// The unwinding payload of a tripped check.
struct InterruptSignal;

/// Run `collect` so that a tripped check stops the engine where it is.
/// The validator is only borrowed and the report of a tripped check is dropped,
/// so no state observed after the unwind is partial.
fn collect_interruptible(validator: &Validator, value: &Value, report: &mut Report) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        collect(validator, value, report)
    }));
    if let Err(payload) = outcome {
        if !payload.is::<InterruptSignal>() {
            std::panic::resume_unwind(payload);
        }
    }
}

struct Interrupt;

impl<'i> Keyword<'i> for Interrupt {
    fn validate(&self, _instance: &'i Value) -> Result<(), ValidationError<'i>> {
        if interrupted() {
            Err(ValidationError::custom("tos schema check interrupted"))
        } else {
            Ok(())
        }
    }

    fn is_valid(&self, _instance: &'i Value) -> bool {
        !interrupted()
    }
}

/// A copy of a schema whose every applied subschema carries the interrupt keyword.
/// It follows the keyword positions of the strict selected-keyword policy; values
/// of `const`, `enum`, `default` and annotations are data and stay untouched.
fn with_interrupts(schema: &Value) -> Value {
    fn inject(schema: &mut Value) {
        let Some(object) = schema.as_object_mut() else {
            return;
        };
        for key in [
            "additionalProperties",
            "unevaluatedProperties",
            "unevaluatedItems",
            "additionalItems",
            "items",
            "contains",
            "not",
            "if",
            "then",
            "else",
            "propertyNames",
        ] {
            if let Some(child) = object.get_mut(key) {
                if let Value::Array(children) = child {
                    // Draft-4 style tuple `items` / `additionalItems` arrays.
                    children.iter_mut().for_each(inject);
                } else {
                    inject(child);
                }
            }
        }
        for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
            if let Some(Value::Array(children)) = object.get_mut(key) {
                children.iter_mut().for_each(inject);
            }
        }
        for key in [
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
        ] {
            if let Some(Value::Object(children)) = object.get_mut(key) {
                children.values_mut().for_each(inject);
            }
        }
        object.insert(INTERRUPT_KEYWORD.to_owned(), Value::Bool(true));
    }
    let mut copy = schema.clone();
    inject(&mut copy);
    copy
}

/// The decoded state an instance holds, counted until `limit` is passed.
fn value_state_bytes(value: &Value, limit: u64) -> u64 {
    let mut total = 0u64;
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        total = total.saturating_add(std::mem::size_of::<Value>() as u64);
        match value {
            Value::String(text) => total = total.saturating_add(text.len() as u64),
            Value::Number(number) => total = total.saturating_add(number.as_str().len() as u64),
            Value::Array(items) => pending.extend(items),
            Value::Object(map) => {
                for (key, item) in map {
                    total =
                        total.saturating_add((key.len() + std::mem::size_of::<String>()) as u64);
                    pending.push(item);
                }
            }
            Value::Null | Value::Bool(_) => {}
        }
        if total > limit {
            break;
        }
    }
    total
}

impl InProcessSchemaEngine {
    pub fn new(
        resources: impl IntoIterator<Item = SchemaResource>,
        profile: FormatProfile,
    ) -> Result<Self, SchemaProbeError> {
        Ok(Self {
            probe: SchemaBackendProbe::prepare_diagnostics_resources(resources, profile)?,
            profile,
            validators: BTreeMap::new(),
        })
    }
    pub fn schema_set_digest(&self) -> Digest256 {
        self.probe.schema_set_digest()
    }
    pub fn resource_digest(&self, uri: &str) -> Option<Digest256> {
        self.probe.resource_digest(uri)
    }

    fn validator(&mut self, root: &str) -> Result<&Validator, SchemaProbeError> {
        if !self.validators.contains_key(root) {
            if self.validators.len() == BatchBudget::MAX_UNITS {
                return Err(SchemaProbeError::BudgetExceeded);
            }
            let registry = Registry::new()
                .extend(
                    self.probe
                        .resources
                        .iter()
                        .map(|(uri, value)| (uri.as_str(), value.clone())),
                )
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?
                .prepare()
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?;
            crate::check_selected_schema_keywords(&registry, root, |_| Ok(()))?;
            // The checked closure is compiled from copies carrying the interrupt keyword.
            let interruptible = Registry::new()
                .extend(
                    self.probe
                        .resources
                        .iter()
                        .map(|(uri, value)| (uri.as_str(), with_interrupts(value))),
                )
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?
                .prepare()
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?;
            let schema = with_interrupts(self.probe.selected_schema(root)?.as_ref());
            let mut options = jsonschema::options()
                .with_draft(Draft::Draft202012)
                .with_registry(&interruptible)
                .with_keyword(INTERRUPT_KEYWORD, |_, _, _| {
                    Ok(Box::new(Interrupt) as Box<dyn for<'i> Keyword<'i>>)
                })
                .offline()
                .should_validate_formats(true)
                .should_ignore_unknown_formats(false);
            if self.profile == FormatProfile::LegacyPythonObserved20260923 {
                options = options
                    .with_format("date-time", |_| true)
                    .with_format("uri", |_| true)
                    .with_format("uri-reference", |_| true);
            }
            let validator = options
                .build(&schema)
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?;
            self.validators.insert(root.to_owned(), validator);
        }
        Ok(&self.validators[root])
    }

    /// Compiled validators held by this engine (one per selected root).
    pub fn compiled_validators(&self) -> usize {
        self.validators.len()
    }

    pub fn validate(
        &mut self,
        unit: Unit<'_>,
        caps: Caps,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Report, Incomplete> {
        self.validate_within(unit, caps, UnitCeilings::default(), deadline, cancelled)
    }

    /// Validate one unit inside a caller's ceilings. The deadline, cancellation and
    /// CPU ceiling interrupt the validation itself, not only its start and end.
    pub fn validate_within(
        &mut self,
        unit: Unit<'_>,
        caps: Caps,
        ceilings: UnitCeilings,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Report, Incomplete> {
        check(deadline, cancelled)?;
        let cpu_deadline = match ceilings.cpu {
            Some(allowance) => Some(
                thread_cpu_time()
                    .and_then(|now| now.checked_add(allowance))
                    .ok_or(Incomplete::BatchBudget)?,
            ),
            None => None,
        };
        if unit.root_uri.len() > crate::executor::MAX_URI_BYTES {
            return Err(Incomplete::BatchBudget);
        }
        let mut report = Report {
            engine: ENGINE_ID,
            schema_set_sha256: self.schema_set_digest(),
            profile: self.profile,
            input_mode: unit.input_mode,
            root_uri: unit.root_uri.to_owned(),
            input_sha256: Digest256::of_bytes(unit.raw),
            caps,
            status: Status::Indeterminate,
            failure: Failure::ValidatorRuntime,
            total_issue_count: 0,
            truncated: false,
            issues: Vec::new(),
        };
        if !caps.validate()
            || (caps.max_report_bytes_per_unit as usize) < DIAGNOSTIC_UNIT_HEADER_BYTES
        {
            return Err(Incomplete::BatchBudget);
        }
        if matches!(unit.input_mode, InputMode::LegacyPythonObservedBounded { max_instance_bytes } if max_instance_bytes == 0 || max_instance_bytes > BatchBudget::MAX_RAW_BYTES)
            || matches!(unit.input_mode, InputMode::LegacyPythonObservedSelected(limits) if limits.validate().is_err())
        {
            return Err(Incomplete::BatchBudget);
        }
        if matches!(
            unit.input_mode,
            InputMode::LegacyPythonObserved
                | InputMode::LegacyPythonObservedBounded { .. }
                | InputMode::LegacyPythonObservedSelected(_)
        ) && self.profile != FormatProfile::LegacyPythonObserved20260923
        {
            report.failure = Failure::UnsupportedInputSemantics;
            check(deadline, cancelled)?;
            return Ok(report);
        }
        let value = match unit.input_mode {
            InputMode::PublishedStrict | InputMode::PublishedStrictSelected => {
                let max_bytes = if unit.input_mode == InputMode::PublishedStrictSelected {
                    BatchBudget::MAX_RAW_BYTES
                } else {
                    SchemaBackendProbe::MAX_INSTANCE_BYTES
                };
                published_value(unit.raw, max_bytes).map_err(|e| match e {
                    SchemaProbeError::BudgetExceeded => Failure::InputBudget,
                    SchemaProbeError::InvalidJson | SchemaProbeError::InvalidPublishedJson(_) => {
                        Failure::InvalidJson
                    }
                    _ => Failure::UnsupportedInputSemantics,
                })
            }
            InputMode::SerdeNormalized => {
                if unit.raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
                    Err(Failure::InputBudget)
                } else {
                    serde_json::from_slice::<Value>(unit.raw)
                        .map_err(|_| Failure::InvalidJson)
                        .and_then(|decoded| {
                            serde_json::to_vec(&decoded).map_err(|_| Failure::ValidatorRuntime)
                        })
                        .and_then(|normalized| {
                            published_value(&normalized, SchemaBackendProbe::MAX_INSTANCE_BYTES)
                                .map_err(|error| match error {
                                    SchemaProbeError::BudgetExceeded => Failure::InputBudget,
                                    SchemaProbeError::InvalidJson
                                    | SchemaProbeError::InvalidPublishedJson(_) => {
                                        Failure::InvalidJson
                                    }
                                    _ => Failure::UnsupportedInputSemantics,
                                })
                        })
                }
            }
            InputMode::LegacyPythonObservedSelected(limits) => {
                crate::selected_legacy::in_process_selected_legacy(unit.raw, limits)
            }
            InputMode::LegacyPythonObserved | InputMode::LegacyPythonObservedBounded { .. } => {
                match legacy_tree(
                    unit.raw,
                    match unit.input_mode {
                        InputMode::LegacyPythonObservedBounded { max_instance_bytes } => {
                            max_instance_bytes
                        }
                        _ => SchemaBackendProbe::MAX_INSTANCE_BYTES,
                    },
                ) {
                    Ok(tree) => match legacy_value(&tree) {
                        Some(value) => Ok(value),
                        None => {
                            #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
                            {
                                if self.profile != FormatProfile::LegacyPythonObserved20260923 {
                                    report.failure = Failure::UnsupportedInputSemantics;
                                } else {
                                    match self.validator(unit.root_uri) {
                                        Ok(_) => {
                                            (
                                                report.status,
                                                report.failure,
                                                report.total_issue_count,
                                                report.truncated,
                                                report.issues,
                                            ) = crate::executor::in_process_exceptional(
                                                &self.probe.resources,
                                                unit.root_uri,
                                                &tree,
                                                caps,
                                            );
                                        }
                                        Err(SchemaProbeError::UnknownKeyword(_)) => {
                                            report.failure = Failure::UnsupportedInputSemantics
                                        }
                                        Err(_) => {}
                                    }
                                }
                            }
                            #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
                            {
                                report.failure = Failure::UnsupportedInputSemantics;
                            }
                            check(deadline, cancelled)?;
                            return Ok(report);
                        }
                    },
                    Err(failure) => Err(failure),
                }
            }
        };
        match value {
            Err(failure) => {
                report.failure = failure;
                report.status = if matches!(failure, Failure::InvalidJson | Failure::InputBudget) {
                    Status::InputRejected
                } else {
                    Status::Indeterminate
                };
            }
            Ok(value)
                if ceilings
                    .max_state_bytes
                    .is_some_and(|limit| value_state_bytes(&value, limit) > limit) =>
            {
                report.failure = Failure::InputBudget;
                report.status = Status::InputRejected;
            }
            Ok(value) => match self.validator(unit.root_uri) {
                Ok(validator) => {
                    let running = RunningCheck::enter(deadline, cpu_deadline, cancelled);
                    collect_interruptible(validator, &value, &mut report);
                    if let Some(incomplete) = running.tripped() {
                        return Err(incomplete);
                    }
                }
                Err(SchemaProbeError::UnknownKeyword(_)) => {
                    report.failure = Failure::UnsupportedInputSemantics
                }
                Err(_) => {}
            },
        }
        if cpu_deadline
            .zip(thread_cpu_time())
            .is_some_and(|(limit, now)| now >= limit)
        {
            return Err(Incomplete::BatchBudget);
        }
        check(deadline, cancelled)?;
        Ok(report)
    }

    /// Returns complete ordered coverage or an explicit incomplete result.
    pub fn validate_batch(
        &mut self,
        units: &[Unit<'_>],
        caps: Caps,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<Report>, Incomplete> {
        if units.is_empty()
            || units.len() > BatchBudget::MAX_UNITS
            || units
                .iter()
                .try_fold(0usize, |n, u| n.checked_add(u.raw.len()))
                .is_none_or(|n| n > BatchBudget::MAX_RAW_BYTES)
        {
            return Err(Incomplete::BatchBudget);
        }
        units
            .iter()
            .map(|u| {
                self.validate(
                    Unit {
                        root_uri: u.root_uri,
                        raw: u.raw,
                        input_mode: u.input_mode,
                    },
                    caps,
                    deadline,
                    cancelled,
                )
            })
            .collect()
    }
}

fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), Incomplete> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Incomplete::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Incomplete::Deadline)
    } else {
        Ok(())
    }
}

fn legacy_tree(raw: &[u8], max_instance_bytes: usize) -> Result<JsonValue, Failure> {
    let limits = JsonLimits::new(max_instance_bytes, 64, 300_000, 4_300)
        .map_err(|_| Failure::InputBudget)?;
    let doc = parse_json(raw, JsonMode::LegacyPythonObserved, limits).map_err(|e| {
        if e.code == FoundationErrorCode::BudgetExceeded {
            Failure::InputBudget
        } else {
            Failure::InvalidJson
        }
    })?;
    Ok(doc.into_root())
}

fn legacy_value(value: &JsonValue) -> Option<Value> {
    fn convert(value: &JsonValue) -> Option<Value> {
        Some(match value {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(v) => Value::Bool(*v),
            JsonValue::Number(n) => Value::Number(match n.kind {
                JsonNumberKind::Int => n.lexeme.parse().ok()?,
                JsonNumberKind::Float => serde_json::Number::from_f64(n.as_python_float()?)?,
            }),
            JsonValue::String(s) => Value::String(s.as_str()?.to_owned()),
            JsonValue::Array(a) => Value::Array(a.iter().map(convert).collect::<Option<_>>()?),
            JsonValue::Object(o) => {
                let mut map = serde_json::Map::new();
                for (k, v) in o {
                    map.insert(k.as_str()?.to_owned(), convert(v)?);
                }
                Value::Object(map)
            }
        })
    }
    convert(value)
}

fn collect(validator: &Validator, value: &Value, report: &mut Report) {
    let caps = report.caps;
    let mut retained = BinaryHeap::new();
    let mut indeterminate = false;
    for error in validator.iter_errors(value) {
        if RUNNING.with(|cell| cell.get().is_some_and(|running| running.tripped.is_some())) {
            return;
        }
        report.total_issue_count = report.total_issue_count.saturating_add(1);
        let Some((issue, failed)) = diag::issue_from_validation_error(&error, caps) else {
            report.truncated = true;
            continue;
        };
        indeterminate |= failed;
        if retained.len() < caps.max_issues_per_unit as usize {
            retained.push(issue);
        } else {
            report.truncated = true;
            if retained.peek().is_some_and(|largest| issue < *largest) {
                retained.pop();
                retained.push(issue);
            }
        }
    }
    let mut issues = retained.into_vec();
    issues.sort();
    let mut bytes = DIAGNOSTIC_UNIT_HEADER_BYTES;
    for issue in issues {
        let Some(next) = diag::issue_payload(&issue).and_then(|p| bytes.checked_add(p.len()))
        else {
            report.status = Status::Indeterminate;
            report.failure = Failure::ValidatorRuntime;
            return;
        };
        if next > caps.max_report_bytes_per_unit as usize {
            report.truncated = true;
            break;
        }
        bytes = next;
        report.issues.push(issue);
    }
    report.status = if indeterminate {
        Status::Indeterminate
    } else if report.truncated {
        Status::Truncated
    } else if report.total_issue_count == 0 {
        Status::Valid
    } else {
        Status::Invalid
    };
    report.failure = if indeterminate {
        Failure::ValidatorRuntime
    } else {
        Failure::None
    };
}

/// Stable in-process evidence codec, independent of the historical worker wire format.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceV1 {
    engine: String,
    schema_set_sha256: String,
    profile: String,
    input_mode: InputMode,
    root_uri: String,
    input_sha256: String,
    caps: Caps,
    status: Status,
    failure: Failure,
    total_issue_count: u64,
    truncated: bool,
    issues: Vec<Issue>,
}

impl Report {
    pub fn to_evidence_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&EvidenceV1 {
            engine: self.engine.to_owned(),
            schema_set_sha256: self.schema_set_sha256.to_hex(),
            profile: self.profile.id().to_owned(),
            input_mode: self.input_mode,
            root_uri: self.root_uri.clone(),
            input_sha256: self.input_sha256.to_hex(),
            caps: self.caps,
            status: self.status,
            failure: self.failure,
            total_issue_count: self.total_issue_count,
            truncated: self.truncated,
            issues: self.issues.clone(),
        })
    }

    pub fn from_evidence_json(raw: &[u8]) -> Result<Self, &'static str> {
        let json = published_value(raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
            .map_err(|_| "invalid evidence JSON")?;
        let value: EvidenceV1 =
            serde_json::from_value(json).map_err(|_| "invalid evidence JSON")?;
        if value.engine != ENGINE_ID {
            return Err("unknown engine");
        }
        let profile = [
            FormatProfile::LegacyPythonObserved20260923,
            FormatProfile::AssertedSourceCandidateV1,
        ]
        .into_iter()
        .find(|profile| profile.id() == value.profile)
        .ok_or("unknown profile")?;
        let report = Self {
            engine: ENGINE_ID,
            schema_set_sha256: Digest256::from_hex(&value.schema_set_sha256)
                .map_err(|_| "invalid schema digest")?,
            profile,
            input_mode: value.input_mode,
            root_uri: value.root_uri,
            input_sha256: Digest256::from_hex(&value.input_sha256)
                .map_err(|_| "invalid input digest")?,
            caps: value.caps,
            status: value.status,
            failure: value.failure,
            total_issue_count: value.total_issue_count,
            truncated: value.truncated,
            issues: value.issues,
        };
        if !report.is_well_formed() {
            return Err("invalid diagnostic report");
        }
        Ok(report)
    }

    pub(crate) fn is_well_formed(&self) -> bool {
        self.engine == ENGINE_ID
            && self.caps.validate()
            && match self.input_mode {
                InputMode::LegacyPythonObservedSelected(limits) => limits.validate().is_ok(),
                InputMode::LegacyPythonObservedBounded { max_instance_bytes } => {
                    max_instance_bytes > 0 && max_instance_bytes <= BatchBudget::MAX_RAW_BYTES
                }
                _ => true,
            }
            && self.issues.len() <= self.caps.max_issues_per_unit as usize
            && !self.root_uri.is_empty()
            && self.root_uri.len() <= crate::executor::MAX_URI_BYTES
            && diag::status_is_well_formed(
                self.status,
                self.failure,
                self.total_issue_count,
                self.truncated,
                self.issues.len(),
            )
            && !self.issues.windows(2).any(|p| p[0] > p[1])
            && !self.issues.iter().any(|i| {
                i.schema_keyword != i.reason.schema_keyword()
                    || !diag::path_within_caps(&i.instance_path, self.caps)
                    || !diag::path_within_caps(&i.schema_path, self.caps)
            })
            && self
                .issues
                .iter()
                .try_fold(DIAGNOSTIC_UNIT_HEADER_BYTES, |n, i| {
                    n.checked_add(diag::issue_payload(i)?.len())
                })
                .is_some_and(|n| n <= self.caps.max_report_bytes_per_unit as usize)
    }
}

#[cfg(test)]
mod input_mode_identity_tests {
    use super::*;

    #[test]
    fn bounded_legacy_mode_keeps_large_input_and_binds_the_limit() {
        let uri = "https://example.test/legacy.schema.json";
        let mut engine = InProcessSchemaEngine::new(
            [SchemaResource { uri: uri.into(), raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://example.test/legacy.schema.json","type":"object"}"#.to_vec() }],
            FormatProfile::LegacyPythonObserved20260923,
        ).unwrap();
        let mut raw = vec![b' '; SchemaBackendProbe::MAX_INSTANCE_BYTES];
        raw.extend_from_slice(b"{}");
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let bounded = InputMode::LegacyPythonObservedBounded {
            max_instance_bytes: raw.len(),
        };
        let report = engine
            .validate(
                Unit {
                    root_uri: uri,
                    raw: &raw,
                    input_mode: bounded,
                },
                Caps::CURRENT,
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(report.status, Status::Valid);
        let smaller = InputMode::LegacyPythonObservedBounded {
            max_instance_bytes: raw.len() - 1,
        };
        assert_ne!(bounded.identity_sha256(), smaller.identity_sha256());
        let rejected = engine
            .validate(
                Unit {
                    root_uri: uri,
                    raw: &raw,
                    input_mode: smaller,
                },
                Caps::CURRENT,
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(
            (rejected.status, rejected.failure),
            (Status::InputRejected, Failure::InputBudget)
        );
    }

    #[test]
    fn one_cached_validator_serves_every_input_mode_and_selected_limits_bind_it() {
        let uri = "https://example.test/number.schema.json";
        let mut engine = InProcessSchemaEngine::new(
            [SchemaResource {
                uri: uri.into(),
                raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://example.test/number.schema.json","type":"integer"}"#.to_vec(),
            }],
            FormatProfile::AssertedSourceCandidateV1,
        ).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        for mode in [
            InputMode::PublishedStrict,
            InputMode::SerdeNormalized,
            InputMode::PublishedStrict,
        ] {
            let report = engine
                .validate(
                    Unit {
                        root_uri: uri,
                        raw: b"1",
                        input_mode: mode,
                    },
                    Caps::CURRENT,
                    deadline,
                    &cancelled,
                )
                .unwrap();
            assert_eq!(report.status, Status::Valid);
        }
        // Input modes differ in how bytes decode, not in the compiled schema:
        // one root compiles once for all of them.
        assert_eq!(engine.validators.len(), 1);
        let first = crate::executor::LegacySelectedDiagnosticsLimits {
            max_instance_bytes: 1024,
            max_visits: 100,
            parser_state_bytes: 4096,
            conversion_state_bytes: 4096,
        };
        let mut second = first;
        second.max_visits += 1;
        assert_ne!(
            InputMode::LegacyPythonObservedSelected(first).identity_sha256(),
            InputMode::LegacyPythonObservedSelected(second).identity_sha256()
        );
    }
}
