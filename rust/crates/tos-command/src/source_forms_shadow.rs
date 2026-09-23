//! Private, in-memory CMD.2 shadow for one frozen public Work HumanForms vector.
//!
//! This module is deliberately not exported by `lib.rs`. It neither reads nor
//! writes owner files, holds a source lock, checks rights, nor consumes a VAL
//! attestation. Exact fixture identity bounds every result; no result is an
//! authorization to publish or admit source.

use std::collections::{HashMap, HashSet};

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
    /// Host-observed UID for mechanical comparison; this does not prove the
    /// protection or independent selection of an owner configuration file.
    pub effective_uid: u64,
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

/// An API-shaped result for local comparison. `proposed_form_set` is only a
/// byte candidate; callers must never treat it as a publication instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkCommandShadow {
    pub response: JsonValue,
    pub proposed_form_set: Option<Vec<u8>>,
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

#[derive(Clone, Debug)]
struct WorkField {
    id: String,
    pointer: String,
    role: &'static str,
    language: JsonValue,
    script: JsonValue,
    context: Vec<String>,
}

impl WorkField {
    fn public(&self) -> JsonValue {
        obj(vec![
            ("field_id", string(&self.id)),
            ("role", string(self.role)),
            ("language", self.language.clone()),
            ("script", self.script.clone()),
        ])
    }
}

fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn pointer(source: &JsonValue, path: &str) -> Result<JsonValue> {
    if path.is_empty() {
        return Ok(source.clone());
    }
    let mut value = source;
    for token in path
        .strip_prefix('/')
        .ok_or(ShadowError::Invalid("invalid source pointer"))?
        .split('/')
    {
        let token = token.replace("~1", "/").replace("~0", "~");
        value = match value {
            JsonValue::Object(_) => value.object_get(&token),
            JsonValue::Array(items) => token.parse::<usize>().ok().and_then(|at| items.get(at)),
            _ => None,
        }
        .ok_or(ShadowError::Invalid("source pointer does not resolve"))?;
    }
    Ok(value.clone())
}

fn declared_language(source: &JsonValue, key: &str) -> Result<(JsonValue, JsonValue, bool)> {
    let declarations = source.object_get("field_languages");
    let declaration = declarations.and_then(|value| value.object_get(key));
    if let Some(value) = declarations {
        let entries = value
            .as_object()
            .ok_or(ShadowError::Unsupported("other Work language declarations"))?;
        if entries
            .iter()
            .any(|(key, _)| !matches!(key.as_str(), Some("notes" | "preferred_label")))
        {
            return Err(ShadowError::Unsupported("other Work language declarations"));
        }
    }
    let Some(value) = declaration else {
        return Ok((JsonValue::Null, JsonValue::Null, false));
    };
    let entries = value
        .as_object()
        .ok_or(ShadowError::Unsupported("other Work language declaration"))?;
    if entries
        .iter()
        .any(|(key, _)| !matches!(key.as_str(), Some("language" | "script")))
    {
        return Err(ShadowError::Unsupported("other Work language declaration"));
    }
    let language = value
        .object_get("language")
        .cloned()
        .unwrap_or(JsonValue::Null);
    let script = value
        .object_get("script")
        .cloned()
        .unwrap_or(JsonValue::Null);
    if language.as_str().is_none() && !language.is_null()
        || script.as_str().is_none() && !script.is_null()
    {
        return Err(ShadowError::Unsupported("other Work language declaration"));
    }
    Ok((language, script, true))
}

