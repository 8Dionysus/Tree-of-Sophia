//! Source retirement operation and exact membership-only transition.
//! This is the frozen Python row 14 mechanical rule, not source acceptance.
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
            .check(event_ref, &raw, SCHEMA, limits.deadline, cancelled)
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
        if timestamp(text(&event["ended_at"])?)? < timestamp(text(&event["started_at"])?)? {
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
        _ => source("retirement input is not strict finite JSON"),
    })?;
    if !value.is_object() {
        return Err(source("retirement input must be JSON object"));
    }
    Ok(value)
}

// Compare UTC seconds plus exact fractional digits. No lexical ordering of
// offset timestamps and no float rounding. Leap seconds are outside Python's
// datetime profile and refuse; schema format acceptance alone is insufficient.
fn timestamp(s: &str) -> Result<(i64, String), RetirementRefusal> {
    let b = s.as_bytes();
    let bad = || {
        RetirementRefusal::Unsupported(
            "retirement date-time is outside supported Python datetime profile".into(),
        )
    };
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return Err(bad());
    }
    let number = |a: usize, z: usize| -> Result<i64, RetirementRefusal> {
        let v = b.get(a..z).ok_or_else(bad)?;
        if !v.iter().all(u8::is_ascii_digit) {
            return Err(bad());
        }
        Ok(v.iter().fold(0, |n, c| n * 10 + i64::from(c - b'0')))
    };
    let y = number(0, 4)?;
    let m = number(5, 7)?;
    let d = number(8, 10)?;
    let h = number(11, 13)?;
    let min = number(14, 16)?;
    let sec = number(17, 19)?;
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = match m {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return Err(bad()),
    };
    if y == 0 || d == 0 || d > month_days || h > 23 || min > 59 || sec > 59 {
        return Err(bad());
    }
    let mut i = 19;
    let mut fraction = String::new();
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return Err(bad());
        }
        fraction = s[start..i].trim_end_matches('0').to_owned();
    }
    // Python datetime truncates finer-than-microsecond fractions.
    if fraction.len() > 6 {
        fraction.truncate(6);
        fraction = fraction.trim_end_matches('0').to_owned();
    }
    // Right padding gives fixed-width exact comparison even for .1 versus .09.
    while fraction.len() < 6 {
        fraction.push('0');
    }
    let offset = match b.get(i) {
        Some(b'Z') if i + 1 == b.len() => 0,
        Some(sign @ (b'+' | b'-')) if i + 6 == b.len() && b[i + 3] == b':' => {
            let hh = number(i + 1, i + 3)?;
            let mm = number(i + 4, i + 6)?;
            if hh > 23 || mm > 59 {
                return Err(bad());
            }
            (hh * 3600 + mm * 60) * if *sign == b'+' { 1 } else { -1 }
        }
        _ => return Err(bad()),
    };
    // Gregorian days relative to year 1; only ordering, not an epoch, matters.
    let prior = y - 1;
    let mut days = prior * 365 + prior / 4 - prior / 100 + prior / 400;
    for month in 1..m {
        days += match month {
            2 => {
                if leap {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
    }
    days += d - 1;
    Ok((days * 86400 + h * 3600 + min * 60 + sec - offset, fraction))
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
        assert!(object(b"[]", MAX_EVENT_BYTES).is_err());
        assert!(matches!(
            object(b"{\"a\":1}", 2),
            Err(RetirementRefusal::Budget)
        ));
    }

    #[test]
    fn chronological_order_uses_python_microseconds_and_utc_offsets() {
        // Python datetime.fromisoformat comparisons, independently computed.
        assert_eq!(
            timestamp("2026-09-14T01:00:00+01:00").unwrap(),
            timestamp("2026-09-14T00:00:00Z").unwrap()
        );
        assert!(
            timestamp("2026-09-13T23:59:59Z").unwrap() < timestamp("2026-09-14T00:00:00Z").unwrap()
        );
        assert!(
            timestamp("2026-09-14T00:00:00.09Z").unwrap()
                < timestamp("2026-09-14T00:00:00.1Z").unwrap()
        );
        assert_eq!(
            timestamp("2026-09-14T00:00:00.1234569Z").unwrap(),
            timestamp("2026-09-14T00:00:00.123456Z").unwrap()
        );
        assert!(timestamp("2024-02-29T00:00:00Z").is_ok());
        assert!(timestamp("2026-02-29T00:00:00Z").is_err());
        assert!(timestamp("2026-09-14T00:00:60Z").is_err());
        assert!(timestamp("2026-09-14T00:00:00z").is_err());
    }
}
