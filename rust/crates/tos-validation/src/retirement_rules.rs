//! Source retirement operation and exact membership-only transition.
//! This implements the frozen Python row 14 mechanical route, not source acceptance.
//! Chronology uses the frozen Python datetime grammar and integer microseconds.
//! Stored base IDs/dependencies are carried claims: retained bytes, a review
//! binding, and this result grant no rights, canon, publication or currentness.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde_json::{Value, json};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, MemberMetadata, Snapshot};

use crate::item_rules::ItemRefusal;
use crate::source_cut::CutSchemaExecutor;

const SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct RetirementLimits {
    pub max_member_bytes: usize,
    pub max_total_bytes: u64,
    pub max_state_bytes: usize,
    pub max_entries: usize,
    pub deadline: Instant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetirementRefusal {
    Budget,
    Schema(ItemRefusal),
    Deadline,
    Source(String),
    Unsupported(String),
}
/// Mechanical transfer of the exact base index claims, never an accepted index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetirementIndex {
    pub identities: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, Vec<String>>,
}
#[derive(Debug)]
pub struct RetirementFamilyReport {
    pub revision: SourceRevision,
    pub base_revision: Option<SourceRevision>,
    pub schema_sha256: Option<Digest256>,
    pub event_identities: BTreeMap<String, String>,
    /// None means no retirement or a wider source edit needs its full owner route.
    pub membership_transition: Option<RetirementIndex>,
    pub metadata_bytes_read: u64,
    /// This rule cannot certify accepted base facts or complete source coverage.
    pub source_admission_complete: bool,
}

struct Budget<'a> {
    limits: RetirementLimits,
    cancelled: &'a AtomicBool,
    bytes: u64,
    state: usize,
    entries: usize,
}
impl Budget<'_> {
    fn check(&self) -> Result<(), RetirementRefusal> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.limits.deadline {
            Err(RetirementRefusal::Deadline)
        } else {
            Ok(())
        }
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), RetirementRefusal> {
        self.check()?;
        self.state = self
            .state
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or(RetirementRefusal::Budget)?;
        self.entries = self
            .entries
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_entries)
            .ok_or(RetirementRefusal::Budget)?;
        Ok(())
    }
    fn read(
        &mut self,
        cut: &CorpusCutReader,
        revision: SourceRevision,
        path: &RelativePath,
        cap: usize,
    ) -> Result<Vec<u8>, RetirementRefusal> {
        self.check()?;
        let metadata = cut
            .revisions()
            .find(|s| s.revision() == revision)
            .and_then(|s| s.member(path))
            .ok_or_else(|| source("source companion is missing"))?;
        self.add_bytes(metadata.size_bytes)?;
        let member = cut
            .read_member(
                revision,
                path,
                cap.min(self.limits.max_member_bytes) as u64,
                self.limits.deadline,
                self.cancelled,
            )
            .map_err(store)?;
        self.check()?;
        Ok(member.raw)
    }
    fn add_bytes(&mut self, bytes: u64) -> Result<(), RetirementRefusal> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_total_bytes)
            .ok_or(RetirementRefusal::Budget)?;
        Ok(())
    }
}

