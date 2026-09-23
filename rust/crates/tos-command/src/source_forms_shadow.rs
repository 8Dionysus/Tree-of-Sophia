//! Private, in-memory CMD.2 shadow for one frozen public Work HumanForms vector.
//!
//! This module is deliberately not exported by `lib.rs`. It neither reads nor
//! writes owner files, holds a source lock, checks rights, nor consumes a VAL
//! attestation. Exact fixture identity bounds every result; no result is an
//! authorization to publish or admit source.

use std::collections::HashSet;

use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonNumber,
    JsonNumberKind, JsonString, JsonValue, canonical_bytes_v1, canonical_digest_v1,
    emit_json_profile, parse_json,
};

const SOURCE_RAW_SHA: &str =
    "sha256:032d3acdc89f751943c98a0b450835effdf0bcc8e0c1ddd362bb9e2d90658f3c";
const INITIAL_SET_RAW_SHA: &str =
    "sha256:7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b";
const PUBLISHED_SET_RAW_SHA: &str =
    "sha256:68eaceb9d4689d37d5814fc626a7ab67c4b14ed52c654f5569d3a71d041fcb74";
const OWNER_RAW_SHA: &str =
    "sha256:4087525b658c9b6ff2f2147d16eb46786f7a826e83deeec0604e4e75e7e21f9d";
const REQUEST_DIGEST: &str =
    "sha256:b1280cd1a73668bed88c003f8bbf5e03cd068b0a2b70672f94bed7ad312a44bf";
const COMMAND_ID: &str = "cmd2:oracle:jgb-forms:1";
const RECORDED_AT: &str = "2026-01-01T12:34:56+00:00";
const EXISTING_ID: &str = "tos.form.jenseits-von-gut-und-boese.name-original";
const NEW_ID: &str = "tos.form.oracle.jgb-name-ru-copy";

#[derive(Clone, Copy, Debug)]
pub struct WorkFormsInput<'a> {
    pub source_raw: &'a [u8],
    pub form_set_raw: &'a [u8],
    /// Fixture bytes for the configuration digest, not a delegation proof.
    pub owner_config_raw: &'a [u8],
    pub request_raw: &'a [u8],
    pub recorded_at: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkFormsShadow {
    pub form_set_bytes: Vec<u8>,
    pub revision: Digest256,
    pub request_digest: Digest256,
    pub receipt: JsonValue,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShadowError {
    Invalid(&'static str),
    Denied(&'static str),
    Conflict(&'static str),
    Unsupported(&'static str),
}

type Result<T> = std::result::Result<T, ShadowError>;

fn limits(max_bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes,
        ..JsonLimits::default()
    }
}

fn parse(raw: &[u8], max_bytes: usize) -> Result<JsonValue> {
    parse_json(raw, JsonMode::PublishedStrict, limits(max_bytes))
        .map(|doc| doc.into_root())
        .map_err(|_| ShadowError::Invalid("strict JSON or byte budget"))
}

fn canonical(value: &JsonValue) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        limits(1_048_576),
    )
    .map_err(|_| ShadowError::Invalid("canonical JSON input"))
}

fn sorted_copy(value: &JsonValue) -> Result<JsonValue> {
    parse(&canonical(value)?, 1_048_576)
}

fn digest(value: &JsonValue) -> Result<Digest256> {
    canonical_digest_v1(
        value,
        CanonicalProfile::SourceRecordDigestV1,
        limits(1_048_576),
    )
    .map_err(|_| ShadowError::Invalid("canonical record digest"))
}

fn obj(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn field<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue> {
    value
        .object_get(name)
        .ok_or(ShadowError::Invalid("missing object field"))
}

fn text<'a>(value: &'a JsonValue, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or(ShadowError::Invalid("field must be text"))
}

fn integer(value: &JsonValue, name: &str) -> Result<u64> {
    field(value, name)?
        .as_u64()
        .ok_or(ShadowError::Invalid("field must be nonnegative integer"))
}

fn array<'a>(value: &'a JsonValue, name: &str) -> Result<&'a [JsonValue]> {
    field(value, name)?
        .as_array()
        .ok_or(ShadowError::Invalid("field must be array"))
}

fn exact_keys(value: &JsonValue, expected: &[&str]) -> Result<()> {
    let members = value
        .as_object()
        .ok_or(ShadowError::Invalid("expected object"))?;
    if members.len() != expected.len()
        || members
            .iter()
            .any(|(key, _)| !expected.contains(&key.as_str().unwrap_or("")))
    {
        return Err(ShadowError::Invalid("unexpected object fields"));
    }
    Ok(())
}

