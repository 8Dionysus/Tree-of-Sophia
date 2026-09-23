//! One source-derived, family-local lineage check for adjacent HumanForm sets.
//!
//! This is deliberately a shadow result. It does not check schema resources,
//! source-copy materialization, cross-subject ID allocation, owner authority,
//! or the complete source publication snapshot. It cannot create a
//! `ValidationOutcome::MechanicallyValid` or CMD attestation.

use std::collections::{BTreeMap, BTreeSet};

use tos_foundation::{
    CanonicalProfile, Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind,
    JsonValue, canonical_bytes_v1, parse_json,
};

const MAX_SET_BYTES: usize = 2_097_152;
const MAX_RECORD_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageError {
    BudgetExceeded,
    InvalidPublishedJson(FoundationErrorCode),
    Malformed,
    UnsupportedProfile,
    Cardinality,
    DuplicateVersion,
    MixedSubject,
    DuplicateCurrent,
    InitialHasPredecessor,
    BrokenPredecessor,
    OrphanHistory,
    CurrentNotLatest,
    DuplicateCommand,
    ReceiptSourceMismatch,
    ReceiptResultMissing,
}

/// A family-local observation, expressly incomplete for source admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowLineage {
    pub raw_set_digest: Digest256,
    pub subject_id: String,
    pub current_forms: usize,
    pub prior_forms: usize,
    pub growth_receipts: usize,
    pub current_ids: BTreeSet<String>,
    pub current_refs: Vec<ObservedFormRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedFormRef {
    pub id: String,
    pub version: String,
    pub digest: String,
}

fn member<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue, LineageError> {
    value.object_get(name).ok_or(LineageError::Malformed)
}

fn string(value: &JsonValue) -> Result<&str, LineageError> {
    value.as_str().ok_or(LineageError::Malformed)
}

fn array(value: &JsonValue) -> Result<&[JsonValue], LineageError> {
    value.as_array().ok_or(LineageError::Malformed)
}

fn positive_integer(value: &JsonValue) -> Result<String, LineageError> {
    let JsonValue::Number(number) = value else {
        return Err(LineageError::Malformed);
    };
    if number.kind != JsonNumberKind::Int || number.lexeme.starts_with('-') || number.lexeme == "0"
    {
        return Err(LineageError::Malformed);
    }
    Ok(number.lexeme.clone())
}

fn previous_version(version: &str) -> Option<String> {
    if version == "1" {
        return None;
    }
    let mut digits = version.as_bytes().to_vec();
    for digit in digits.iter_mut().rev() {
        if *digit > b'0' {
            *digit -= 1;
            break;
        }
        *digit = b'9';
    }
    if digits.first() == Some(&b'0') {
        digits.remove(0);
    }
    String::from_utf8(digits).ok()
}

fn reference(value: &JsonValue) -> Result<ObservedFormRef, LineageError> {
    Ok(ObservedFormRef {
        id: string(member(value, "id")?)?.to_owned(),
        version: positive_integer(member(value, "version")?)?,
        digest: string(member(value, "digest")?)?.to_owned(),
    })
}

fn record_ref(form: &JsonValue) -> Result<ObservedFormRef, LineageError> {
    let limits = JsonLimits::new(MAX_RECORD_BYTES, 64, 300_000, 4_300)
        .map_err(|_| LineageError::BudgetExceeded)?;
    let canonical = canonical_bytes_v1(form, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                LineageError::BudgetExceeded
            } else {
                LineageError::InvalidPublishedJson(error.code)
            }
        })?;
    Ok(ObservedFormRef {
        id: string(member(form, "form_id")?)?.to_owned(),
        version: positive_integer(member(form, "form_version")?)?,
        digest: Digest256::of_bytes(&canonical).to_prefixed(),
    })
}

