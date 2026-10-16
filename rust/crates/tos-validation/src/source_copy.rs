//! Corpus-record source-copy readiness shadow for the first public forms command.
//!
//! This reproduces bounded field/context observations from the current
//! metadata adapter. It does not run the required bounded schema worker,
//! receipt-time check, global form-ID universe, rights/current authority, or
//! the whole source audit. `CompatibleCore` is only a local observation.

use serde_json::Value;
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationErrorCode, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};

use crate::{SchemaProbeError, published_value, source_forms::inspect_lineage_raw};

const MAX_SOURCE_BYTES: usize = 1_048_576;
const MAX_SET_BYTES: usize = 2_097_152;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyShadowState {
    CompatibleCore,
    Invalid,
    Unavailable,
    Stale,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyShadowError {
    BudgetExceeded,
    InvalidPublishedJson(FoundationErrorCode),
    IncompatibleJson,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyShadow {
    pub state: CopyShadowState,
    pub source_raw_digest: Digest256,
    pub form_set_raw_digest: Digest256,
    pub selected_form_id: String,
    pub observed_pointers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactRef {
    id: String,
    version: String,
    digest: String,
}

fn positive_version(value: &Value) -> Option<String> {
    let text = value.as_number()?.to_string();
    if text == "0"
        || text.starts_with('-')
        || text.starts_with('0')
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(text)
}

fn exact_ref(value: &Value) -> Option<ExactRef> {
    let object = value.as_object()?;
    Some(ExactRef {
        id: object.get("id")?.as_str()?.to_owned(),
        version: positive_version(object.get("version")?)?,
        digest: object.get("digest")?.as_str()?.to_owned(),
    })
}

fn map_probe_error(error: SchemaProbeError) -> CopyShadowError {
    match error {
        SchemaProbeError::BudgetExceeded => CopyShadowError::BudgetExceeded,
        SchemaProbeError::InvalidPublishedJson(code) => CopyShadowError::InvalidPublishedJson(code),
        _ => CopyShadowError::IncompatibleJson,
    }
}

fn source_ref(raw: &[u8], source: &Value) -> Result<Option<ExactRef>, CopyShadowError> {
    let Some(id) = source.get("record_id").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(version) = source.get("record_version").and_then(positive_version) else {
        return Ok(None);
    };
    let limits = JsonLimits::new(MAX_SOURCE_BYTES, 64, 300_000, 4_300)
        .map_err(|_| CopyShadowError::BudgetExceeded)?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            CopyShadowError::BudgetExceeded
        } else {
            CopyShadowError::InvalidPublishedJson(error.code)
        }
    })?;
    let canonical = canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            CopyShadowError::BudgetExceeded
        } else {
            CopyShadowError::InvalidPublishedJson(error.code)
        }
    })?;
    Ok(Some(ExactRef {
        id: id.to_owned(),
        version,
        digest: Digest256::of_bytes(&canonical).to_prefixed(),
    }))
}

fn pointer<'a>(source: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(source);
    }
    let mut value = source;
    for token in path.strip_prefix('/')?.split('/') {
        let token = token.replace("~1", "/").replace("~0", "~");
        value = match value {
            Value::Object(object) => object.get(&token)?,
            Value::Array(array)
                if token.bytes().all(|byte| byte.is_ascii_digit())
                    && (token == "0" || !token.starts_with('0')) =>
            {
                array.get(token.parse::<usize>().ok()?)?
            }
            _ => return None,
        };
    }
    Some(value)
}

struct Field {
    pointer: String,
    role: &'static str,
    language: Option<String>,
    script: Option<String>,
    context: Vec<String>,
}

