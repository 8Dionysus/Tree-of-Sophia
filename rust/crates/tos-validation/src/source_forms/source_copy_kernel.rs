//! The existing maintained source-copy forms kernel, shared by command and
//! read-only compound reconstruction. No custody, owner grant or publication.
use std::collections::{HashMap, HashSet};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, parse_json,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormMechanicsError {
    Invalid(&'static str),
    Conflict(&'static str),
    Denied(&'static str),
    Unsupported(&'static str),
}
use FormMechanicsError as Error;
pub type Result<T> = std::result::Result<T, FormMechanicsError>;
fn validate_instant(value: &str) -> Result<()> {
    crate::retirement_rules::observed_instant_order(value, value)
        .map(|_| ())
        .map_err(|_| Error::Invalid("instant requires explicit valid timezone"))
}

fn parse(raw: &[u8]) -> Result<JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map(|doc| doc.into_root())
    .map_err(|_| Error::Invalid("strict JSON input"))
}
fn canonical(value: &JsonValue) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Invalid("source canonical input"))
}
fn record_digest(value: &JsonValue) -> Result<Digest256> {
    Ok(Digest256::of_bytes(&canonical(value)?))
}
fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn string(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn field<'a>(v: &'a JsonValue, k: &str) -> Result<&'a JsonValue> {
    v.object_get(k)
        .ok_or(Error::Invalid("missing object field"))
}
fn text<'a>(v: &'a JsonValue, k: &str) -> Result<&'a str> {
    field(v, k)?
        .as_str()
        .ok_or(Error::Invalid("expected text field"))
}
fn integer(v: &JsonValue, k: &str) -> Result<u64> {
    field(v, k)?
        .as_u64()
        .ok_or(Error::Invalid("expected integer field"))
}
fn array<'a>(v: &'a JsonValue, k: &str) -> Result<&'a [JsonValue]> {
    field(v, k)?
        .as_array()
        .ok_or(Error::Invalid("expected array field"))
}
fn exact_keys(v: &JsonValue, keys: &[&str]) -> Result<()> {
    let members = v.as_object().ok_or(Error::Invalid("expected object"))?;
    if members.len() != keys.len()
        || members
            .iter()
            .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
    {
        return Err(Error::Invalid("unexpected object fields"));
    }
    Ok(())
}
fn set(v: &mut JsonValue, k: &str, new: JsonValue) -> Result<()> {
    let JsonValue::Object(members) = v else {
        return Err(Error::Invalid("expected object"));
    };
    if let Some((_, old)) = members.iter_mut().find(|(key, _)| key.as_str() == Some(k)) {
        *old = new;
    } else {
        members.push((JsonString::from_utf8(k), new));
    }
    Ok(())
}
fn same(a: &JsonValue, b: &JsonValue) -> Result<bool> {
    Ok(canonical(a)? == canonical(b)?)
}
/// Preserve the maintained Python owner's Unicode 16 `str.strip()` test.
/// The scalar budget is bounded by the already selected UTF-8 byte length.
fn nonblank(value: &str) -> bool {
    tos_foundation::python_strip_unicode16_v1(value, value.len())
        .is_ok_and(|stripped| !stripped.is_empty())
}
fn reference(v: &JsonValue, id: &str, version: &str) -> Result<JsonValue> {
    Ok(object(vec![
        ("id", string(text(v, id)?)),
        ("version", number(integer(v, version)?)),
        ("digest", string(&record_digest(v)?.to_prefixed())),
    ]))
}

fn record_reference(id: &str, version: u64, body: &JsonValue) -> Result<JsonValue> {
    Ok(object(vec![
        ("id", string(id)),
        ("version", number(version)),
        ("digest", string(&record_digest(body)?.to_prefixed())),
    ]))
}
pub fn form_reference(form: &JsonValue) -> Result<JsonValue> {
    reference(form, "form_id", "form_version")
}
pub fn metadata_subject(source: &JsonValue) -> Result<JsonValue> {
    if source.object_get("claim_id").is_some() {
        return reference(source, "claim_id", "claim_version");
    }
    let id = match text(source, "schema_version")? {
        "tos_canonical_node_v1" => "node_id",
        "tos_scholarly_composite_witness_v1" => "composite_id",
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => "artifact_id",
        _ => "record_id",
    };
    reference(source, id, "record_version")
}