fn work_fields(source: &JsonValue) -> Result<Vec<WorkField>> {
    if text(source, "schema_version")? != "tos_corpus_record_v1"
        || text(source, "record_type")? != "work"
        || !text(source, "record_id")?.starts_with("tos.work.")
    {
        return Err(ShadowError::Unsupported(
            "only public Work metadata sources",
        ));
    }
    // These richer source profiles have different context or visibility law.
    if [
        "semantic_scope",
        "semantic_content",
        "form_identity",
        "native_text_binding",
        "visibility",
    ]
    .iter()
    .any(|key| source.object_get(key).is_some())
    {
        return Err(ShadowError::Unsupported(
            "other Work metadata context profile",
        ));
    }
    let mut context = Vec::new();
    for key in ["identity_status", "same_as_posture"] {
        if source.object_get(key).is_some() {
            context.push(format!("/{key}"));
        }
    }
    if context.len() != 2 {
        return Err(ShadowError::Unsupported("incomplete Work identity context"));
    }
    let mut fields = Vec::new();
    for (key, id, role) in [
        ("preferred_label", "metadata.preferred-name", "name"),
        ("notes", "metadata.source-note", "hover"),
    ] {
        let (language, script, has_declaration) = declared_language(source, key)?;
        if let Some(value) = source.object_get(key).and_then(JsonValue::as_str) {
            if !value.trim().is_empty() {
                let mut guards = context.clone();
                if has_declaration {
                    guards.push(format!("/field_languages/{key}"));
                }
                fields.push(WorkField {
                    id: id.to_owned(),
                    pointer: format!("/{key}"),
                    role,
                    language,
                    script,
                    context: guards,
                });
            }
        } else if has_declaration {
            return Err(ShadowError::Invalid("declared Work wording is missing"));
        }
    }
    if let Some(variants) = source.object_get("variant_labels") {
        for (index, variant) in variants
            .as_array()
            .ok_or(ShadowError::Invalid("Work variants must be an array"))?
            .iter()
            .enumerate()
        {
            let entries = variant
                .as_object()
                .ok_or(ShadowError::Invalid("Work variant must be an object"))?;
            let Some(wording) = variant.object_get("value").and_then(JsonValue::as_str) else {
                continue;
            };
            if wording.trim().is_empty() {
                continue;
            }
            let language = variant
                .object_get("language")
                .cloned()
                .unwrap_or(JsonValue::Null);
            let script = variant
                .object_get("script")
                .cloned()
                .unwrap_or(JsonValue::Null);
            if language.as_str().is_none() && !language.is_null()
                || script.as_str().is_none() && !script.is_null()
            {
                return Err(ShadowError::Invalid(
                    "variant language or script is malformed",
                ));
            }
            let base = format!("/variant_labels/{index}/");
            let mut guards = context.clone();
            for (key, _) in entries {
                let name = key
                    .as_str()
                    .ok_or(ShadowError::Invalid("variant key is not UTF-8"))?;
                if name != "value" {
                    guards.push(format!("{base}{}", pointer_token(name)));
                }
            }
            fields.push(WorkField {
                id: format!("metadata.variant-name:{index}"),
                pointer: format!("{base}value"),
                role: "name",
                language,
                script,
                context: guards,
            });
        }
    }
    if fields.is_empty() || fields.len() > 64 {
        return Err(ShadowError::Unsupported(
            "Work field catalogue outside shadow budget",
        ));
    }
    Ok(fields)
}

fn binding(subject: &JsonValue, path: &str) -> JsonValue {
    obj(vec![("record", subject.clone()), ("pointer", string(path))])
}

fn prepared_change(
    set: &JsonValue,
    subject: &JsonValue,
    principal: &str,
    id: &str,
    field: &WorkField,
) -> Result<JsonValue> {
    let old = array(set, "forms")?
        .iter()
        .find(|form| form.object_get("form_id").and_then(JsonValue::as_str) == Some(id));
    let operation = if old.is_some() {
        "form.revise"
    } else {
        "form.create"
    };
    let predecessor = old.map(form_ref).transpose()?.unwrap_or(JsonValue::Null);
    let mut bindings = vec![(
        JsonString::from_utf8("wording"),
        binding(subject, &field.pointer),
    )];
    for (index, path) in field.context.iter().enumerate() {
        bindings.push((
            JsonString::from_utf8(&format!("context-{index}")),
            binding(subject, path),
        ));
    }
    let form = obj(vec![
        ("schema_version", string("tos_human_form_v1")),
        ("form_id", string(id)),
        (
            "form_version",
            number(
                old.map(|form| integer(form, "form_version"))
                    .transpose()?
                    .unwrap_or(0)
                    + 1,
            ),
        ),
        ("subject", subject.clone()),
        ("role", string(field.role)),
        ("language", field.language.clone()),
        ("script", field.script.clone()),
        ("creator_id", string(principal)),
        ("revises", predecessor.clone()),
        ("bindings", JsonValue::Object(bindings)),
        (
            "content",
            obj(vec![
                ("kind", string("source-copy")),
                ("slot", string("wording")),
            ]),
        ),
    ]);
    Ok(obj(vec![
        ("operation", string(operation)),
        ("expected_form", predecessor),
        ("form", form),
    ]))
}