fn language_pair(value: Option<&Value>) -> (Option<String>, Option<String>) {
    let value = value.and_then(Value::as_object);
    (
        value
            .and_then(|object| object.get("language"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        value
            .and_then(|object| object.get("script"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
}

fn metadata_fields(source: &Value) -> Option<Vec<Field>> {
    let object = source.as_object()?;
    let mut context = Vec::new();
    for field in [
        "identity_status",
        "same_as_posture",
        "semantic_scope",
        "semantic_content",
        "form_identity",
        "native_text_binding",
    ] {
        if object.contains_key(field) {
            context.push(format!("/{field}"));
        }
    }
    if object.get("record_type").and_then(Value::as_str) == Some("sign") {
        context.push("/promotion_basis".to_owned());
    }
    // This versioned corpus-record adapter does not inherit native artifact,
    // canonical-node, Claim or source-link field catalogs.
    let mut result = Vec::new();
    for (field, role) in [("preferred_label", "name"), ("notes", "hover")] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
        {
            let (language, script) =
                language_pair(source.get("field_languages").and_then(|v| v.get(field)));
            let mut required = context.clone();
            if source
                .get("field_languages")
                .and_then(|v| v.get(field))
                .is_some()
            {
                required.push(format!("/field_languages/{field}"));
            }
            result.push(Field {
                pointer: format!("/{field}"),
                role,
                language,
                script,
                context: required,
            });
        }
    }
    if let Some(variants) = source.get("variant_labels").and_then(Value::as_array) {
        for (index, variant) in variants.iter().enumerate() {
            let variant_object = variant.as_object()?;
            if variant_object
                .get("value")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty())
            {
                let (language, script) = language_pair(Some(variant));
                let mut required = context.clone();
                for key in variant_object.keys().filter(|key| key.as_str() != "value") {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    required.push(format!("/variant_labels/{index}/{escaped}"));
                }
                result.push(Field {
                    pointer: format!("/variant_labels/{index}/value"),
                    role: "name",
                    language,
                    script,
                    context: required,
                });
            }
        }
    }
    Some(result)
}

fn form_state(
    source: &Value,
    set: &Value,
    form: &Value,
    subject: &ExactRef,
) -> (CopyShadowState, Vec<String>) {
    let Some(set_ref) = set.get("subject").and_then(exact_ref) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    if set_ref.id != subject.id {
        return (CopyShadowState::Invalid, vec![]);
    }
    if set_ref != *subject {
        return (CopyShadowState::Stale, vec![]);
    }
    let Some(form_subject) = form.get("subject").and_then(exact_ref) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    if form_subject.id != subject.id {
        return (CopyShadowState::Invalid, vec![]);
    }
    if form_subject != *subject {
        return (CopyShadowState::Stale, vec![]);
    }
    let Some(content) = form.get("content") else {
        return (CopyShadowState::Invalid, vec![]);
    };
    if content.get("kind").and_then(Value::as_str) != Some("source-copy") {
        return (CopyShadowState::Unsupported, vec![]);
    }
    let Some(slot) = content.get("slot").and_then(Value::as_str) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    let Some(bindings) = form.get("bindings").and_then(Value::as_object) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    let Some(selected) = bindings.get(slot) else {
        return (CopyShadowState::Unavailable, vec![]);
    };
    let Some(selected_record) = selected.get("record").and_then(exact_ref) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    if selected_record != *subject {
        return (CopyShadowState::Unavailable, vec![]);
    }
    let Some(selected_pointer) = selected.get("pointer").and_then(Value::as_str) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    let Some(role) = form.get("role").and_then(Value::as_str) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    let Some(fields) = metadata_fields(source) else {
        return (CopyShadowState::Invalid, vec![]);
    };
    let Some(field) = fields
        .iter()
        .find(|field| field.role == role && field.pointer == selected_pointer)
    else {
        return (CopyShadowState::Unavailable, vec![]);
    };
    let mut observed = Vec::new();
    for binding in bindings.values() {
        let Some(record) = binding.get("record").and_then(exact_ref) else {
            return (CopyShadowState::Invalid, observed);
        };
        if record.id != subject.id {
            return (CopyShadowState::Unavailable, observed);
        }
        if record != *subject {
            return (CopyShadowState::Stale, observed);
        }
        let Some(path) = binding.get("pointer").and_then(Value::as_str) else {
            return (CopyShadowState::Invalid, observed);
        };
        observed.push(path.to_owned());
        if pointer(source, path).is_none() {
            return (CopyShadowState::Invalid, observed);
        }
    }
    for path in &field.context {
        if !bindings.values().any(|binding| {
            binding.get("pointer").and_then(Value::as_str) == Some(path)
                && binding.get("record").and_then(exact_ref).as_ref() == Some(subject)
        }) {
            return (CopyShadowState::Invalid, observed);
        }
    }
    if form
        .get("language_context")
        .is_some_and(|value| !value.is_null())
    {
        return (CopyShadowState::Invalid, observed);
    }
    let wording = pointer(source, selected_pointer).and_then(Value::as_str);
    if wording.is_none_or(|text| text.trim().is_empty()) {
        return (CopyShadowState::Invalid, observed);
    }
    if form.get("language").and_then(Value::as_str) != field.language.as_deref()
        || form.get("script").and_then(Value::as_str) != field.script.as_deref()
    {
        return (CopyShadowState::Invalid, observed);
    }
    (CopyShadowState::CompatibleCore, observed)
}

/// Inspect one selected current form under the corpus-record metadata field
/// catalog. An owner rule must still run the complete source/assessment gates.
pub fn inspect_source_copy_raw(
    raw_source: &[u8],
    raw_set: &[u8],
    selected_form_id: &str,
) -> Result<CopyShadow, CopyShadowError> {
    if raw_source.len() > MAX_SOURCE_BYTES || raw_set.len() > MAX_SET_BYTES {
        return Err(CopyShadowError::BudgetExceeded);
    }
    let source = published_value(raw_source, MAX_SOURCE_BYTES).map_err(map_probe_error)?;
    let set = published_value(raw_set, MAX_SET_BYTES).map_err(map_probe_error)?;
    let mut state = CopyShadowState::Unsupported;
    let mut observed_pointers = Vec::new();
    if source.get("schema_version").and_then(Value::as_str) == Some("tos_corpus_record_v1") {
        if let Some(subject) = source_ref(raw_source, &source)? {
            if inspect_lineage_raw(raw_set).is_ok() {
                if let Some(form) = set
                    .get("forms")
                    .and_then(Value::as_array)
                    .and_then(|forms| {
                        forms.iter().find(|form| {
                            form.get("form_id").and_then(Value::as_str) == Some(selected_form_id)
                        })
                    })
                {
                    (state, observed_pointers) = form_state(&source, &set, form, &subject);
                } else {
                    state = CopyShadowState::Unavailable;
                }
            } else {
                state = CopyShadowState::Invalid;
            }
        } else {
            state = CopyShadowState::Invalid;
        }
    }
    Ok(CopyShadow {
        state,
        source_raw_digest: Digest256::of_bytes(raw_source),
        form_set_raw_digest: Digest256::of_bytes(raw_set),
        selected_form_id: selected_form_id.to_owned(),
        observed_pointers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: &[u8] = include_bytes!("../tests/fixtures/jgb-work.json");
    const FORMS: &[u8] = include_bytes!("../tests/fixtures/jgb-work.human-forms.json");
    const FIRST_ID: &str = "tos.form.jenseits-von-gut-und-boese.name-original";

    fn mutate(raw: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut value: Value = serde_json::from_slice(raw).expect("pinned fixture");
        f(&mut value);
        serde_json::to_vec(&value).expect("mutated fixture")
    }

    #[test]
    fn source_copy_core_matches_four_independent_python_outcomes() {
        let baseline = inspect_source_copy_raw(WORK, FORMS, FIRST_ID).expect("baseline parses");
        assert_eq!(baseline.state, CopyShadowState::CompatibleCore);
        assert_eq!(
            baseline.source_raw_digest.to_hex(),
            "032d3acdc89f751943c98a0b450835effdf0bcc8e0c1ddd362bb9e2d90658f3c"
        );
        assert_eq!(baseline.observed_pointers.len(), 3);

        let omitted = mutate(FORMS, |set| {
            set["forms"][0]["bindings"]
                .as_object_mut()
                .unwrap()
                .remove("context-0");
        });
        assert_eq!(
            inspect_source_copy_raw(WORK, &omitted, FIRST_ID)
                .unwrap()
                .state,
            CopyShadowState::Invalid
        );

        let unsupported_role = mutate(FORMS, |set| {
            set["forms"][0]["role"] = Value::String("statement".into());
        });
        assert_eq!(
            inspect_source_copy_raw(WORK, &unsupported_role, FIRST_ID)
                .unwrap()
                .state,
            CopyShadowState::Unavailable
        );

        let changed_work = mutate(WORK, |work| {
            work["preferred_label"] = Value::String("Jenseits von Gut und Böse changed".into());
        });
        assert_eq!(
            inspect_source_copy_raw(&changed_work, FORMS, FIRST_ID)
                .unwrap()
                .state,
            CopyShadowState::Stale
        );
    }

    #[test]
    fn strict_raw_budget_and_profile_refuse_unsafe_core_observations() {
        let duplicate = String::from_utf8(WORK.to_vec()).unwrap().replacen(
            "\"record_id\":",
            "\"record_id\":\"forged\", \"record_\\u0069d\":",
            1,
        );
        assert_eq!(
            inspect_source_copy_raw(duplicate.as_bytes(), FORMS, FIRST_ID),
            Err(CopyShadowError::InvalidPublishedJson(
                FoundationErrorCode::DuplicateMember
            ))
        );
        assert_eq!(
            inspect_source_copy_raw(&vec![b' '; MAX_SOURCE_BYTES + 1], FORMS, FIRST_ID),
            Err(CopyShadowError::BudgetExceeded)
        );
        let unknown = mutate(WORK, |work| {
            work["schema_version"] = Value::String("tos_corpus_record_v2".into());
        });
        assert_eq!(
            inspect_source_copy_raw(&unknown, FORMS, FIRST_ID)
                .unwrap()
                .state,
            CopyShadowState::Unsupported
        );
    }
}
