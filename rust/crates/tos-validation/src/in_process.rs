//! Prepared, offline schema execution for one immutable resource set.

use std::collections::{BTreeMap, BinaryHeap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use jsonschema::{Draft, Registry, Validator};
use serde_json::Value;
use tos_foundation::{Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind, JsonValue, parse_json};

use crate::executor::{BatchBudget, DIAGNOSTIC_UNIT_HEADER_BYTES, schema_diagnostics::{self as diag, Caps, Failure, Issue, Status}};
use crate::{FormatProfile, SchemaBackendProbe, SchemaProbeError, SchemaResource, published_value};

pub const ENGINE_ID: &str = "tos_schema_engine_in_process_v1";

/// Input decoding is independent of the schema format profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode { PublishedStrict, LegacyPythonObserved }

/// Evidence refers to the exact input and selected resource set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub engine: &'static str,
    pub schema_set_sha256: Digest256,
    pub profile: FormatProfile,
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
pub enum Incomplete { Cancelled, Deadline, BatchBudget }

pub struct Unit<'a> { pub root_uri: &'a str, pub raw: &'a [u8], pub input_mode: InputMode }

/// The cache is local to one exact schema set and bounded by the batch selector limit.
pub struct InProcessSchemaEngine {
    probe: SchemaBackendProbe,
    profile: FormatProfile,
    validators: BTreeMap<String, Validator>,
}

impl InProcessSchemaEngine {
    pub fn new(resources: impl IntoIterator<Item = SchemaResource>, profile: FormatProfile) -> Result<Self, SchemaProbeError> {
        Ok(Self { probe: SchemaBackendProbe::prepare_diagnostics_resources(resources, profile)?, profile, validators: BTreeMap::new() })
    }
    pub fn schema_set_digest(&self) -> Digest256 { self.probe.schema_set_digest() }
    pub fn resource_digest(&self, uri: &str) -> Option<Digest256> { self.probe.resource_digest(uri) }

    fn validator(&mut self, root: &str) -> Result<&Validator, SchemaProbeError> {
        if !self.validators.contains_key(root) {
            if self.validators.len() == BatchBudget::MAX_UNITS { return Err(SchemaProbeError::BudgetExceeded); }
            let registry = Registry::new().extend(self.probe.resources.iter().map(|(uri, value)| (uri.as_str(), value.clone())))
                .map_err(|e| SchemaProbeError::Backend(e.to_string()))?.prepare().map_err(|e| SchemaProbeError::Backend(e.to_string()))?;
            crate::check_selected_schema_keywords(&registry, root, |_| Ok(()))?;
            let schema = self.probe.selected_schema(root)?;
            let mut options = jsonschema::options().with_draft(Draft::Draft202012).with_registry(&registry).offline().should_validate_formats(true).should_ignore_unknown_formats(false);
            if self.profile == FormatProfile::LegacyPythonObserved20260923 {
                options = options.with_format("date-time", |_| true).with_format("uri", |_| true).with_format("uri-reference", |_| true);
            }
            let validator = options.build(schema.as_ref()).map_err(|e| SchemaProbeError::Backend(e.to_string()))?;
            self.validators.insert(root.to_owned(), validator);
        }
        Ok(&self.validators[root])
    }

    pub fn validate(&mut self, unit: Unit<'_>, caps: Caps, deadline: Instant, cancelled: &AtomicBool) -> Result<Report, Incomplete> {
        check(deadline, cancelled)?;
        let mut report = Report { engine: ENGINE_ID, schema_set_sha256: self.schema_set_digest(), profile: self.profile, root_uri: unit.root_uri.to_owned(), input_sha256: Digest256::of_bytes(unit.raw), caps, status: Status::Indeterminate, failure: Failure::ValidatorRuntime, total_issue_count: 0, truncated: false, issues: Vec::new() };
        if !caps.validate() { return Err(Incomplete::BatchBudget); }
        let value = match unit.input_mode {
            InputMode::PublishedStrict => published_value(unit.raw, SchemaBackendProbe::MAX_INSTANCE_BYTES).map_err(|e| match e {
                SchemaProbeError::BudgetExceeded => Failure::InputBudget,
                SchemaProbeError::InvalidJson | SchemaProbeError::InvalidPublishedJson(_) => Failure::InvalidJson,
                _ => Failure::UnsupportedInputSemantics,
            }),
            InputMode::LegacyPythonObserved => legacy_value(unit.raw),
        };
        match value {
            Err(failure) => { report.failure = failure; report.status = if matches!(failure, Failure::InvalidJson | Failure::InputBudget) { Status::InputRejected } else { Status::Indeterminate }; }
            Ok(value) => match self.validator(unit.root_uri) {
                Ok(validator) => collect(validator, &value, &mut report),
                Err(SchemaProbeError::UnknownKeyword(_)) => report.failure = Failure::UnsupportedInputSemantics,
                Err(_) => {},
            },
        }
        check(deadline, cancelled)?;
        Ok(report)
    }

    /// Returns complete ordered coverage or an explicit incomplete result.
    pub fn validate_batch(&mut self, units: &[Unit<'_>], caps: Caps, deadline: Instant, cancelled: &AtomicBool) -> Result<Vec<Report>, Incomplete> {
        if units.is_empty() || units.len() > BatchBudget::MAX_UNITS || units.iter().try_fold(0usize, |n, u| n.checked_add(u.raw.len())).is_none_or(|n| n > BatchBudget::MAX_RAW_BYTES) { return Err(Incomplete::BatchBudget); }
        units.iter().map(|u| self.validate(Unit { root_uri: u.root_uri, raw: u.raw, input_mode: u.input_mode }, caps, deadline, cancelled)).collect()
    }
}

fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), Incomplete> {
    if cancelled.load(Ordering::Relaxed) { Err(Incomplete::Cancelled) } else if Instant::now() >= deadline { Err(Incomplete::Deadline) } else { Ok(()) }
}

fn legacy_value(raw: &[u8]) -> Result<Value, Failure> {
    let limits = JsonLimits::new(SchemaBackendProbe::MAX_INSTANCE_BYTES, 64, 300_000, 4_300).map_err(|_| Failure::InputBudget)?;
    let doc = parse_json(raw, JsonMode::LegacyPythonObserved, limits).map_err(|e| if e.code == FoundationErrorCode::BudgetExceeded { Failure::InputBudget } else { Failure::InvalidJson })?;
    fn convert(value: &JsonValue) -> Option<Value> {
        Some(match value {
            JsonValue::Null => Value::Null, JsonValue::Bool(v) => Value::Bool(*v),
            JsonValue::Number(n) => Value::Number(match n.kind { JsonNumberKind::Int => n.lexeme.parse().ok()?, JsonNumberKind::Float => serde_json::Number::from_f64(n.as_python_float()?)? }),
            JsonValue::String(s) => Value::String(s.as_str()?.to_owned()),
            JsonValue::Array(a) => Value::Array(a.iter().map(convert).collect::<Option<_>>()?),
            JsonValue::Object(o) => { let mut map = serde_json::Map::new(); for (k, v) in o { map.insert(k.as_str()?.to_owned(), convert(v)?); } Value::Object(map) }
        })
    }
    convert(doc.root()).ok_or(Failure::UnsupportedInputSemantics)
}

fn collect(validator: &Validator, value: &Value, report: &mut Report) {
    let caps = report.caps;
    let mut retained = BinaryHeap::new();
    let mut indeterminate = false;
    for error in validator.iter_errors(value) {
        report.total_issue_count = report.total_issue_count.saturating_add(1);
        let Some((issue, failed)) = diag::issue_from_validation_error(&error, caps) else { report.truncated = true; continue; };
        indeterminate |= failed;
        if retained.len() < caps.max_issues_per_unit as usize { retained.push(issue); }
        else { report.truncated = true; if retained.peek().is_some_and(|largest| issue < *largest) { retained.pop(); retained.push(issue); } }
    }
    let mut issues = retained.into_vec(); issues.sort();
    let mut bytes = DIAGNOSTIC_UNIT_HEADER_BYTES;
    for issue in issues {
        let Some(next) = diag::issue_payload(&issue).and_then(|p| bytes.checked_add(p.len())) else { report.status = Status::Indeterminate; report.failure = Failure::ValidatorRuntime; return; };
        if next > caps.max_report_bytes_per_unit as usize { report.truncated = true; break; }
        bytes = next; report.issues.push(issue);
    }
    report.status = if indeterminate { Status::Indeterminate } else if report.truncated { Status::Truncated } else if report.total_issue_count == 0 { Status::Valid } else { Status::Invalid };
    report.failure = if indeterminate { Failure::ValidatorRuntime } else { Failure::None };
}