fn materialization(
    source: &JsonValue,
    subject: &JsonValue,
    form: &JsonValue,
    fields: &[WorkField],
) -> Result<JsonValue> {
    if !same_json(field(form, "subject")?, subject)?
        || text(field(form, "content")?, "kind")? != "source-copy"
        || text(field(form, "content")?, "slot")? != "wording"
    {
        return Err(ShadowError::Unsupported("other Work materialization state"));
    }
    let selected = field(field(form, "bindings")?, "wording")?;
    let field = fields
        .iter()
        .find(|candidate| {
            candidate.pointer
                == selected
                    .object_get("pointer")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                && candidate.role
                    == form
                        .object_get("role")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
        })
        .ok_or(ShadowError::Unsupported("unsupported Work form field"))?;
    if !same_json(selected, &binding(subject, &field.pointer))?
        || !same_json(field_for(form, "language")?, &field.language)?
        || !same_json(field_for(form, "script")?, &field.script)?
    {
        return Err(ShadowError::Unsupported("Work form source binding changed"));
    }
    let bindings = field_for(form, "bindings")?;
    if bindings.as_object().map(|members| members.len()) != Some(field.context.len() + 1) {
        return Err(ShadowError::Unsupported(
            "Work form context binding count changed",
        ));
    }
    let mut context = Vec::new();
    for (index, path) in field.context.iter().enumerate() {
        let slot = format!("context-{index}");
        let actual = field_for(bindings, &slot)?;
        let expected = binding(subject, path);
        if !same_json(actual, &expected)? {
            return Err(ShadowError::Unsupported(
                "Work form context binding changed",
            ));
        }
        context.push(obj(vec![
            ("slot", string(&slot)),
            ("binding", expected),
            ("value", pointer(source, path)?),
        ]));
    }
    let wording = pointer(source, &field.pointer)?;
    if wording.as_str().is_none() {
        return Err(ShadowError::Unsupported("Work wording is not text"));
    }
    Ok(obj(vec![
        (
            "schema_version",
            string("tos_human_form_materialization_v1"),
        ),
        ("form", form_ref(form)?),
        ("subject", subject.clone()),
        ("state", string("ready")),
        ("display_text", wording),
        ("context", JsonValue::Array(context)),
        ("issues", JsonValue::Array(vec![])),
        ("admission", JsonValue::Null),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
        ("role", string(field.role)),
        ("language", field.language.clone()),
        ("script", field.script.clone()),
        ("derivation", string("source-copy")),
        ("dependencies", JsonValue::Array(vec![subject.clone()])),
        ("standalone_reading", JsonValue::Bool(false)),
    ]))
}

fn field_for<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue> {
    field(value, name)
}