/// Actual anchored carrier seam. Only newly appended retirements use the current
/// provenance schema. Earlier ledger events retain their original revision/profile.
/// `schemas` must execute the exact current SCHEMA under the frozen retirement
/// format profile with process/resource custody; unsupported resources refuse.
/// This function never trusts caller-supplied target lists or event bytes.
pub fn inspect_retirements_from_cut(
    cut: &CorpusCutReader,
    limits: RetirementLimits,
    cancelled: &AtomicBool,
    schemas: &mut impl CutSchemaExecutor,
) -> Result<RetirementFamilyReport, RetirementRefusal> {
    if limits.max_member_bytes == 0
        || limits.max_member_bytes == usize::MAX
        || limits.max_total_bytes == 0
        || limits.max_total_bytes == u64::MAX
        || limits.max_state_bytes == 0
        || limits.max_state_bytes == usize::MAX
        || limits.max_entries == 0
        || limits.max_entries == usize::MAX
    {
        return Err(RetirementRefusal::Budget);
    }
    let mut budget = Budget {
        limits,
        cancelled,
        bytes: 0,
        state: 0,
        entries: 0,
    };
    budget.check()?;
    let current = cut.current();
    let base = current
        .base_revision()
        .map(|revision| {
            cut.revisions()
                .find(|s| s.revision() == revision)
                .ok_or_else(|| source("retirement retained base is missing"))
        })
        .transpose()?;
    let previous_ledger = base.map_or(&[][..], Snapshot::retirements);
    if !current.retirements().starts_with(previous_ledger) {
        return Err(source(
            "retirement history must preserve exact earlier entries",
        ));
    }
    let new = &current.retirements()[previous_ledger.len()..];
    let mut report = RetirementFamilyReport {
        revision: current.revision(),
        base_revision: current.base_revision(),
        schema_sha256: None,
        event_identities: BTreeMap::new(),
        membership_transition: None,
        metadata_bytes_read: 0,
        source_admission_complete: false,
    };
    if new.is_empty() {
        return Ok(report);
    }
    let base = base.ok_or_else(|| source("source retirement requires an accepted base"))?;
    let schema_path = path(SCHEMA)?;
    let schema = budget.read(cut, current.revision(), &schema_path, MAX_EVENT_BYTES)?;
    object(&schema, MAX_EVENT_BYTES)?;
    report.schema_sha256 = Some(Digest256::of_bytes(&schema));
    // Keep only bounded references to immutable ledger metadata, not retained bytes.
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    let mut retired = BTreeSet::new();
    for (offset, entry) in new.iter().enumerate() {
        budget.reserve(entry.path.as_str().len() + entry.event_ref.as_str().len() + 128)?;
        if !retired.insert(entry.path.as_str().to_owned()) {
            return Err(source("duplicate retirement target in source batch"));
        }
        if base.member(&entry.path).map(|m| m.sha256) != Some(entry.sha256) {
            return Err(source("retirement target differs from the exact base"));
        }
        let event = current
            .member(&entry.event_ref)
            .ok_or_else(|| source("retirement event missing from current membership"))?;
        if event.sha256 != entry.event_sha256
            || event.size_bytes != entry.event_size_bytes
            || entry.path == entry.event_ref
        {
            return Err(source("retirement event bytes differ from ledger binding"));
        }
        groups
            .entry(entry.event_ref.as_str().to_owned())
            .or_default()
            .push(previous_ledger.len() + offset);
    }
    if groups.keys().any(|p| retired.contains(p)) {
        return Err(source("retirement event removed by the same batch"));
    }
    let mut reviews = BTreeSet::new();
    let mut event_reviews = BTreeMap::new();
    for (event_ref, indices) in &groups {
        budget.check()?;
        if !event_ref.starts_with("ToS/source-witnesses/retirements/")
            || !event_ref.ends_with(".json")
        {
            return Err(source(
                "retirement event must use the source retirement owner path",
            ));
        }
        let raw = budget.read(cut, current.revision(), &path(event_ref)?, MAX_EVENT_BYTES)?;
        let event = object(&raw, MAX_EVENT_BYTES)?;
        if !schemas
            .check_reusing_scalar(event_ref, &raw, SCHEMA, limits.deadline, cancelled)
            .map_err(schema_error)?
        {
            return Err(source("retirement event violates provenance schema"));
        }
        budget.check()?;
        if event["schema_version"] != "tos_provenance_event_v1"
            || event["event_type"] != "migration"
            || !text(&event["event_id"])?.starts_with("tos.event.")
            || !matches!(
                event["status"].as_str(),
                Some("completed" | "completed_with_warnings")
            )
            || event["method"]["name"] != "corpus-source-retirement"
            || event["method"]["version"] != "1"
        {
            return Err(source(
                "retirement event does not declare the source retirement operation",
            ));
        }
        if observed_datetime_order(text(&event["started_at"])?, text(&event["ended_at"])?).map_err(
            |error| match error {
                ObservedDateTimeError::Budget => RetirementRefusal::Budget,
                ObservedDateTimeError::Invalid => {
                    source("retirement date-time is invalid or mixes naive and aware values")
                }
            },
        )? == std::cmp::Ordering::Greater
        {
            return Err(source("retirement event ends before it starts"));
        }
        let config = event["method"]["configuration"]
            .as_object()
            .ok_or_else(|| source("retirement configuration is not an object"))?;
        let keys: BTreeSet<&str> = config.keys().map(String::as_str).collect();
        if keys
            != BTreeSet::from([
                "base_revision",
                "retirements",
                "reason",
                "review_ref",
                "review_sha256",
            ])
        {
            return Err(source(
                "retirement configuration must bind base, exact targets and owner review",
            ));
        }
        let mut targets = indices
            .iter()
            .map(|i| {
                let entry = &current.retirements()[*i];
                json!({"path": entry.path.as_str(), "sha256": entry.sha256.to_hex()})
            })
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        if config["base_revision"] != base.revision().0.to_hex()
            || config["retirements"] != json!(targets)
        {
            return Err(source(
                "retirement targets or accepted base differ from source batch",
            ));
        }
        if text(&config["reason"])?.trim().is_empty() {
            return Err(source("retirement needs a source-visible reason"));
        }
        let review_ref = text(&config["review_ref"])?;
        let review_path = path(review_ref)?;
        let review_digest = Digest256::from_hex(text(&config["review_sha256"])?)
            .map_err(|_| source("retirement review digest is invalid"))?;
        if !review_ref.starts_with("ToS/review-ledger/") || review_ref == event_ref {
            return Err(source(
                "retirement review must return to source-owned review ledger",
            ));
        }
        let metadata = current
            .member(&review_path)
            .ok_or_else(|| source("retirement review is missing"))?;
        if metadata.sha256 != review_digest || metadata.size_bytes == 0 {
            return Err(source(
                "retirement review is empty or has a different digest",
            ));
        }
        // Read selected owner review privately without interpreting it as approval.
        budget.read(
            cut,
            current.revision(),
            &review_path,
            limits.max_member_bytes,
        )?;
        let mut inputs = targets.iter().map(|target| json!({"ref": target["path"], "role": "retired_source", "sha256": target["sha256"]})).collect::<Vec<_>>();
        inputs.push(json!({"ref": review_ref, "role": "source_owner_review", "sha256": review_digest.to_hex()}));
        if event["inputs"] != json!(inputs)
            || event["outputs"] != json!([{"ref": event_ref, "role": "corpus_retirement_event"}])
            || event["receipt_refs"] != json!([review_ref])
        {
            return Err(source(
                "retirement provenance does not bind exact sources, review and output",
            ));
        }
        let identity = text(&event["event_id"])?;
        budget.reserve(identity.len() + event_ref.len() + review_ref.len())?;
        if report
            .event_identities
            .insert(identity.to_owned(), event_ref.clone())
            .is_some()
        {
            return Err(source("duplicate source retirement event ID"));
        }
        reviews.insert(review_ref.to_owned());
        event_reviews.insert(event_ref.clone(), review_ref.to_owned());
        for index in indices {
            let entry = &current.retirements()[*index];
            let old = base
                .member(&entry.path)
                .ok_or_else(|| source("retired base member missing"))?;
            // STO stages privately and verifies digest/length before exposing bytes.
            budget.add_bytes(old.size_bytes)?;
            budget.add_bytes(entry.event_size_bytes)?;
            let retained = cut
                .read_retirement(
                    current.revision(),
                    *index,
                    limits.max_member_bytes as u64,
                    limits.deadline,
                    cancelled,
                )
                .map_err(store)?;
            if retained.raw.len() as u64 != old.size_bytes || retained.event_raw != raw {
                return Err(source(
                    "retained retirement objects differ from source bindings",
                ));
            }
            budget.check()?;
        }
    }
    report.membership_transition = membership_transition(
        current,
        base,
        &retired,
        &groups,
        &reviews,
        &event_reviews,
        &report.event_identities,
        &mut budget,
    )?;
    report.metadata_bytes_read = budget.bytes;
    budget.check()?;
    Ok(report)
}