/// Reproduce the mechanical chain/receipt core of `source_commands._validate_history`
/// on exact raw set bytes. Schema and timestamp gates still belong to later
/// rule modules; this result is not the complete Python function's judgment.
pub fn inspect_lineage_raw(raw: &[u8]) -> Result<ShadowLineage, LineageError> {
    let limits = JsonLimits::new(MAX_SET_BYTES, 64, 300_000, 4_300)
        .map_err(|_| LineageError::BudgetExceeded)?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            LineageError::BudgetExceeded
        } else {
            LineageError::InvalidPublishedJson(error.code)
        }
    })?;
    let set = document.root();
    if string(member(set, "schema_version")?)? != "tos_human_form_set_v1" {
        return Err(LineageError::UnsupportedProfile);
    }
    let subject_id = string(member(member(set, "subject")?, "id")?)?.to_owned();
    let current = array(member(set, "forms")?)?;
    let prior = array(member(set, "prior_forms")?)?;
    let receipts = match set.object_get("growth_history") {
        Some(value) => array(value)?,
        None => &[],
    };
    if current.is_empty() || current.len() > 32 || prior.len() > 256 || receipts.len() > 256 {
        return Err(LineageError::Cardinality);
    }
    let mut indexed = BTreeMap::<(String, String), (&JsonValue, ObservedFormRef)>::new();
    let mut current_ids = BTreeSet::new();
    for form in prior.iter().chain(current) {
        if string(member(form, "schema_version")?)? != "tos_human_form_v1" {
            return Err(LineageError::UnsupportedProfile);
        }
        let form_ref = record_ref(form)?;
        let embedded_subject = string(member(member(form, "subject")?, "id")?)?;
        if embedded_subject != subject_id {
            return Err(LineageError::MixedSubject);
        }
        let key = (form_ref.id.clone(), form_ref.version.clone());
        if indexed.insert(key, (form, form_ref)).is_some() {
            return Err(LineageError::DuplicateVersion);
        }
    }
    let mut current_refs = Vec::with_capacity(current.len());
    for form in current {
        let id = string(member(form, "form_id")?)?.to_owned();
        if !current_ids.insert(id) {
            return Err(LineageError::DuplicateCurrent);
        }
        current_refs.push(record_ref(form)?);
    }
    for ((id, version), (form, _)) in &indexed {
        let revises = member(form, "revises")?;
        match previous_version(version) {
            None if revises != &JsonValue::Null => {
                return Err(LineageError::InitialHasPredecessor);
            }
            Some(previous) => {
                let expected = indexed
                    .get(&(id.clone(), previous))
                    .ok_or(LineageError::BrokenPredecessor)?;
                if reference(revises)? != expected.1 {
                    return Err(LineageError::BrokenPredecessor);
                }
            }
            None => {}
        }
        if !current_ids.contains(id) {
            return Err(LineageError::OrphanHistory);
        }
    }
    for form in current {
        let id = string(member(form, "form_id")?)?;
        let version = positive_integer(member(form, "form_version")?)?;
        // Same-ID versions are arbitrary precision decimal integers. Their
        // length then lexicographic spelling orders positive canonical ints.
        if indexed.keys().any(|(other_id, other_version)| {
            other_id == id
                && (other_version.len(), other_version.as_str()) > (version.len(), version.as_str())
        }) {
            return Err(LineageError::CurrentNotLatest);
        }
    }
    let mut commands = BTreeSet::new();
    for receipt in receipts {
        let command_id = string(member(receipt, "command_id")?)?;
        if !commands.insert(command_id) {
            return Err(LineageError::DuplicateCommand);
        }
        if string(member(member(receipt, "source")?, "id")?)? != subject_id {
            return Err(LineageError::ReceiptSourceMismatch);
        }
        for item in array(member(receipt, "results")?)? {
            let result = reference(item)?;
            let key = (result.id.clone(), result.version.clone());
            if indexed
                .get(&key)
                .is_none_or(|(_, actual)| *actual != result)
            {
                return Err(LineageError::ReceiptResultMissing);
            }
        }
    }
    Ok(ShadowLineage {
        raw_set_digest: Digest256::of_bytes(raw),
        subject_id,
        current_forms: current.len(),
        prior_forms: prior.len(),
        growth_receipts: receipts.len(),
        current_ids,
        current_refs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormatProfile, SchemaBackendProbe, SchemaResource};
    use serde_json::Value;

    const WORK_FORMS: &[u8] = include_bytes!("../tests/fixtures/jgb-work.human-forms.json");
    const RECEIPT_FORMS: &[u8] = include_bytes!("../tests/fixtures/scoped-work.human-forms.json");

    fn mutate(raw: &[u8], operation: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut value: Value = serde_json::from_slice(raw).expect("frozen fixture");
        operation(&mut value);
        serde_json::to_vec(&value).expect("mutated fixture")
    }

    #[test]
    fn pinned_python_oracle_baselines_and_record_digest() {
        let work = inspect_lineage_raw(WORK_FORMS).expect("Python oracle accepted Work set");
        assert_eq!(
            work.raw_set_digest.to_hex(),
            "7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b"
        );
        assert_eq!(work.current_forms, 3);
        assert_eq!(
            work.current_refs[0].digest,
            "sha256:79cfb97a52bef0e52c7728dcd41a568dba0426aad8c955a5b32a137762d2469d"
        );
        let receipts =
            inspect_lineage_raw(RECEIPT_FORMS).expect("Python oracle accepted receipt set");
        assert_eq!(
            receipts.raw_set_digest.to_hex(),
            "412bd2cd80e17dd3e8a2fc43e375a6655f16502090ba024e9ee5bd2760c2b490"
        );
        assert_eq!(receipts.growth_receipts, 1);
    }

    #[test]
    fn pinned_python_oracle_negative_lineage_cases() {
        let duplicate = mutate(WORK_FORMS, |set| {
            let form = set["forms"][0].clone();
            set["forms"].as_array_mut().unwrap().push(form);
        });
        assert!(matches!(
            inspect_lineage_raw(&duplicate),
            Err(LineageError::DuplicateVersion)
        ));

        let predecessor = mutate(WORK_FORMS, |set| {
            set["forms"][0]["revises"]["digest"] =
                Value::String(format!("sha256:{}", "0".repeat(64)));
        });
        assert_eq!(
            inspect_lineage_raw(&predecessor),
            Err(LineageError::BrokenPredecessor)
        );

        let subject = mutate(WORK_FORMS, |set| {
            set["prior_forms"][0]["subject"]["id"] = Value::String("tos.work.other".into());
        });
        assert_eq!(
            inspect_lineage_raw(&subject),
            Err(LineageError::MixedSubject)
        );

        let receipt = mutate(RECEIPT_FORMS, |set| {
            set["growth_history"][0]["results"][0]["digest"] =
                Value::String(format!("sha256:{}", "0".repeat(64)));
        });
        assert_eq!(
            inspect_lineage_raw(&receipt),
            Err(LineageError::ReceiptResultMissing)
        );

        let repeated_command = mutate(RECEIPT_FORMS, |set| {
            let first = set["growth_history"][0].clone();
            set["growth_history"].as_array_mut().unwrap().push(first);
        });
        assert_eq!(
            inspect_lineage_raw(&repeated_command),
            Err(LineageError::DuplicateCommand)
        );
    }

    #[test]
    fn strict_raw_duplicate_member_and_byte_budget_fail_closed() {
        let duplicated = br#"{"subject":{"id":"x","id":"y"},"forms":[],"prior_forms":[]}"#;
        assert_eq!(
            inspect_lineage_raw(duplicated),
            Err(LineageError::InvalidPublishedJson(
                FoundationErrorCode::DuplicateMember
            ))
        );
        assert_eq!(
            inspect_lineage_raw(&vec![b' '; MAX_SET_BYTES + 1]),
            Err(LineageError::BudgetExceeded)
        );
        let next_profile = mutate(WORK_FORMS, |set| {
            set["schema_version"] = Value::String("tos_human_form_set_v2".into());
        });
        assert_eq!(
            inspect_lineage_raw(&next_profile),
            Err(LineageError::UnsupportedProfile)
        );
    }

    #[test]
    fn source_schema_accepts_pinned_forms_and_rejects_invalid_version() {
        let resources = [
            (
                "knowledge-assessment.schema.json",
                include_bytes!("../../../../ToS/contracts/knowledge-assessment.schema.json")
                    .as_slice(),
            ),
            (
                "human-form.schema.json",
                include_bytes!("../../../../ToS/contracts/human-form.schema.json").as_slice(),
            ),
            (
                "human-form-set.schema.json",
                include_bytes!("../../../../ToS/contracts/human-form-set.schema.json").as_slice(),
            ),
        ];
        let resources = resources.into_iter().map(|(name, raw)| SchemaResource {
            uri: format!("https://treeofsophia.local/ToS/contracts/{name}"),
            raw: raw.to_vec(),
        });
        let probe = SchemaBackendProbe::new(resources, FormatProfile::LegacyPythonObserved20260923)
            .expect("declared schema closure");
        let root = "https://treeofsophia.local/ToS/contracts/human-form-set.schema.json";
        assert_eq!(probe.is_valid_raw(root, WORK_FORMS), Ok(true));
        let invalid = mutate(WORK_FORMS, |set| {
            set["forms"][0]["form_version"] = Value::Number(0.into());
        });
        assert_eq!(probe.is_valid_raw(root, &invalid), Ok(false));
    }
}