fn language(source: &JsonValue, key: &str) -> Result<(JsonValue, JsonValue)> {
    let declaration = source
        .object_get("field_languages")
        .and_then(|v| v.object_get(key));
    let language = declaration
        .and_then(|v| v.object_get("language"))
        .cloned()
        .unwrap_or(JsonValue::Null);
    let script = declaration
        .and_then(|v| v.object_get("script"))
        .cloned()
        .unwrap_or(JsonValue::Null);
    if !language.is_null() && language.as_str().is_none()
        || !script.is_null() && script.as_str().is_none()
    {
        return Err(Error::Invalid("field language declaration"));
    }
    Ok((language, script))
}

/// Literal selectors of the maintained metadata/canonical/Claim adapters.
/// Schemas, record kind/profile and path binding are checked by their callers.
pub fn metadata_fields(source: &JsonValue) -> Result<Vec<FormField>> {
    let mut fields = Vec::new();
    if source.object_get("claim_id").is_some() {
        let qualifiers = field(source, "qualifiers")?;
        let Some(statement) = qualifiers
            .object_get("statement")
            .and_then(JsonValue::as_str)
            .filter(|s| nonblank(s))
        else {
            return Ok(fields);
        };
        let _ = statement;
        fields.push(FormField {
            id: "claim.statement".into(),
            pointer: "/qualifiers/statement".into(),
            role: "statement".into(),
            language: qualifiers
                .object_get("statement_language")
                .cloned()
                .unwrap_or(JsonValue::Null),
            script: qualifiers
                .object_get("statement_script")
                .cloned()
                .unwrap_or(JsonValue::Null),
            context: vec!["".into()],
        });
        if let Some(display) = qualifiers.object_get("display_fields").filter(|v| {
            v.object_get("schema_version").and_then(JsonValue::as_str)
                == Some("tos_claim_display_fields_v1")
        }) {
            for role in ["name", "caption", "hover"] {
                if let Some(wording) = display.object_get(role) {
                    fields.push(FormField {
                        id: format!("claim.{role}"),
                        pointer: format!("/qualifiers/display_fields/{role}/text"),
                        role: role.into(),
                        language: field(wording, "language")?.clone(),
                        script: field(wording, "script")?.clone(),
                        context: vec!["".into()],
                    });
                }
            }
        }
        return Ok(fields);
    }
    let version = text(source, "schema_version")?;
    if matches!(
        version,
        "tos_artifact_source_witness_v1"
            | "tos_artifact_source_witness_v2"
            | "tos_scholarly_composite_witness_v1"
    ) {
        let composite = version == "tos_scholarly_composite_witness_v1";
        let name_context = vec![
            if composite {
                "/identity_status"
            } else {
                "/custody"
            }
            .into(),
            "/layer_separation".into(),
            "/authority".into(),
            "/rights_ref".into(),
        ];
        return Ok(vec![
            FormField {
                id: "metadata.preferred-name".into(),
                pointer: if composite {
                    "/preferred_label"
                } else {
                    "/custody/inventory_numbers/0"
                }
                .into(),
                role: "name".into(),
                language: JsonValue::Null,
                script: JsonValue::Null,
                context: name_context,
            },
            FormField {
                id: "metadata.source-note".into(),
                pointer: if composite {
                    "/editorial_object/description"
                } else {
                    "/path_identity/note"
                }
                .into(),
                role: "hover".into(),
                language: JsonValue::Null,
                script: JsonValue::Null,
                context: vec!["".into()],
            },
        ]);
    }
    let canonical = version == "tos_canonical_node_v1";
    let mut context = if canonical {
        vec!["".into()]
    } else {
        [
            "identity_status",
            "same_as_posture",
            "semantic_scope",
            "semantic_content",
            "form_identity",
            "native_text_binding",
        ]
        .iter()
        .filter(|key| source.object_get(key).is_some())
        .map(|key| format!("/{key}"))
        .collect::<Vec<_>>()
    };
    if version == "tos_source_link_v1" {
        context.extend(
            [
                "uri",
                "provider_label",
                "link_kind",
                "access_status",
                "observed_at",
                "observation_ref",
                "association_claim_refs",
                "provenance_event_ref",
            ]
            .iter()
            .map(|key| format!("/{key}")),
        );
    }
    if source.object_get("record_type").and_then(JsonValue::as_str) == Some("sign") {
        context.push("/promotion_basis".into());
    }
    if let Some(declarations) = source.object_get("field_languages") {
        for (key, _) in declarations
            .as_object()
            .ok_or(Error::Invalid("field language map"))?
        {
            let key = key
                .as_str()
                .ok_or(Error::Invalid("language declaration key"))?;
            if source
                .object_get(key)
                .and_then(JsonValue::as_str)
                .is_none_or(|s| !nonblank(s))
            {
                return Err(Error::Invalid("language declaration has no wording"));
            }
        }
    }
    for (key, id, role) in if canonical {
        vec![
            ("preferred_label", "canonical.preferred-name", "name"),
            ("distilled_thesis", "canonical.thesis", "statement"),
        ]
    } else {
        vec![
            ("preferred_label", "metadata.preferred-name", "name"),
            ("notes", "metadata.source-note", "hover"),
        ]
    } {
        if source
            .object_get(key)
            .and_then(JsonValue::as_str)
            .is_some_and(nonblank)
        {
            let (language, script) = language(source, key)?;
            let mut guards = context.clone();
            if !canonical
                && source
                    .object_get("field_languages")
                    .and_then(|v| v.object_get(key))
                    .is_some()
            {
                guards.push(format!("/field_languages/{key}"));
            }
            fields.push(FormField {
                id: id.into(),
                pointer: format!("/{key}"),
                role: role.into(),
                language,
                script,
                context: guards,
            });
        } else if canonical && key == "distilled_thesis" {
            return Err(Error::Invalid("canonical thesis missing"));
        }
    }
    if let Some(variants) = source.object_get("variant_labels") {
        for (index, variant) in variants
            .as_array()
            .ok_or(Error::Invalid("variant array"))?
            .iter()
            .enumerate()
        {
            if variant
                .object_get("value")
                .and_then(JsonValue::as_str)
                .is_none_or(|s| !nonblank(s))
            {
                if canonical {
                    return Err(Error::Invalid("canonical variant incomplete"));
                } else {
                    continue;
                }
            }
            let base = format!("/variant_labels/{index}/");
            let mut guards = context.clone();
            if !canonical {
                for (key, _) in variant
                    .as_object()
                    .ok_or(Error::Invalid("variant object"))?
                {
                    let key = key.as_str().ok_or(Error::Invalid("variant field"))?;
                    if key != "value" {
                        guards.push(format!("{base}{}", pointer_token(key)));
                    }
                }
            }
            fields.push(FormField {
                id: format!(
                    "{}.variant-name:{index}",
                    if canonical { "canonical" } else { "metadata" }
                ),
                pointer: format!("{base}value"),
                role: "name".into(),
                language: variant
                    .object_get("language")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
                script: variant
                    .object_get("script")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
                context: guards,
            });
        }
    }
    // Canonical catalogue orders thesis after variant labels, as the owner does.
    if canonical {
        if let Some(at) = fields.iter().position(|f| f.id == "canonical.thesis") {
            let thesis = fields.remove(at);
            fields.push(thesis);
        }
    }
    Ok(fields)
}