fn membership_transition(
    current: &Snapshot,
    base: &Snapshot,
    retired: &BTreeSet<String>,
    events: &BTreeMap<String, Vec<usize>>,
    reviews: &BTreeSet<String>,
    event_reviews: &BTreeMap<String, String>,
    event_ids: &BTreeMap<String, String>,
    budget: &mut Budget<'_>,
) -> Result<Option<RetirementIndex>, RetirementRefusal> {
    if retired
        .iter()
        .any(|p| !p.starts_with("ToS/source-witnesses/"))
    {
        return Ok(None);
    }
    let mut previous = BTreeMap::<String, &MemberMetadata>::new();
    for member in base.members() {
        budget.reserve(member.path.as_str().len() + 64)?;
        previous.insert(member.path.as_str().to_owned(), member);
    }
    if events.keys().any(|p| previous.contains_key(p)) {
        return Ok(None);
    }
    let mut expected_added = events.keys().cloned().collect::<BTreeSet<_>>();
    expected_added.extend(
        reviews
            .iter()
            .filter(|p| !previous.contains_key(*p))
            .cloned(),
    );
    let mut removed = previous.keys().cloned().collect::<BTreeSet<_>>();
    let mut added = BTreeSet::new();
    for member in current.members() {
        budget.check()?;
        let p = member.path.as_str();
        if let Some(old) = previous.get(p) {
            if retired.contains(p) || *old != member {
                return Ok(None);
            }
            removed.remove(p);
        } else {
            budget.reserve(p.len() + 64)?;
            added.insert(p.to_owned());
        }
    }
    if removed != *retired || added != expected_added {
        return Ok(None);
    }
    let mut result = RetirementIndex {
        identities: BTreeMap::new(),
        dependencies: BTreeMap::new(),
    };
    for member in base.members() {
        budget.check()?;
        let p = member.path.as_str();
        if retired.contains(p) {
            continue;
        }
        if let Some(targets) = base.indexed_dependencies(&member.path) {
            let mut carried = Vec::new();
            for target in targets {
                if retired.contains(target.as_str()) {
                    return Err(source(
                        "retirement leaves an incoming source dependency unresolved",
                    ));
                }
                budget.reserve(target.as_str().len() + 32)?;
                carried.push(target.as_str().to_owned());
            }
            budget.reserve(p.len() + 64)?;
            result.dependencies.insert(p.to_owned(), carried);
        }
    }
    for (id, p) in base.indexed_identities() {
        budget.check()?;
        if event_ids.contains_key(id) {
            return Err(source(
                "retirement event reuses an accepted source identity",
            ));
        }
        if !retired.contains(p.as_str()) {
            budget.reserve(id.len() + p.as_str().len() + 64)?;
            result
                .identities
                .insert(id.to_owned(), p.as_str().to_owned());
        }
    }
    for (id, p) in event_ids {
        budget.reserve(id.len() + p.len() + 64)?;
        result.identities.insert(id.clone(), p.clone());
    }
    for (event, review) in event_reviews {
        budget.reserve(event.len() + review.len() + 64)?;
        result
            .dependencies
            .insert(event.clone(), vec![review.clone()]);
    }
    Ok(Some(result))
}
fn source(message: &str) -> RetirementRefusal {
    RetirementRefusal::Source(message.to_owned())
}
fn store(error: tos_source_store::StoreError) -> RetirementRefusal {
    if error.code == tos_source_store::StoreErrorCode::BudgetExceeded {
        RetirementRefusal::Budget
    } else {
        source(&error.to_string())
    }
}
fn schema_error(error: ItemRefusal) -> RetirementRefusal {
    match error {
        ItemRefusal::Budget => RetirementRefusal::Budget,
        error @ (ItemRefusal::BudgetCheck { .. } | ItemRefusal::Executor(_)) => {
            RetirementRefusal::Schema(error)
        }
        ItemRefusal::Deadline => RetirementRefusal::Deadline,
        ItemRefusal::Source(s) => RetirementRefusal::Source(s),
        ItemRefusal::Unsupported(s) => RetirementRefusal::Unsupported(s),
    }
}
fn path(s: &str) -> Result<RelativePath, RetirementRefusal> {
    RelativePath::parse(s).map_err(|_| source("retirement relative path is invalid"))
}
fn text(value: &Value) -> Result<&str, RetirementRefusal> {
    value
        .as_str()
        .ok_or_else(|| source("retirement string field is missing or invalid"))
}
fn object(raw: &[u8], cap: usize) -> Result<Value, RetirementRefusal> {
    let value = crate::published_value(raw, cap).map_err(|e| match e {
        crate::SchemaProbeError::BudgetExceeded => RetirementRefusal::Budget,
        crate::SchemaProbeError::InvalidPublishedJson(
            tos_foundation::FoundationErrorCode::InvalidUnicodeScalar,
        )
        | crate::SchemaProbeError::IncompatibleJsonRepresentation => RetirementRefusal::Unsupported(
            "Python retirement JSON admits a representation outside the scalar-string Rust profile"
                .into(),
        ),
        _ => source("retirement input is not strict finite JSON"),
    })?;
    if !value.is_object() {
        return Err(source("retirement input must be JSON object"));
    }
    // The source _json additionally json.dumps(..., allow_nan=False) after
    // Python loads floats. FND retains exact decimal lexemes; a JSON exponent
    // must not evade that finite-float check by staying arbitrary precision.
    finite_python_numbers(&value)?;
    Ok(value)
}