fn validate_history(set: &JsonValue, subject: &JsonValue) -> Result<()> {
    if text(set, "schema_version")? != "tos_human_form_set_v1" {
        return Err(ShadowError::Unsupported("other HumanForm set schema"));
    }
    if text(field(set, "subject")?, "id")? != text(subject, "id")? {
        return Err(ShadowError::Conflict("Work and HumanForm subject differ"));
    }
    let current = array(set, "forms")?;
    let prior = array(set, "prior_forms")?;
    if current.len() + prior.len() > 1024 {
        return Err(ShadowError::Unsupported(
            "Work form history exceeds shadow budget",
        ));
    }
    let mut indexed: HashMap<(String, u64), &JsonValue> = HashMap::new();
    let mut current_ids = HashSet::new();
    for form in prior.iter().chain(current.iter()) {
        let id = text(form, "form_id")?.to_owned();
        let version = integer(form, "form_version")?;
        if version == 0
            || text(field(form, "subject")?, "id")? != text(subject, "id")?
            || indexed.insert((id, version), form).is_some()
        {
            return Err(ShadowError::Unsupported(
                "broken Work form identity history",
            ));
        }
    }
    for form in current {
        if !current_ids.insert(text(form, "form_id")?.to_owned()) {
            return Err(ShadowError::Unsupported(
                "duplicate current Work form identity",
            ));
        }
    }
    for ((id, version), form) in &indexed {
        if !current_ids.contains(id) {
            return Err(ShadowError::Unsupported(
                "prior Work form has no current identity",
            ));
        }
        let predecessor = field(form, "revises")?;
        if *version == 1 {
            if !predecessor.is_null() {
                return Err(ShadowError::Unsupported(
                    "initial Work form has predecessor",
                ));
            }
        } else {
            let Some(previous) = indexed.get(&(id.clone(), version - 1)) else {
                return Err(ShadowError::Unsupported("missing Work form predecessor"));
            };
            if !same_json(predecessor, &form_ref(previous)?)? {
                return Err(ShadowError::Unsupported("wrong Work form predecessor"));
            }
        }
    }
    for form in current {
        let id = text(form, "form_id")?;
        let version = integer(form, "form_version")?;
        if indexed
            .keys()
            .any(|(other, next)| other == id && *next > version)
        {
            return Err(ShadowError::Unsupported("current Work form is not latest"));
        }
    }
    let mut command_ids = HashSet::new();
    if let Some(history) = set.object_get("growth_history") {
        for receipt in history
            .as_array()
            .ok_or(ShadowError::Invalid("growth history must be array"))?
        {
            if !command_ids.insert(text(receipt, "command_id")?.to_owned())
                || text(field(receipt, "source")?, "id")? != text(subject, "id")?
            {
                return Err(ShadowError::Unsupported(
                    "duplicate or foreign Work receipt",
                ));
            }
            for reference in array(receipt, "results")? {
                let key = (
                    text(reference, "id")?.to_owned(),
                    integer(reference, "version")?,
                );
                let Some(form) = indexed.get(&key) else {
                    return Err(ShadowError::Unsupported("receipt result not retained"));
                };
                if !same_json(reference, &form_ref(form)?)? {
                    return Err(ShadowError::Unsupported("receipt result digest differs"));
                }
            }
        }
    }
    Ok(())
}

fn check_work_owner(input: WorkFormsInput<'_>, config: &JsonValue) -> Result<()> {
    exact_keys(
        config,
        &[
            "schema_version",
            "uid",
            "principal_id",
            "source_root",
            "source_path",
            "authority_ref",
            "allowed_form_ids",
            "allowed_operations",
            "expires_at",
        ],
    )?;
    if text(config, "schema_version")? != "tos_local_source_command_owner_v1" {
        return Err(ShadowError::Unsupported("other form owner schema"));
    }
    if integer(config, "uid")? != input.effective_uid {
        return Err(ShadowError::Denied("owner UID differs"));
    }
    if text(config, "principal_id")?.trim().is_empty()
        || text(config, "authority_ref")?.trim().is_empty()
    {
        return Err(ShadowError::Denied("owner identity is empty"));
    }
    let root = text(config, "source_root")?;
    let path = text(config, "source_path")?;
    if !root.starts_with('/')
        || root.split('/').any(|part| part == "..")
        || !path.starts_with("ToS/source-witnesses/works/")
        || !path.ends_with("/work.json")
        || path
            .split('/')
            .any(|part| part == ".." || part == "payload" || part == "catalog")
    {
        return Err(ShadowError::Denied("other source owner path"));
    }
    let expiry = text(config, "expires_at")?;
    if expiry.len() != 20
        || !expiry.is_ascii()
        || !expiry.ends_with('Z')
        || input.recorded_at.len() < 19
        || &expiry[..19] <= &input.recorded_at[..19]
    {
        return Err(ShadowError::Denied(
            "owner delegation expired or unsupported clock",
        ));
    }
    let allowed_ids = array(config, "allowed_form_ids")?;
    let allowed_ops = array(config, "allowed_operations")?;
    if allowed_ids.len() > 32 || allowed_ops.len() > 2 {
        return Err(ShadowError::Invalid(
            "owner scope exceeds Work shadow budget",
        ));
    }
    let mut unique = HashSet::new();
    for id in allowed_ids {
        let value = id
            .as_str()
            .ok_or(ShadowError::Invalid("form ID must be text"))?;
        if !value.starts_with("tos.form.") || !unique.insert(value) {
            return Err(ShadowError::Invalid("invalid owner form identity scope"));
        }
    }
    unique.clear();
    for operation in allowed_ops {
        let value = operation
            .as_str()
            .ok_or(ShadowError::Invalid("operation must be text"))?;
        if !matches!(value, "form.create" | "form.revise") || !unique.insert(value) {
            return Err(ShadowError::Invalid("invalid owner operation scope"));
        }
    }
    Ok(())
}

