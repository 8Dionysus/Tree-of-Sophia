//! Local proposal stage/import digest gate. The digest is a trace marker,
//! never a signature, source assessment, rights grant or canon admission.

use std::collections::HashSet;
use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};

const MAX_BYTES: usize = 1_000_000;
const MAX_REVISION: u64 = 1_000_000_000;
const REQUIRED: &[&str] = &[
    "id",
    "kind",
    "parent_hypothesis_id",
    "statement",
    "source_refs",
    "evidence_refs",
    "confidence_posture",
    "actor_origin",
    "base_page_revision",
    "base_workspace_revision",
    "data_fingerprint",
    "created_at",
    "local_only",
    "review_status",
    "canon",
];
const OPTIONAL: &[&str] = &[
    "target_id",
    "from_id",
    "to_id",
    "review_requirement",
    "digest",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceProposalErrorCode {
    InvalidInput,
    InvalidProposal,
    StaleRevision,
    MissingParent,
    DuplicateProposal,
    DigestMismatch,
    OutputBudget,
}
impl WorkspaceProposalErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidProposal => "invalid_proposal",
            Self::StaleRevision => "stale_revision",
            Self::MissingParent => "missing_parent",
            Self::DuplicateProposal => "duplicate_proposal",
            Self::DigestMismatch => "digest_mismatch",
            Self::OutputBudget => "output_budget",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceProposalError {
    pub code: WorkspaceProposalErrorCode,
}
impl WorkspaceProposalError {
    const fn new(code: WorkspaceProposalErrorCode) -> Self {
        Self { code }
    }
}

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}
fn string<'a>(value: &'a JsonValue, name: &str) -> Option<&'a str> {
    field(value, name)?.as_str()
}
fn invalid() -> WorkspaceProposalError {
    WorkspaceProposalError::new(WorkspaceProposalErrorCode::InvalidProposal)
}
fn js_trim(ch: char) -> bool {
    matches!(ch, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}'
        | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn exact_keys(value: &JsonValue, required: &[&str], optional: &[&str]) -> bool {
    let Some(entries) = value.as_object() else {
        return false;
    };
    required.iter().all(|name| field(value, name).is_some())
        && entries.iter().all(|(name, _)| {
            name.as_str()
                .is_some_and(|name| required.contains(&name) || optional.contains(&name))
        })
}
fn bounded<'a>(
    value: &'a JsonValue,
    name: &str,
    max: usize,
) -> Result<&'a str, WorkspaceProposalError> {
    let item = string(value, name).ok_or_else(invalid)?;
    if item.is_empty() || item.trim_matches(js_trim) != item || item.encode_utf16().count() > max {
        return Err(invalid());
    }
    Ok(item)
}
fn optional_bounded<'a>(
    value: &'a JsonValue,
    name: &str,
    max: usize,
) -> Result<Option<&'a str>, WorkspaceProposalError> {
    match field(value, name) {
        None => Ok(None),
        Some(_) => bounded(value, name, max).map(Some),
    }
}
fn revision(value: &JsonValue, name: &str) -> Result<u64, WorkspaceProposalError> {
    field(value, name)
        .and_then(JsonValue::as_u64)
        .filter(|number| *number <= MAX_REVISION)
        .ok_or_else(invalid)
}
fn refs(value: &JsonValue, name: &str) -> Result<(), WorkspaceProposalError> {
    let items = field(value, name)
        .and_then(JsonValue::as_array)
        .ok_or_else(invalid)?;
    if items.is_empty() || items.len() > 256 {
        return Err(invalid());
    }
    let mut seen = HashSet::new();
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(invalid());
        };
        if text.is_empty()
            || text.trim_matches(js_trim) != text
            || text.encode_utf16().count() > 1024
            || !seen.insert(text)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn contains_id(
    request: &JsonValue,
    field_name: &str,
    id: &str,
) -> Result<bool, WorkspaceProposalError> {
    let items = field(request, field_name)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| WorkspaceProposalError::new(WorkspaceProposalErrorCode::InvalidInput))?;
    if items.len() > 256 || items.iter().any(|item| item.as_str().is_none()) {
        return Err(WorkspaceProposalError::new(
            WorkspaceProposalErrorCode::InvalidInput,
        ));
    }
    Ok(items.iter().any(|item| item.as_str() == Some(id)))
}
fn canonical_utc(value: &str) -> bool {
    let (year, tail) = if value.starts_with('+') || value.starts_with('-') {
        if value.len() != 27 {
            return false;
        }
        (value.get(0..7), value.get(7..))
    } else {
        if value.len() != 24 {
            return false;
        }
        (value.get(0..4), value.get(4..))
    };
    let (Some(year), Some(tail)) = (year, tail) else {
        return false;
    };
    if !year
        .trim_start_matches(|c| c == '+' || c == '-')
        .bytes()
        .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let Ok(year_number) = year.parse::<i32>() else {
        return false;
    };
    if year.starts_with('+') && year_number < 10_000
        || year.starts_with('-') && year_number >= 0
        || year_number.abs() > 275_760
    {
        return false;
    }
    let b = tail.as_bytes();
    if b.len() != 20
        || b[0] != b'-'
        || b[3] != b'-'
        || b[6] != b'T'
        || b[9] != b':'
        || b[12] != b':'
        || b[15] != b'.'
        || b[19] != b'Z'
    {
        return false;
    }
    let part = |start, end| -> Option<u32> {
        tail.get(start..end)
            .filter(|text| text.bytes().all(|b| b.is_ascii_digit()))?
            .parse()
            .ok()
    };
    let (Some(month), Some(day), Some(hour), Some(minute), Some(second), Some(millis)) = (
        part(1, 3),
        part(4, 6),
        part(7, 9),
        part(10, 12),
        part(13, 15),
        part(16, 19),
    ) else {
        return false;
    };
    if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    let leap = year_number % 4 == 0 && (year_number % 100 != 0 || year_number % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days).contains(&day) {
        return false;
    }
    // JS Date TimeClip accepts only an exact UTC millisecond in
    // [-8.64e15, 8.64e15]. The shape/calendar check alone would admit, for
    // example, +275760-12-31, while the maintained TS rule rejects it.
    let mut year = i64::from(year_number);
    let month = i64::from(month);
    year -= i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let year_of_era_day = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146_097 + year_of_era_day - 719_468;
    let milliseconds = days_since_epoch * 86_400_000
        + i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1_000
        + i64::from(millis);
    (-8_640_000_000_000_000..=8_640_000_000_000_000).contains(&milliseconds)
}
fn sort_keys(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(sort_keys).collect()),
        JsonValue::Object(entries) => {
            let mut items: Vec<(JsonString, JsonValue)> = entries
                .iter()
                .map(|(key, item)| (key.clone(), sort_keys(item)))
                .collect();
            items.sort_by(|(left, _), (right, _)| left.units().cmp(right.units()));
            JsonValue::Object(items)
        }
        _ => value.clone(),
    }
}
fn proposal_digest(value: &JsonValue) -> Result<String, WorkspaceProposalError> {
    let unsigned = value
        .without_top_field("digest")
        .unwrap_or_else(|_| value.clone());
    let bytes = emit_value_preserved_json(
        &sort_keys(&unsigned),
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| WorkspaceProposalError::new(WorkspaceProposalErrorCode::OutputBudget))?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

/// The host supplies an already normalized v1 proposal packet. `stage`
/// checks the workspace revision and current IDs; `verify` checks a retained
/// proposal without relabeling old pending-human-review history. Whole-session
/// import/undo/redo and LocalStorage require a separate state-machine ABI.
pub fn workspace_proposal_digest_v1(raw: &[u8]) -> Result<String, WorkspaceProposalError> {
    let document = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: MAX_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| WorkspaceProposalError::new(WorkspaceProposalErrorCode::InvalidInput))?;
    let request = document.root();
    let operation = string(request, "operation")
        .ok_or_else(|| WorkspaceProposalError::new(WorkspaceProposalErrorCode::InvalidInput))?;
    if operation != "stage" && operation != "verify" {
        return Err(WorkspaceProposalError::new(
            WorkspaceProposalErrorCode::InvalidInput,
        ));
    }
    let proposal = field(request, "proposal")
        .ok_or_else(|| WorkspaceProposalError::new(WorkspaceProposalErrorCode::InvalidInput))?;
    if !exact_keys(proposal, REQUIRED, OPTIONAL) {
        return Err(invalid());
    }
    let id = bounded(proposal, "id", 256)?;
    let parent = bounded(proposal, "parent_hypothesis_id", 256)?;
    let kind = bounded(proposal, "kind", 256)?;
    if ![
        "relation",
        "interpretation",
        "metadata_correction",
        "source_route",
        "concept_enrichment",
    ]
    .contains(&kind)
    {
        return Err(invalid());
    }
    let target = optional_bounded(proposal, "target_id", 256)?;
    let from = optional_bounded(proposal, "from_id", 256)?;
    let to = optional_bounded(proposal, "to_id", 256)?;
    if from.is_some() != to.is_some()
        || (["relation", "source_route"].contains(&kind) && from.is_none())
        || ([
            "interpretation",
            "metadata_correction",
            "concept_enrichment",
        ]
        .contains(&kind)
            && target.is_none())
    {
        return Err(invalid());
    }
    bounded(proposal, "statement", 4000)?;
    refs(proposal, "source_refs")?;
    refs(proposal, "evidence_refs")?;
    let confidence = field(proposal, "confidence_posture").ok_or_else(invalid)?;
    if !exact_keys(confidence, &["value", "meaning"], &[])
        || !matches!(
            string(confidence, "value"),
            Some("unknown" | "low" | "medium" | "high")
        )
        || string(confidence, "meaning") != Some("maker_declared_uncertainty_not_truth_probability")
    {
        return Err(invalid());
    }
    if !matches!(string(proposal, "actor_origin"), Some("human" | "agent")) {
        return Err(invalid());
    }
    revision(proposal, "base_page_revision")?;
    let base = revision(proposal, "base_workspace_revision")?;
    bounded(proposal, "data_fingerprint", 1024)?;
    let created = bounded(proposal, "created_at", 64)?;
    if !canonical_utc(created) {
        return Err(invalid());
    }
    if field(proposal, "local_only").and_then(JsonValue::as_bool) != Some(true)
        || field(proposal, "canon").and_then(JsonValue::as_bool) != Some(false)
    {
        return Err(invalid());
    }
    let status = string(proposal, "review_status").ok_or_else(invalid)?;
    if !["pending_review", "pending_human_review"].contains(&status)
        || status == "pending_review"
            && string(proposal, "review_requirement") != Some("human_or_authorized_agent")
        || field(proposal, "review_requirement").is_some()
            && string(proposal, "review_requirement") != Some("human_or_authorized_agent")
    {
        return Err(invalid());
    }
    if !contains_id(request, "hypothesis_ids", parent)? {
        return Err(WorkspaceProposalError::new(
            WorkspaceProposalErrorCode::MissingParent,
        ));
    }
    match operation {
        "stage" => {
            if field(proposal, "digest").is_some() {
                return Err(invalid());
            }
            let current = revision(request, "current_revision")?;
            if base != current {
                return Err(WorkspaceProposalError::new(
                    WorkspaceProposalErrorCode::StaleRevision,
                ));
            }
            if contains_id(request, "existing_proposal_ids", id)? {
                return Err(WorkspaceProposalError::new(
                    WorkspaceProposalErrorCode::DuplicateProposal,
                ));
            }
            proposal_digest(proposal)
        }
        "verify" => {
            let supplied = string(proposal, "digest").ok_or_else(invalid)?;
            let expected = proposal_digest(proposal)?;
            if supplied != expected {
                return Err(WorkspaceProposalError::new(
                    WorkspaceProposalErrorCode::DigestMismatch,
                ));
            }
            Ok(expected)
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROPOSAL: &str = r#"{"id":"proposal:one","kind":"interpretation","parent_hypothesis_id":"hyp:one","target_id":"edge:one","statement":"Another reading.","source_refs":["source:one"],"evidence_refs":["evidence:one"],"confidence_posture":{"value":"low","meaning":"maker_declared_uncertainty_not_truth_probability"},"actor_origin":"agent","base_page_revision":7,"base_workspace_revision":1,"data_fingerprint":"sha256:fixture","created_at":"2026-09-23T12:00:00.000Z","local_only":true,"review_status":"pending_review","review_requirement":"human_or_authorized_agent","canon":false}"#;
    #[test]
    fn stage_matches_independent_browser_digest() {
        let request = format!(
            r#"{{"operation":"stage","current_revision":1,"hypothesis_ids":["hyp:one"],"existing_proposal_ids":[],"proposal":{PROPOSAL}}}"#
        );
        assert_eq!(
            workspace_proposal_digest_v1(request.as_bytes()).unwrap(),
            "fnv1a64:602af812971bbce6"
        );
    }
    #[test]
    fn stale_parent_duplicate_and_tamper_fail() {
        let stage = format!(
            r#"{{"operation":"stage","current_revision":2,"hypothesis_ids":["hyp:one"],"existing_proposal_ids":[],"proposal":{PROPOSAL}}}"#
        );
        assert_eq!(
            workspace_proposal_digest_v1(stage.as_bytes())
                .unwrap_err()
                .code,
            WorkspaceProposalErrorCode::StaleRevision
        );
        let absent = format!(
            r#"{{"operation":"stage","current_revision":1,"hypothesis_ids":[],"existing_proposal_ids":[],"proposal":{PROPOSAL}}}"#
        );
        assert_eq!(
            workspace_proposal_digest_v1(absent.as_bytes())
                .unwrap_err()
                .code,
            WorkspaceProposalErrorCode::MissingParent
        );
        let duplicate = format!(
            r#"{{"operation":"stage","current_revision":1,"hypothesis_ids":["hyp:one"],"existing_proposal_ids":["proposal:one"],"proposal":{PROPOSAL}}}"#
        );
        assert_eq!(
            workspace_proposal_digest_v1(duplicate.as_bytes())
                .unwrap_err()
                .code,
            WorkspaceProposalErrorCode::DuplicateProposal
        );
        let signed = PROPOSAL.replace(
            "\"canon\":false",
            "\"canon\":false,\"digest\":\"fnv1a64:602af812971bbce6\"",
        );
        let verify =
            format!(r#"{{"operation":"verify","hypothesis_ids":["hyp:one"],"proposal":{signed}}}"#);
        assert_eq!(
            workspace_proposal_digest_v1(verify.as_bytes()).unwrap(),
            "fnv1a64:602af812971bbce6"
        );
        let tampered = verify.replace("Another reading.", "Tampered reading.");
        assert_eq!(
            workspace_proposal_digest_v1(tampered.as_bytes())
                .unwrap_err()
                .code,
            WorkspaceProposalErrorCode::DigestMismatch
        );
    }

    #[test]
    fn retained_pending_human_review_digest_is_not_rewritten() {
        let legacy = PROPOSAL
            .replace(",\"review_requirement\":\"human_or_authorized_agent\"", "")
            .replace("\"pending_review\"", "\"pending_human_review\"")
            .replace(
                "\"canon\":false",
                "\"canon\":false,\"digest\":\"fnv1a64:8199d8cd9ebba5ad\"",
            );
        let verify =
            format!(r#"{{"operation":"verify","hypothesis_ids":["hyp:one"],"proposal":{legacy}}}"#);
        assert_eq!(
            workspace_proposal_digest_v1(verify.as_bytes()).unwrap(),
            "fnv1a64:8199d8cd9ebba5ad"
        );
        let promoted = verify.replace("\"canon\":false", "\"canon\":true");
        assert_eq!(
            workspace_proposal_digest_v1(promoted.as_bytes())
                .unwrap_err()
                .code,
            WorkspaceProposalErrorCode::InvalidProposal
        );
    }

    #[test]
    fn canonical_utc_obeys_javascript_timeclip_edges() {
        assert!(canonical_utc("+275760-09-13T00:00:00.000Z"));
        assert!(!canonical_utc("+275760-09-13T00:00:00.001Z"));
        assert!(!canonical_utc("+275760-12-31T00:00:00.000Z"));
        assert!(canonical_utc("-271821-04-20T00:00:00.000Z"));
        assert!(!canonical_utc("-271821-04-19T23:59:59.999Z"));
    }
}