fn finite_python_numbers(value: &Value) -> Result<(), RetirementRefusal> {
    match value {
        Value::Number(number) => {
            let lexeme = number.to_string();
            if lexeme.contains(['.', 'e', 'E']) && !lexeme.parse::<f64>().is_ok_and(f64::is_finite)
            {
                return Err(source("retirement input is not finite Python JSON"));
            }
        }
        Value::Array(values) => {
            for value in values {
                finite_python_numbers(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                finite_python_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Frozen Python 3.14 `datetime.fromisoformat(s.replace('Z', '+00:00'))`
/// comparison. Calendar/basic/ISO-week dates, date-only values, a single
/// Unicode separator, reduced clock precision and normalized offset fields
/// retain their observed source behavior. Fractions truncate to microseconds.
/// A mixed naive/aware comparison is invalid, as Python's TypeError is invalid
/// source for this operation. This helper grants no clock/current authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedDateTimeError {
    Invalid,
    Budget,
}

/// Source knowledge-assessment `_instant`: aware inputs only, then UTC
/// normalization. Naive values are invalid even when both operands are naive;
/// UTC normalization outside Python's supported years is also invalid.
/// This compares observations and supplies no trusted clock or current grant.
pub fn observed_instant_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    let start = observed_datetime(start, true)?;
    let end = observed_datetime(end, true)?;
    let upper = year_days(10000) * 86_400_000_000;
    if !start.1 || !end.1 || !(0..upper).contains(&start.0) || !(0..upper).contains(&end.0) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(start.0.cmp(&end.0))
}

/// Elapsed aware UTC microseconds using the same observed source parser.
/// This supplies no trusted clock or authorization.
pub fn observed_instant_elapsed_micros(
    start: &str,
    end: &str,
) -> Result<i128, ObservedDateTimeError> {
    let start = observed_datetime(start, true)?;
    let end = observed_datetime(end, true)?;
    let upper = year_days(10000) * 86_400_000_000;
    if !start.1 || !end.1 || !(0..upper).contains(&start.0) || !(0..upper).contains(&end.0) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(i128::from(end.0) - i128::from(start.0))
}

pub fn observed_datetime_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    observed_datetime_compare(start, end, true)
}

/// Provenance uses direct Python fromisoformat, preserving Z as a possible
/// single date/time separator. Retirement alone applies the global replacement.
pub(crate) fn observed_datetime_raw_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    observed_datetime_compare(start, end, false)
}

fn observed_datetime_compare(
    start: &str,
    end: &str,
    replace_z: bool,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    let start = observed_datetime(start, replace_z)?;
    let end = observed_datetime(end, replace_z)?;
    if start.1 != end.1 {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(start.0.cmp(&end.0))
}

fn observed_datetime(s: &str, replace_z: bool) -> Result<(i64, bool), ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    if s.len() > MAX_EVENT_BYTES {
        return Err(ObservedDateTimeError::Budget);
    }
    // This global replacement is source behavior, including the surprising
    // date-only "...Z" -> naive midnight case. Expansion is bounded by six
    // times the independently capped event input, and no float is allocated.
    let normalized = if replace_z && s.contains('Z') {
        std::borrow::Cow::Owned(s.replace('Z', "+00:00"))
    } else {
        std::borrow::Cow::Borrowed(s)
    };
    let s = normalized.as_ref();
    let b = s.as_bytes();
    let year = digits(b, 0, 4)?;
    if !(1..=9999).contains(&year) {
        return Err(Invalid);
    }
    let (date_len, days) = if b.get(4) == Some(&b'-') && b.get(5) == Some(&b'W') {
        let week = digits(b, 6, 8)?;
        if b.get(8) == Some(&b'-') {
            (10, week_days(year, week, digits(b, 9, 10)?)?)
        } else {
            (8, week_days(year, week, 1)?)
        }
    } else if b.get(4) == Some(&b'W') {
        let week = digits(b, 5, 7)?;
        // CPython resolves the basic week/day versus numeric separator
        // ambiguity by the next character. Preserve its actual choice.
        let has_day = b.get(7).is_some_and(u8::is_ascii_digit)
            && (b.len() == 8 || !b.get(8).is_some_and(u8::is_ascii_digit));
        if has_day {
            (8, week_days(year, week, digits(b, 7, 8)?)?)
        } else {
            (7, week_days(year, week, 1)?)
        }
    } else if b.get(4) == Some(&b'-') {
        if b.get(7) != Some(&b'-') {
            return Err(Invalid);
        }
        (
            10,
            calendar_days(year, digits(b, 5, 7)?, digits(b, 8, 10)?)?,
        )
    } else {
        (8, calendar_days(year, digits(b, 4, 6)?, digits(b, 6, 8)?)?)
    };
    if b.len() == date_len {
        return Ok((days * 86_400_000_000, false));
    }
    // A separator is exactly one Unicode code point, including numeric and
    // non-ASCII separators. Date bytes have already been checked as ASCII.
    let rest = s.get(date_len..).ok_or(Invalid)?;
    let separator = rest.chars().next().ok_or(Invalid)?;
    let time = rest.get(separator.len_utf8()..).ok_or(Invalid)?;
    if time.is_empty() {
        return Err(Invalid);
    }
    let tz_start = time.bytes().position(|c| matches!(c, b'+' | b'-' | b'Z'));
    let (clock, timezone) = match tz_start {
        Some(i) => (&time[..i], Some(&time[i..])),
        None => (time, None),
    };
    let (h, m, sec, micros) = clock_fields(clock)?;
    if h > 23 || m > 59 || sec > 59 {
        return Err(Invalid);
    }
    let local = days * 86_400_000_000 + (h * 3600 + m * 60 + sec) * 1_000_000 + micros;
    let Some(zone) = timezone else {
        return Ok((local, false));
    };
    if zone == "Z" {
        return Ok((local, true));
    }
    let sign = match zone.as_bytes().first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err(Invalid),
    };
    let (h, m, sec, micros) = clock_fields(&zone[1..])?;
    // Unlike wall-clock fields, Python normalizes timezone minute/second
    // values up to 99, then requires the total offset to be below one day.
    let offset = (h * 3600 + m * 60 + sec) * 1_000_000 + micros;
    if offset >= 86_400_000_000 {
        return Err(Invalid);
    }
    Ok((local - sign * offset, true))
}