fn same_json(a: &JsonValue, b: &JsonValue) -> Result<bool> {
    Ok(canonical(a)? == canonical(b)?)
}

fn record_ref(identifier: &str, version: u64, body: &JsonValue) -> Result<JsonValue> {
    Ok(obj(vec![
        ("id", string(identifier)),
        ("version", number(version)),
        ("digest", string(&digest(body)?.to_prefixed())),
    ]))
}

fn form_ref(form: &JsonValue) -> Result<JsonValue> {
    record_ref(text(form, "form_id")?, integer(form, "form_version")?, form)
}

fn check_scope(config: &JsonValue, changes: &[JsonValue]) -> Result<()> {
    let principal = text(config, "principal_id")?;
    let allowed_ids = array(config, "allowed_form_ids")?;
    let allowed_ops = array(config, "allowed_operations")?;
    let mut seen = HashSet::new();
    for change in changes {
        exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = field(change, "form")?;
        let id = text(form, "form_id")?;
        let operation = text(change, "operation")?;
        if !allowed_ids.iter().any(|item| item.as_str() == Some(id))
            || !allowed_ops
                .iter()
                .any(|item| item.as_str() == Some(operation))
            || text(form, "creator_id")? != principal
        {
            return Err(ShadowError::Denied(
                "change outside synthetic fixture scope",
            ));
        }
        if !seen.insert(id) {
            return Err(ShadowError::Invalid("duplicate form identity in batch"));
        }
    }
    Ok(())
}