fn result_value(
    source: &JsonValue,
    set: &JsonValue,
    config: &JsonValue,
    fields: &[WorkField],
    subject: &JsonValue,
    revision: &str,
    receipt: JsonValue,
    replayed: bool,
) -> Result<JsonValue> {
    let path = text(config, "source_path")?;
    let target = path
        .strip_suffix("work.json")
        .ok_or(ShadowError::Unsupported("other Work source filename"))?
        .to_owned()
        + "work.human-forms.json";
    let forms = array(set, "forms")?;
    let form_refs = forms.iter().map(form_ref).collect::<Result<Vec<_>>>()?;
    let views = forms
        .iter()
        .map(|form| materialization(source, subject, form, fields))
        .collect::<Result<Vec<_>>>()?;
    Ok(obj(vec![
        (
            "schema_version",
            string("tos_local_source_command_result_v1"),
        ),
        ("authentication", string("local-unix-account")),
        (
            "owner_configuration",
            string(&digest(config)?.to_prefixed()),
        ),
        ("source", subject.clone()),
        ("source_path", string(path)),
        ("target_path", string(&target)),
        ("revision", string(revision)),
        (
            "supported_operations",
            JsonValue::Array(vec![string("form.create"), string("form.revise")]),
        ),
        (
            "allowed_operations",
            field(config, "allowed_operations")?.clone(),
        ),
        (
            "command_operations",
            JsonValue::Array(vec![string("describe"), string("prepare"), string("apply")]),
        ),
        (
            "source_fields",
            JsonValue::Array(fields.iter().map(WorkField::public).collect()),
        ),
        (
            "allowed_form_ids",
            field(config, "allowed_form_ids")?.clone(),
        ),
        ("forms", JsonValue::Array(form_refs)),
        ("materializations", JsonValue::Array(views)),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
    ]))
}

fn response_insert(response: &mut JsonValue, key: &str, value: JsonValue) -> Result<()> {
    let items = match response {
        JsonValue::Object(items) => items,
        _ => return Err(ShadowError::Invalid("response is not object")),
    };
    if items.iter().any(|(name, _)| name.as_str() == Some(key)) {
        return Err(ShadowError::Invalid("response field already exists"));
    }
    items.push((JsonString::from_utf8(key), value));
    Ok(())
}

fn array_mut<'a>(root: &'a mut JsonValue, name: &str) -> Result<&'a mut Vec<JsonValue>> {
    let entries = match root {
        JsonValue::Object(entries) => entries,
        _ => return Err(ShadowError::Invalid("form set is not an object")),
    };
    entries
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .and_then(|(_, value)| match value {
            JsonValue::Array(items) => Some(items),
            _ => None,
        })
        .ok_or(ShadowError::Invalid("missing form-set array"))
}