pub fn prepare_form_change(
    source: &JsonValue,
    set: Option<&JsonValue>,
    principal: &str,
    id: &str,
    field_id: &str,
) -> Result<JsonValue> {
    let fields = metadata_fields(source)?;
    let selected = fields
        .iter()
        .find(|field| field.id == field_id)
        .ok_or(Error::Invalid("unknown source field selector"))?;
    prepare_form_change_from_fields(source,set,principal,id,selected)
}
// Same pure preparation law; the compound consumer borrows a selected field
// from its one priced catalogue instead of rebuilding that catalogue per form.
pub(crate) fn prepare_form_change_from_fields(source:&JsonValue,set:Option<&JsonValue>,principal:&str,id:&str,selected:&FormField)->Result<JsonValue> {
    let subject = metadata_subject(source)?;
    let empty = empty_set(&subject);
    prepared_change(set.unwrap_or(&empty), &subject, principal, id, selected)
}
pub(crate) fn empty_set(subject: &JsonValue) -> JsonValue {
    object(vec![
        ("schema_version", string("tos_human_form_set_v1")),
        ("subject", subject.clone()),
        ("forms", JsonValue::Array(vec![])),
        ("prior_forms", JsonValue::Array(vec![])),
    ])
}

pub fn apply_form_changes(
    prior_set: Option<&JsonValue>,
    subject: &JsonValue,
    changes: &[JsonValue],
) -> Result<JsonValue> {
    let mut successor = match prior_set {
        Some(v) => parse(&canonical(v)?)?,
        None => empty_set(subject),
    };
    for change in changes {
        exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = field(change, "form")?.clone();
        if !same(field(&form, "subject")?, subject)? {
            return Err(Error::Conflict("form subject changed"));
        }
        let id = text(&form, "form_id")?;
        let at = array(&successor, "forms")?
            .iter()
            .position(|old| old.object_get("form_id").and_then(JsonValue::as_str) == Some(id));
        match text(change, "operation")? {
            "form.create" => {
                if at.is_some()
                    || !field(change, "expected_form")?.is_null()
                    || integer(&form, "form_version")? != 1
                    || !field(&form, "revises")?.is_null()
                {
                    return Err(Error::Conflict("create requires absent initial form"));
                }
                array_mut(&mut successor, "forms")?.push(form);
            }
            "form.revise" => {
                let at = at.ok_or(Error::Conflict("form predecessor absent"))?;
                let old = &array(&successor, "forms")?[at];
                let previous = form_reference(old)?;
                if !same(field(change, "expected_form")?, &previous)?
                    || !same(field(&form, "revises")?, &previous)?
                    || integer(&form, "form_version")?
                        != integer(old, "form_version")?
                            .checked_add(1)
                            .ok_or(Error::Invalid("version overflow"))?
                {
                    return Err(Error::Conflict("revision must extend exact form"));
                }
                let old = std::mem::replace(&mut array_mut(&mut successor, "forms")?[at], form);
                array_mut(&mut successor, "prior_forms")?.push(old);
            }
            _ => return Err(Error::Invalid("unknown form operation")),
        }
    }
    set(&mut successor, "subject", subject.clone())?;
    validate_history(&successor, subject)?;
    Ok(successor)
}