/// Calculate the exact bounded legacy candidate without any source write.
///
/// Only the frozen Work/source, protected-config *bytes*, two prepared changes,
/// initial set and its one known successor are supported. The fixture guard is
/// intentional: this is not a general Work validator or delegation checker.
pub fn apply_or_replay(input: WorkFormsInput<'_>) -> Result<WorkFormsShadow> {
    if input.recorded_at != RECORDED_AT {
        return Err(ShadowError::Unsupported("fixed oracle instant only"));
    }
    if Digest256::of_bytes(input.source_raw).to_prefixed() != SOURCE_RAW_SHA {
        return Err(ShadowError::Unsupported("other source bytes"));
    }
    let source = parse(input.source_raw, 1_048_576)?;
    let config = parse(input.owner_config_raw, 1_048_576)?;
    let request = parse(input.request_raw, 1_048_576)?;
    exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_source",
            "expected_revision",
            "expected_configuration",
            "changes",
        ],
    )?;
    if text(&request, "schema_version")? != "tos_local_source_command_v1"
        || text(&request, "operation")? != "apply"
    {
        return Err(ShadowError::Unsupported("only public-source-forms apply"));
    }
    let changes = array(&request, "changes")?;
    if changes.len() != 2 {
        return Err(ShadowError::Unsupported("only the frozen two-change batch"));
    }
    check_scope(&config, changes)?; // Fixture comparison below is not owner authorization.
    if Digest256::of_bytes(input.owner_config_raw).to_prefixed() != OWNER_RAW_SHA {
        return Err(ShadowError::Unsupported("other owner configuration bytes"));
    }
    if text(&config, "schema_version")? != "tos_local_source_command_owner_v1"
        || text(&config, "source_path")?
            != "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json"
    {
        return Err(ShadowError::Unsupported("other owner route"));
    }
    let request_digest = Digest256::of_bytes(&canonical(&request)?);
    let subject = record_ref(
        text(&source, "record_id")?,
        integer(&source, "record_version")?,
        &source,
    )?;
    let set_sha = Digest256::of_bytes(input.form_set_raw).to_prefixed();
    if set_sha != INITIAL_SET_RAW_SHA && set_sha != PUBLISHED_SET_RAW_SHA {
        return Err(ShadowError::Unsupported("other form-set history"));
    }
    let set = parse(input.form_set_raw, 2_097_152)?;
    if !same_json(field(&set, "subject")?, &subject)? {
        return Err(ShadowError::Conflict("source and form-set subjects differ"));
    }
    let old_forms = array(&set, "forms")?;
    if old_forms
        .first()
        .and_then(|v| v.object_get("form_id"))
        .and_then(JsonValue::as_str)
        != Some(EXISTING_ID)
    {
        return Err(ShadowError::Unsupported("other Work form order"));
    }
    let command_id = text(&request, "command_id")?;
    if set_sha == PUBLISHED_SET_RAW_SHA {
        let history = array(&set, "growth_history")?;
        if history.len() != 1 || text(&history[0], "command_id")? != command_id {
            return Err(ShadowError::Unsupported("only exact known replay"));
        }
        if text(&history[0], "request_digest")? != request_digest.to_prefixed() {
            return Err(ShadowError::Conflict(
                "command identity reused with different request",
            ));
        }
        return Ok(WorkFormsShadow {
            form_set_bytes: input.form_set_raw.to_vec(),
            revision: Digest256::of_bytes(input.form_set_raw),
            request_digest,
            receipt: history[0].clone(),
            replayed: true,
        });
    }
    if !same_json(field(&request, "expected_source")?, &subject)?
        || text(&request, "expected_configuration")? != digest(&config)?.to_prefixed()
        || text(&request, "expected_revision")? != set_sha
    {
        return Err(ShadowError::Conflict(
            "stale source, config or set revision",
        ));
    }
    if command_id != COMMAND_ID || request_digest.to_prefixed() != REQUEST_DIGEST {
        return Err(ShadowError::Unsupported(
            "other prepared changes or command identity",
        ));
    }
    if text(&changes[0], "operation")? != "form.revise"
        || text(&changes[1], "operation")? != "form.create"
        || text(field(&changes[0], "form")?, "form_id")? != EXISTING_ID
        || text(field(&changes[1], "form")?, "form_id")? != NEW_ID
        || !same_json(
            field(&changes[0], "expected_form")?,
            &form_ref(&old_forms[0])?,
        )?
        || !field(&changes[1], "expected_form")?.is_null()
    {
        return Err(ShadowError::Unsupported("other form transition"));
    }

    // Python `_apply` begins with json.loads(_canonical(payload)): every
    // retained object gets sorted keys before the prepared changes are added.
    let mut successor = sorted_copy(&set)?;
    let items = match &mut successor {
        JsonValue::Object(items) => items,
        _ => return Err(ShadowError::Invalid("form set is not an object")),
    };
    let forms = items
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("forms"))
        .and_then(|(_, value)| match value {
            JsonValue::Array(v) => Some(v),
            _ => None,
        })
        .ok_or(ShadowError::Invalid("missing forms"))?;
    let old = std::mem::replace(&mut forms[0], field(&changes[0], "form")?.clone());
    forms.push(field(&changes[1], "form")?.clone());
    let prior = items
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("prior_forms"))
        .and_then(|(_, value)| match value {
            JsonValue::Array(v) => Some(v),
            _ => None,
        })
        .ok_or(ShadowError::Invalid("missing prior forms"))?;
    prior.push(old);
    let retained_subject = items
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("subject"))
        .ok_or(ShadowError::Invalid("missing subject"))?;
    retained_subject.1 = subject.clone();
    let results = changes
        .iter()
        .map(|change| form_ref(field(change, "form")?))
        .collect::<Result<Vec<_>>>()?;
    let receipt = obj(vec![
        ("command_id", string(command_id)),
        ("request_digest", string(&request_digest.to_prefixed())),
        ("principal_id", field(&config, "principal_id")?.clone()),
        ("authority_ref", field(&config, "authority_ref")?.clone()),
        (
            "owner_configuration",
            string(&digest(&config)?.to_prefixed()),
        ),
        ("recorded_at", string(input.recorded_at)),
        ("source", subject),
        ("previous_revision", string(&set_sha)),
        ("results", JsonValue::Array(results)),
    ]);
    items.push((
        JsonString::from_utf8("growth_history"),
        JsonValue::Array(vec![receipt.clone()]),
    ));
    let encoded = emit_json_profile(
        &successor,
        JsonEmissionProfile::SourceFormSetPublishedV1,
        limits(2_097_152),
    )
    .map_err(|_| ShadowError::Invalid("whole form-set output budget or codec"))?;
    if encoded.sha256.to_prefixed() != PUBLISHED_SET_RAW_SHA {
        return Err(ShadowError::Conflict(
            "candidate differs from frozen legacy output",
        ));
    }
    Ok(WorkFormsShadow {
        form_set_bytes: encoded.bytes,
        revision: encoded.sha256,
        request_digest,
        receipt,
        replayed: false,
    })
}