/// Portable mechanics for the bounded public Work/source-copy subprofile.
///
/// The caller supplies one current source/config/set snapshot. This function
/// does not prove file mode, symlink freedom, independent config selection,
/// complete corpus schema, rights, a writer lock, or VAL FullOnly coverage.
/// Those owner gates are required before any publication; this shadow never
/// performs one. Other HumanForm modes and source families fail closed.
pub fn run_work_command(input: WorkFormsInput<'_>) -> Result<WorkCommandShadow> {
    if input.recorded_at.len() != 25
        || !input.recorded_at.ends_with("+00:00")
        || !input.recorded_at.is_ascii()
    {
        return Err(ShadowError::Unsupported(
            "only explicit UTC shadow instants",
        ));
    }
    let config = parse(input.owner_config_raw, 1_048_576)?;
    check_work_owner(input, &config)?;
    let source = parse(input.source_raw, 1_048_576)?;
    let fields = work_fields(&source)?;
    let route_slug = text(&config, "source_path")?
        .strip_suffix("/work.json")
        .and_then(|route| route.rsplit('/').next())
        .ok_or(ShadowError::Denied("Work route has no subject directory"))?;
    if text(&source, "record_id")?.rsplit('.').next() != Some(route_slug) {
        return Err(ShadowError::Denied(
            "Work source and configured route differ",
        ));
    }
    let subject = record_ref(
        text(&source, "record_id")?,
        integer(&source, "record_version")?,
        &source,
    )?;
    let set = parse(input.form_set_raw, 2_097_152)?;
    validate_history(&set, &subject)?;
    let revision = Digest256::of_bytes(input.form_set_raw).to_prefixed();
    let request = sorted_copy(&parse(input.request_raw, 1_048_576)?)?;
    if text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(ShadowError::Invalid("wrong Work command schema"));
    }
    let operation = text(&request, "operation")?;
    if operation == "describe" {
        exact_keys(&request, &["schema_version", "operation"])?;
        let response = result_value(
            &source,
            &set,
            &config,
            &fields,
            &subject,
            &revision,
            JsonValue::Null,
            false,
        )?;
        return Ok(WorkCommandShadow {
            response,
            proposed_form_set: None,
        });
    }
    if operation == "prepare" {
        exact_keys(
            &request,
            &["schema_version", "operation", "form_id", "field_id"],
        )?;
        let id = text(&request, "form_id")?;
        if !array(&config, "allowed_form_ids")?
            .iter()
            .any(|value| value.as_str() == Some(id))
        {
            return Err(ShadowError::Denied(
                "prepared form outside owner identity scope",
            ));
        }
        let chosen = fields
            .iter()
            .find(|field| field.id == text(&request, "field_id").unwrap_or(""))
            .ok_or(ShadowError::Invalid("unknown Work metadata field"))?;
        let change = prepared_change(&set, &subject, text(&config, "principal_id")?, id, chosen)?;
        if !array(&config, "allowed_operations")?
            .iter()
            .any(|item| item.as_str() == change.object_get("operation").and_then(JsonValue::as_str))
        {
            return Err(ShadowError::Denied(
                "prepared operation outside owner scope",
            ));
        }
        let preview = materialization(&source, &subject, field(&change, "form")?, &fields)?;
        let mut response = result_value(
            &source,
            &set,
            &config,
            &fields,
            &subject,
            &revision,
            JsonValue::Null,
            false,
        )?;
        response_insert(&mut response, "prepared_change", change)?;
        response_insert(&mut response, "prepared_materialization", preview)?;
        return Ok(WorkCommandShadow {
            response,
            proposed_form_set: None,
        });
    }
    if operation != "apply" {
        return Err(ShadowError::Invalid("unknown Work command operation"));
    }
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
    let command_id = text(&request, "command_id")?;
    if command_id.is_empty() || command_id.len() > 256 {
        return Err(ShadowError::Invalid("command identity outside byte budget"));
    }
    let changes = array(&request, "changes")?;
    if changes.is_empty() || changes.len() > 32 {
        return Err(ShadowError::Invalid(
            "Work source-copy batch size outside budget",
        ));
    }
    check_scope(&config, changes)?;
    let request_digest = Digest256::of_bytes(&canonical(&request)?).to_prefixed();
    if let Some(history) = set.object_get("growth_history") {
        if let Some(receipt) = history.as_array().and_then(|items| {
            items.iter().find(|row| {
                row.object_get("command_id").and_then(JsonValue::as_str) == Some(command_id)
            })
        }) {
            if text(receipt, "request_digest")? != request_digest {
                return Err(ShadowError::Conflict(
                    "command identity reused with different request",
                ));
            }
            let response = result_value(
                &source,
                &set,
                &config,
                &fields,
                &subject,
                &revision,
                receipt.clone(),
                true,
            )?;
            return Ok(WorkCommandShadow {
                response,
                proposed_form_set: None,
            });
        }
    }
    if !same_json(field(&request, "expected_source")?, &subject)?
        || text(&request, "expected_configuration")? != digest(&config)?.to_prefixed()
        || text(&request, "expected_revision")? != revision
    {
        return Err(ShadowError::Conflict(
            "stale Work source, config or set revision",
        ));
    }
    let principal = text(&config, "principal_id")?;
    for change in changes {
        let form = field(change, "form")?;
        let selected = field(field(form, "bindings")?, "wording")?;
        let source_path = text(selected, "pointer")?;
        let role = text(form, "role")?;
        let chosen = fields
            .iter()
            .find(|field| field.pointer == source_path && field.role == role)
            .ok_or(ShadowError::Unsupported("other Work form field"))?;
        let expected = prepared_change(&set, &subject, principal, text(form, "form_id")?, chosen)?;
        if !same_json(change, &expected)? {
            return Err(ShadowError::Unsupported(
                "other Work form production mode or binding",
            ));
        }
        materialization(&source, &subject, form, &fields)?;
    }
    let mut successor = sorted_copy(&set)?;
    for change in changes {
        let form = field(change, "form")?.clone();
        let id = text(&form, "form_id")?;
        if text(change, "operation")? == "form.revise" {
            let forms = array_mut(&mut successor, "forms")?;
            let at = forms
                .iter()
                .position(|old| old.object_get("form_id").and_then(JsonValue::as_str) == Some(id))
                .ok_or(ShadowError::Conflict("Work form predecessor absent"))?;
            let old = std::mem::replace(&mut forms[at], form);
            array_mut(&mut successor, "prior_forms")?.push(old);
        } else {
            array_mut(&mut successor, "forms")?.push(form);
        }
    }
    let results = changes
        .iter()
        .map(|change| form_ref(field(change, "form")?))
        .collect::<Result<Vec<_>>>()?;
    let receipt = obj(vec![
        ("command_id", string(command_id)),
        ("request_digest", string(&request_digest)),
        ("principal_id", string(principal)),
        ("authority_ref", field(&config, "authority_ref")?.clone()),
        (
            "owner_configuration",
            string(&digest(&config)?.to_prefixed()),
        ),
        ("recorded_at", string(input.recorded_at)),
        ("source", subject.clone()),
        ("previous_revision", string(&revision)),
        ("results", JsonValue::Array(results)),
    ]);
    if successor.object_get("growth_history").is_some() {
        array_mut(&mut successor, "growth_history")?.push(receipt.clone());
    } else {
        match &mut successor {
            JsonValue::Object(entries) => entries.push((
                JsonString::from_utf8("growth_history"),
                JsonValue::Array(vec![receipt.clone()]),
            )),
            _ => return Err(ShadowError::Invalid("form set is not an object")),
        }
    }
    validate_history(&successor, &subject)?;
    let encoded = emit_json_profile(
        &successor,
        JsonEmissionProfile::SourceFormSetPublishedV1,
        limits(2_097_152),
    )
    .map_err(|_| ShadowError::Invalid("published Work form-set byte budget or codec"))?;
    let response = result_value(
        &source,
        &successor,
        &config,
        &fields,
        &subject,
        &encoded.sha256.to_prefixed(),
        receipt,
        false,
    )?;
    Ok(WorkCommandShadow {
        response,
        proposed_form_set: Some(encoded.bytes),
    })
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