fn stopped(form: &JsonValue, subject: &JsonValue, state: &str, issue: &str) -> Result<JsonValue> {
    Ok(object(vec![
        (
            "schema_version",
            string("tos_human_form_materialization_v1"),
        ),
        ("form", form_reference(form)?),
        ("subject", subject.clone()),
        ("state", string(state)),
        ("display_text", JsonValue::Null),
        ("context", JsonValue::Array(vec![])),
        ("issues", JsonValue::Array(vec![string(issue)])),
        ("admission", JsonValue::Null),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]))
}
pub fn materialize_source_forms(source: &JsonValue, set: &JsonValue) -> Result<Vec<JsonValue>> {
    materialize_source_forms_impl(source,set,None,None).map_err(|error|match error {MaterializeError::Form(error)=>error,MaterializeError::Logical{..}=>Error::Unsupported("form logical workspace")})
}
pub(crate) fn materialize_source_forms_from_fields(source:&JsonValue,set:&JsonValue,fields:&[FormField],available:usize)->std::result::Result<Vec<JsonValue>,crate::item_rules::ItemRefusal> {
    materialize_source_forms_impl(source,set,Some(fields),Some(available)).map_err(|error|match error {
        MaterializeError::Form(error)=>crate::item_rules::ItemRefusal::Unsupported(format!("compound source-copy forms: {error:?}")),
        MaterializeError::Logical{used,limit}=>crate::item_rules::ItemRefusal::BudgetCheck{check:"compound source-copy materialization logical state",used:used.map(|n|n as u64),limit:Some(limit as u64)},
    })
}
enum MaterializeError {Form(FormMechanicsError),Logical{used:Option<usize>,limit:usize}}
impl From<FormMechanicsError> for MaterializeError {fn from(error:FormMechanicsError)->Self {Self::Form(error)}}
fn materialize_source_forms_impl(source:&JsonValue,set:&JsonValue,selected_fields:Option<&[FormField]>,logical_limit:Option<usize>)->std::result::Result<Vec<JsonValue>,MaterializeError> {
    let subject = metadata_subject(source)?;
    validate_history(set, &subject)?;
    if source
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        == Some("tos_canonical_node_v1")
        && array(set, "forms")?
            .iter()
            .chain(array(set, "prior_forms")?)
            .any(|form| {
                form.object_get("content")
                    .and_then(|content| content.object_get("kind"))
                    .and_then(JsonValue::as_str)
                    != Some("source-copy")
            })
    {
        return Err(Error::Denied(
            "canonical current and retained forms permit only source-copy",
        ).into());
    }
    let fields=match selected_fields {Some(fields)=>std::borrow::Cow::Borrowed(fields),None=>std::borrow::Cow::Owned(metadata_fields(source)?)};
    let mut output = Vec::new();
    let mut bytes = 0usize;
    let mut logical=std::mem::size_of::<Vec<JsonValue>>();
    for form in array(set, "forms")? {
        let mut view = materialize_one(source, &subject, set, form, &fields,logical_limit.map(|limit|limit.saturating_sub(logical)))?;
        if source.object_get("claim_id").is_some()
            && !matches!(
                source.object_get("visibility").and_then(JsonValue::as_str),
                Some("public" | "public_metadata_only")
            )
        {
            view = stopped(form, &subject, "restricted", "scope.access-denied")?;
        }
        // Existing output byte law; the private compound caller additionally
        // bounds the temporary emission while earlier views remain retained.
        let wire=if let Some(limit)=logical_limit {
            let tree=crate::record_biblio_cut::ordered_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?;
            let indexes=crate::record_biblio_cut::ordered_emit_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?;
            let base=logical.checked_add(tree).and_then(|n|n.checked_add(indexes)).ok_or(MaterializeError::Logical{used:None,limit})?;
            let room=limit.checked_sub(base).ok_or(MaterializeError::Logical{used:Some(base),limit})?;
            canonical_bytes_v1(&view,CanonicalProfile::SourceCommandInputV1,JsonLimits{max_bytes:room.min(8_388_608),..JsonLimits::default()}).map_err(|error|if error.code==tos_foundation::FoundationErrorCode::BudgetExceeded {MaterializeError::Logical{used:None,limit}}else{MaterializeError::Form(Error::Invalid("source canonical input"))})?
        }else{canonical(&view)?};
        bytes = bytes.checked_add(wire.len()).filter(|size| *size <= 262_144).ok_or(Error::Invalid("form materialization output budget"))?;
        drop(wire);
        if let Some(limit)=logical_limit {
            logical=logical.checked_add(crate::record_biblio_cut::ordered_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?).ok_or(MaterializeError::Logical{used:None,limit})?;
            if logical>limit {return Err(MaterializeError::Logical{used:Some(logical),limit});}
        }
        output.push(view);
    }
    Ok(output)
}
fn materialize_one(
    source: &JsonValue,
    subject: &JsonValue,
    set: &JsonValue,
    form: &JsonValue,
    fields: &[FormField],
    logical_limit:Option<usize>,
) -> std::result::Result<JsonValue,MaterializeError> {
    if !same(field(set, "subject")?, subject)? || !same(field(form, "subject")?, subject)? {
        return stopped(form, subject, "stale", "metadata-adapter.subject-changed").map_err(MaterializeError::Form);
    }
    let content = field(form, "content")?;
    if text(content, "kind")? != "source-copy" {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        ).map_err(MaterializeError::Form);
    }
    let bindings = field(form, "bindings")?;
    let slot = text(content, "slot")?;
    let Some(selected) = bindings.object_get(slot) else {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        ).map_err(MaterializeError::Form);
    };
    if !same(field(selected, "record")?, subject)? {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        ).map_err(MaterializeError::Form);
    }
    let matches = fields
        .iter()
        .filter(|candidate| {
            candidate.role == text(form, "role").unwrap_or("")
                && candidate.pointer == text(selected, "pointer").unwrap_or("")
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(Error::Invalid("ambiguous source field catalogue").into());
    }
    let Some(chosen) = matches.first() else {
        return stopped(
            form,
            subject,
            "unavailable",
            "metadata-adapter.unsupported-role-or-production-mode",
        ).map_err(MaterializeError::Form);
    };
    if !same(field(form, "language")?, &chosen.language)?
        || !same(field(form, "script")?, &chosen.script)?
    {
        return stopped(
            form,
            subject,
            "invalid",
            "source-copy.language-not-bound-to-source",
        ).map_err(MaterializeError::Form);
    }
    if form
        .object_get("language_context")
        .is_some_and(|v| !v.is_null())
    {
        return stopped(
            form,
            subject,
            "invalid",
            "language-context.outside-owner-scope",
        ).map_err(MaterializeError::Form);
    }
    // Resolve every authored binding, including extra slots. A valid wording
    // slot cannot hide a stale or unavailable dependency in another binding.
    for (binding_slot, bound) in bindings
        .as_object()
        .ok_or(Error::Invalid("form binding map"))?
    {
        let binding_slot = binding_slot
            .as_str()
            .ok_or(Error::Invalid("form binding slot"))?;
        if text(field(bound, "record")?, "id")? != text(subject, "id")? {
            return stopped(
                form,
                subject,
                "unavailable",
                &format!("binding.unavailable:{binding_slot}"),
            ).map_err(MaterializeError::Form);
        }
        if !same(field(bound, "record")?, subject)? {
            return stopped(
                form,
                subject,
                "stale",
                &format!("binding.changed:{binding_slot}"),
            ).map_err(MaterializeError::Form);
        }
        if pointer(source, text(bound, "pointer")?).is_err() {
            return stopped(
                form,
                subject,
                "invalid",
                &format!("binding.pointer:{binding_slot}"),
            ).map_err(MaterializeError::Form);
        }
    }
    let mut context = Vec::new();
    for path in &chosen.context {
        let expected = binding(subject, path);
        let matching = bindings
            .as_object()
            .ok_or(Error::Invalid("form binding map"))?
            .iter()
            .find(|(_, value)| same(value, &expected).unwrap_or(false));
        let Some((key, _)) = matching else {
            return stopped(form, subject, "invalid", "context.omitted").map_err(MaterializeError::Form);
        };
        context.push(object(vec![
            (
                "slot",
                string(key.as_str().ok_or(Error::Invalid("form binding slot"))?),
            ),
            ("binding", expected),
            ("value", pointer(source, path)?),
        ]));
    }
    let wording = pointer(source, &chosen.pointer)?;
    if wording.as_str().is_none_or(|s| !nonblank(s)) {
        return stopped(
            form,
            subject,
            "invalid",
            "source-copy.requires-complete-nonempty-string",
        ).map_err(MaterializeError::Form);
    }
    let mut view = object(vec![
        (
            "schema_version",
            string("tos_human_form_materialization_v1"),
        ),
        ("form", form_reference(form)?),
        ("subject", subject.clone()),
        ("state", string("ready")),
        ("display_text", wording),
        ("context", JsonValue::Array(context)),
        ("issues", JsonValue::Array(vec![])),
        ("admission", JsonValue::Null),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
        ("role", string(&chosen.role)),
        ("language", chosen.language.clone()),
        ("script", chosen.script.clone()),
        ("derivation", string("source-copy")),
        ("dependencies", JsonValue::Array(vec![subject.clone()])),
        ("standalone_reading", JsonValue::Bool(false)),
    ]);
    if let Some(limit)=logical_limit {
        let used=crate::record_biblio_cut::ordered_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?;
        if used>limit {return Err(MaterializeError::Logical{used:Some(used),limit});}
    }
    let view_wire=if let Some(limit)=logical_limit {
        let tree=crate::record_biblio_cut::ordered_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?;
        let indexes=crate::record_biblio_cut::ordered_emit_state(&view).map_err(|_|MaterializeError::Logical{used:None,limit})?;
        let base=tree.checked_add(indexes).ok_or(MaterializeError::Logical{used:None,limit})?;
        let room=limit.checked_sub(base).ok_or(MaterializeError::Logical{used:Some(base),limit})?;
        canonical_bytes_v1(&view,CanonicalProfile::SourceCommandInputV1,JsonLimits{max_bytes:room.min(8_388_608),..JsonLimits::default()}).map_err(|error|if error.code==tos_foundation::FoundationErrorCode::BudgetExceeded {MaterializeError::Logical{used:None,limit}}else{MaterializeError::Form(Error::Invalid("source canonical input"))})?
    }else{canonical(&view)?};
    let over_wire=view_wire.len()>65_536;
    drop(view_wire);
    if over_wire {
        view = stopped(
            form,
            subject,
            "over-budget",
            "form.output-budget-exceeded-do-not-truncate",
        )?;
    }
    Ok(view)
}
#[derive(Clone, Debug)]
pub struct FormField {
    pub id: String,
    pub pointer: String,
    pub role: String,
    pub language: JsonValue,
    pub script: JsonValue,
    pub context: Vec<String>,
}