fn digits(b: &[u8], start: usize, end: usize) -> Result<i64, ObservedDateTimeError> {
    let raw = b.get(start..end).ok_or(ObservedDateTimeError::Invalid)?;
    if !raw.iter().all(u8::is_ascii_digit) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(raw.iter().fold(0, |n, c| n * 10 + i64::from(c - b'0')))
}
fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn year_days(year: i64) -> i64 {
    let y = year - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}
fn calendar_days(y: i64, month: i64, day: i64) -> Result<i64, ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let days_in = |m| match m {
        2 => {
            if leap(y) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&month) || day < 1 || day > days_in(month) {
        return Err(Invalid);
    }
    let mut days = year_days(y) + day - 1;
    for m in 1..month {
        days += days_in(m);
    }
    Ok(days)
}
fn week_days(y: i64, week: i64, day: i64) -> Result<i64, ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let jan4 = year_days(y) + 3;
    let monday = jan4 - jan4 % 7; // Year 1 January 1 was a Monday.
    let next_jan4 = year_days(y + 1) + 3;
    let next_monday = next_jan4 - next_jan4 % 7;
    if !(1..=7).contains(&day) || week < 1 || week > (next_monday - monday) / 7 {
        return Err(Invalid);
    }
    let days = monday + (week - 1) * 7 + day - 1;
    if days < 0 || days >= year_days(10000) {
        return Err(Invalid);
    }
    Ok(days)
}
fn clock_fields(s: &str) -> Result<(i64, i64, i64, i64), ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let b = s.as_bytes();
    let colon = b.get(2) == Some(&b':');
    let mut pos = 0;
    let mut fields = [0; 3];
    let mut count = 0;
    for (i, field) in fields.iter_mut().enumerate() {
        *field = digits(b, pos, pos + 2)?;
        pos += 2;
        count += 1;
        if pos == b.len() {
            break;
        }
        if matches!(b.get(pos), Some(b'.' | b',')) {
            break;
        }
        if i == 2 {
            return Err(Invalid);
        }
        if colon {
            if b.get(pos) != Some(&b':') {
                return Err(Invalid);
            }
            pos += 1;
        }
    }
    let mut micros = 0;
    if pos != b.len() {
        // Python 3.14 only permits fractional *seconds*, not fractional
        // hour/minute forms. The fraction must contain ASCII digits.
        if count != 3 || !matches!(b.get(pos), Some(b'.' | b',')) {
            return Err(Invalid);
        }
        pos += 1;
        let fraction = b.get(pos..).ok_or(Invalid)?;
        if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
            return Err(Invalid);
        }
        for i in 0..6 {
            micros = micros * 10 + fraction.get(i).map_or(0, |c| i64::from(c - b'0'));
        }
    }
    Ok((fields[0], fields[1], fields[2], micros))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_strict_json_matches_duplicate_and_finite_oracle_cases() {
        // Existing tests/test_corpus_source_retirement.py duplicate-field case.
        assert!(
            object(
                br#"{"status":"failed","status":"completed"}"#,
                MAX_EVENT_BYTES
            )
            .is_err()
        );
        assert!(
            object(
                br#"{"st\u0061tus":"failed","status":"completed"}"#,
                MAX_EVENT_BYTES
            )
            .is_err()
        );
        assert!(object(br#"{"number":NaN}"#, MAX_EVENT_BYTES).is_err());
        assert!(object(br#"{"number":1e999}"#, MAX_EVENT_BYTES).is_err());
        assert!(object(br#"{"number":1.7976931348623159e308}"#, MAX_EVENT_BYTES).is_err());
        assert!(object(br#"{"number":1e-9999}"#, MAX_EVENT_BYTES).is_ok());
        assert!(object(b"[]", MAX_EVENT_BYTES).is_err());
        assert!(matches!(
            object(b"{\"a\":1}", 2),
            Err(RetirementRefusal::Budget)
        ));
    }

    #[test]
    fn chronological_order_matches_frozen_python_datetime_behavior_table() {
        use std::cmp::Ordering::{Equal, Less};
        // Direct Python 3.14 fromisoformat comparisons, not RFC3339 assumptions.
        for (a, b, expected) in [
            ("2026-09-14T01:00:00+01:00", "2026-09-14T00:00:00Z", Equal),
            ("2026-09-13T23:59:59Z", "2026-09-14T00:00:00Z", Less),
            ("2026-09-14T00:00:00.09Z", "2026-09-14T00:00:00.1Z", Less),
            (
                "2026-09-14T00:00:00.1234569Z",
                "2026-09-14T00:00:00.123456Z",
                Equal,
            ),
            ("20260914", "2026-09-14T00:00:00", Equal),
            ("2026-09-14Z", "2026-09-14T00:00:00", Equal),
            ("2026-09-14Z+01", "2026-09-14T00:00:00+01:00", Equal),
            ("2026-W39-1", "2026-09-21", Equal),
            ("2026W391X12", "2026-09-21T12", Equal),
            ("2026W39112", "2026-09-21T12", Equal),
            ("2026-W39T00", "2026-09-21", Equal),
            ("20260914𝄞12", "2026-09-14X12:00", Equal),
            ("2026-09-14T12:34:56,5", "2026-09-14T123456.500000", Equal),
            ("2026-09-14T12+0199", "2026-09-14T09:21Z", Equal),
            (
                "2026-09-14T12:00:00+00:00:99",
                "2026-09-14T11:58:21Z",
                Equal,
            ),
            (
                "2026-09-14T12:00:00+00:00:00.1",
                "2026-09-14T11:59:59.9Z",
                Equal,
            ),
            (
                "2026-09-14T12:00:00+000000,5",
                "2026-09-14T11:59:59.5Z",
                Equal,
            ),
        ] {
            assert_eq!(observed_datetime_order(a, b), Ok(expected), "{a} / {b}");
        }
        for invalid in [
            "2026-02-29T00:00:00Z",
            "2026-09-14T00:00:60Z",
            "2026-09-14T00:00:00z",
            "2026-09-14T12.5",
            "2026-09-14T1230.5",
            "2026-09-14T12:3456",
            "2026-09-14T1234:56",
            "2026-09-14T12+24:00",
            "2026W398",
            "2026-W391",
            "2026-09-14Z12",
        ] {
            assert!(observed_datetime(invalid, true).is_err(), "{invalid}");
        }
        assert!(observed_datetime_order("2026-09-14", "2026-09-14T00:00:00Z").is_err());
        assert!(observed_instant_order("2026-09-14", "2026-09-15").is_err());
        assert_eq!(
            observed_instant_order("2026-09-14T01:00:00+01:00", "2026-09-14T00:00:00Z"),
            Ok(std::cmp::Ordering::Equal)
        );
        assert!(
            observed_instant_order("0001-01-01T00:00:00+01:00", "0001-01-01T00:00:00Z").is_err()
        );
        assert_eq!(
            observed_datetime_raw_order("2026-09-14Z12", "2026-09-14T12"),
            Ok(Equal)
        );
        assert!(observed_datetime_raw_order("2026-09-14Z", "2026-09-14").is_err());
        assert!(observed_datetime_raw_order("2026-09-14Z+01", "2026-09-14T00:00:00+01").is_err());
    }
}