impl FormField {
    pub fn public(&self) -> JsonValue {
        object(vec![
            ("field_id", string(&self.id)),
            ("role", string(&self.role)),
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
        .ok_or(Error::Invalid("invalid source pointer"))?
        .split('/')
    {
        let token = token.replace("~1", "/").replace("~0", "~");
        value = match value {
            JsonValue::Object(_) => value.object_get(&token),
            JsonValue::Array(items) => token.parse::<usize>().ok().and_then(|at| items.get(at)),
            _ => None,
        }
        .ok_or(Error::Invalid("source pointer does not resolve"))?;
    }
    Ok(value.clone())
}

fn binding(subject: &JsonValue, path: &str) -> JsonValue {
    object(vec![("record", subject.clone()), ("pointer", string(path))])
}

pub(crate) fn prepared_change(
    set: &JsonValue,
    subject: &JsonValue,
    principal: &str,
    id: &str,
    field: &FormField,
) -> Result<JsonValue> {
    let old = array(set, "forms")?
        .iter()
        .find(|form| form.object_get("form_id").and_then(JsonValue::as_str) == Some(id));
    let operation = if old.is_some() {
        "form.revise"
    } else {
        "form.create"
    };
    let predecessor = old
        .map(form_reference)
        .transpose()?
        .unwrap_or(JsonValue::Null);
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
    let form = object(vec![
        ("schema_version", string("tos_human_form_v1")),
        ("form_id", string(id)),
        (
            "form_version",
            number(
                old.map(|form| integer(form, "form_version"))
                    .transpose()?
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(Error::Invalid("form version overflow"))?,
            ),
        ),
        ("subject", subject.clone()),
        ("role", string(&field.role)),
        ("language", field.language.clone()),
        ("script", field.script.clone()),
        ("creator_id", string(principal)),
        ("revises", predecessor.clone()),
        ("bindings", JsonValue::Object(bindings)),
        (
            "content",
            object(vec![
                ("kind", string("source-copy")),
                ("slot", string("wording")),
            ]),
        ),
    ]);
    Ok(object(vec![
        ("operation", string(operation)),
        ("expected_form", predecessor),
        ("form", form),
    ]))
}

pub fn validate_history(set: &JsonValue, subject: &JsonValue) -> Result<()> {
    if text(set, "schema_version")? != "tos_human_form_set_v1" {
        return Err(Error::Unsupported("other HumanForm set schema"));
    }
    if text(field(set, "subject")?, "id")? != text(subject, "id")? {
        return Err(Error::Conflict("source and HumanForm subject differ"));
    }
    let current = array(set, "forms")?;
    let prior = array(set, "prior_forms")?;
    if current.len() > 32 || prior.len() > 256 {
        return Err(Error::Unsupported(
            "source form history exceeds command budget",
        ));
    }
    let mut indexed: HashMap<(&str, u64), &JsonValue> = HashMap::new();
    let mut current_ids = HashSet::new();
    for form in prior.iter().chain(current.iter()) {
        let id = text(form, "form_id")?;
        let version = integer(form, "form_version")?;
        if version == 0
            || text(field(form, "subject")?, "id")? != text(subject, "id")?
            || indexed.insert((id, version), form).is_some()
        {
            return Err(Error::Unsupported("broken source form identity history"));
        }
    }
    for form in current {
        if !current_ids.insert(text(form, "form_id")?) {
            return Err(Error::Unsupported("duplicate current source form identity"));
        }
    }
    for ((id, version), form) in &indexed {
        if !current_ids.contains(*id) {
            return Err(Error::Unsupported(
                "prior source form has no current identity",
            ));
        }
        let predecessor = field(form, "revises")?;
        if *version == 1 {
            if !predecessor.is_null() {
                return Err(Error::Unsupported("initial source form has predecessor"));
            }
        } else {
            let Some(previous) = indexed.get(&(*id, version - 1)) else {
                return Err(Error::Unsupported("missing source form predecessor"));
            };
            if !same(predecessor, &form_reference(previous)?)? {
                return Err(Error::Unsupported("wrong source form predecessor"));
            }
        }
    }
    for form in current {
        let id = text(form, "form_id")?;
        let version = integer(form, "form_version")?;
        if indexed
            .keys()
            .any(|(other, next)| *other == id && *next > version)
        {
            return Err(Error::Unsupported("current source form is not latest"));
        }
    }
    let mut command_ids = HashSet::new();
    if let Some(history) = set.object_get("growth_history") {
        for receipt in history
            .as_array()
            .ok_or(Error::Invalid("growth history must be array"))?
        {
            validate_instant(text(receipt, "recorded_at")?)?;
            if !command_ids.insert(text(receipt, "command_id")?)
                || text(field(receipt, "source")?, "id")? != text(subject, "id")?
            {
                return Err(Error::Unsupported("duplicate or foreign source receipt"));
            }
            for reference in array(receipt, "results")? {
                let key = (
                    text(reference, "id")?,
                    integer(reference, "version")?,
                );
                let Some(form) = indexed.get(&key) else {
                    return Err(Error::Unsupported("receipt result not retained"));
                };
                if !same(reference, &form_reference(form)?)? {
                    return Err(Error::Unsupported("receipt result digest differs"));
                }
            }
        }
    }
    Ok(())
}

fn array_mut<'a>(root: &'a mut JsonValue, name: &str) -> Result<&'a mut Vec<JsonValue>> {
    let entries = match root {
        JsonValue::Object(entries) => entries,
        _ => return Err(Error::Invalid("form set is not an object")),
    };
    entries
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .and_then(|(_, value)| match value {
            JsonValue::Array(items) => Some(items),
            _ => None,
        })
        .ok_or(Error::Invalid("missing form-set array"))
}
