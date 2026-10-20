//! Lossless wire selection over owner-supplied snapshot descriptors.
//! Symbolic wire tokens are compared as tokens, never promoted to physical source
//! digests. Access verification and persistence remain with the existing wrapper
//! and authoritative writer. Returned manifests are constructed values only.
use crate::query_context::same_json;
use crate::temporal::{TemporalBudget, TemporalError};
use crate::{JsonString, JsonValue};
fn obj(v: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        v.into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn get<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn put(v: &mut JsonValue, k: &str, x: JsonValue) {
    if let JsonValue::Object(v) = v {
        if let Some((_, p)) = v.iter_mut().find(|(key, _)| key.as_str() == Some(k)) {
            *p = x;
        } else {
            v.push((JsonString::from_utf8(k), x));
        }
    }
}
fn status(s: &str, r: &str) -> JsonValue {
    obj(vec![("status", text(s)), ("reason", text(r))])
}
fn required<'a>(v: &'a JsonValue, k: &'static str) -> Result<&'a JsonValue, TemporalError> {
    let x = get(v, k);
    if x.is_null() {
        Err(TemporalError::MalformedRequest(k))
    } else {
        Ok(x)
    }
}
fn validate_attributed(value: &JsonValue) -> Result<(), TemporalError> {
    if !value.is_null()
        && (get(value, "knower").as_str().is_none_or(|s| s.is_empty())
            || get(value, "cutoff").is_null())
    {
        return Err(TemporalError::MalformedRequest(
            "attributed knowledge requires knower and cutoff",
        ));
    }
    Ok(())
}
fn concrete_state(state: &JsonValue, snapshot: &JsonValue) -> Result<JsonValue, TemporalError> {
    if state.as_str() == Some("Current") || get(state, "type").as_str() == Some("Current") {
        let cutoff = required(snapshot, "resolved_system_cutoff")?;
        let mut resolved = obj(vec![
            ("type", text("SystemAsOf")),
            ("cutoff", cutoff.clone()),
        ]);
        if let Some(trs) = snapshot.object_get("system_trs_ref") {
            put(&mut resolved, "trs_ref", trs.clone());
        }
        Ok(resolved)
    } else if get(state, "type").as_str() == Some("SystemAsOf") && !get(state, "cutoff").is_null() {
        Ok(state.clone())
    } else {
        Err(TemporalError::MalformedRequest(
            "explicit Current or SystemAsOf required",
        ))
    }
}
fn same_state(a: &JsonValue, b: &JsonValue) -> bool {
    // An omitted wire TRS does not invent one. A supplied conflicting TRS cannot match.
    get(a, "type") == get(b, "type")
        && get(a, "cutoff") == get(b, "cutoff")
        && (a.object_get("trs_ref").is_none() || get(a, "trs_ref") == get(b, "trs_ref"))
}
/// Pure selector resolution. The caller must verify source descriptor provenance.
pub fn resolve_context_json(
    input: &JsonValue,
    _context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let envelope = required(input, "request_envelope")?;
    let snapshot = required(envelope, "source_snapshot_ref")?;
    let request = required(envelope, "temporal")?;
    required(snapshot, "snapshot_id")?;
    required(snapshot, "revision_fence")?;
    validate_attributed(get(request, "attributed_knowledge_as_of"))?;
    let manifest = get(input, "snapshot_manifest");
    let frozen = get(manifest, "frozen_knowledge_selection");
    let is_frozen = !get(request, "chronology_version").is_null();
    let explicit = request.object_get("system_state");
    let system = explicit.map(|s| concrete_state(s, snapshot)).transpose()?;
    let world = get(request, "world_valid_time").clone();
    if is_frozen {
        if get(request, "chronology_version") != get(manifest, "chronology_version_id") {
            return Ok(status("Unresolved", "ChronologyManifestMismatch"));
        }
        if frozen.as_object().is_none()
            || get(get(frozen, "system_state"), "type").as_str() != Some("SystemAsOf")
            || get(frozen, "member_claim_revisions").as_array().is_none()
        {
            return Err(TemporalError::MalformedRequest("invalid frozen selection"));
        }
        required(get(frozen, "source_snapshot_ref"), "snapshot_id")?;
        required(frozen, "source_revision_fence")?;
        required(get(frozen, "system_state"), "cutoff")?;
        let members = get(frozen, "member_claim_revisions").as_array().unwrap();
        if members.len() > budget.max_members {
            return Err(TemporalError::ResourceExhausted("chronology members"));
        }
        let mut mismatches = Vec::new();
        if get(snapshot, "snapshot_id") != get(get(frozen, "source_snapshot_ref"), "snapshot_id") {
            mismatches.push(text("source_snapshot_ref"));
        }
        if get(snapshot, "revision_fence") != get(frozen, "source_revision_fence") {
            mismatches.push(text("source_revision_fence"));
        }
        if system
            .as_ref()
            .is_some_and(|s| !same_state(s, get(frozen, "system_state")))
        {
            mismatches.push(text("system_state"));
        }
        for key in ["attributed_knowledge_as_of", "source_publication_as_of"] {
            if request
                .object_get(key)
                .is_some_and(|v| !same_json(v, get(frozen, key)))
            {
                mismatches.push(text(key));
            }
        }
        if !mismatches.is_empty() {
            let mut r = obj(vec![
                ("status", text("ContextMismatch")),
                ("mismatched_dimensions", JsonValue::Array(mismatches)),
                ("result", JsonValue::Null),
            ]);
            if explicit.is_some_and(|s| s.as_str() == Some("Current")) {
                put(
                    &mut r,
                    "resolved_current",
                    obj(vec![
                        ("cutoff", get(snapshot, "resolved_system_cutoff").clone()),
                        ("revision_fence", get(snapshot, "revision_fence").clone()),
                    ]),
                );
            }
            return Ok(r);
        }
        let mut r = obj(vec![
            ("status", text("Resolved")),
            ("effective_knowledge_selection", frozen.clone()),
            ("world_valid_time", world),
        ]);
        if explicit.is_some()
            || request.object_get("attributed_knowledge_as_of").is_some()
            || request.object_get("source_publication_as_of").is_some()
        {
            put(&mut r, "selector_mode", text("ExactMatch"));
        }
        return Ok(r);
    }
    let Some(system) = system else {
        return Ok(status("MissingSelection", "SystemStateRequired"));
    };
    let selection = obj(vec![
        ("system_state", system),
        (
            "attributed_knowledge_as_of",
            get(request, "attributed_knowledge_as_of").clone(),
        ),
        (
            "source_publication_as_of",
            get(request, "source_publication_as_of").clone(),
        ),
        (
            "source_snapshot_ref",
            obj(vec![(
                "snapshot_id",
                required(snapshot, "snapshot_id")?.clone(),
            )]),
        ),
        (
            "source_revision_fence",
            required(snapshot, "revision_fence")?.clone(),
        ),
    ]);
    Ok(obj(vec![
        ("status", text("Resolved")),
        ("effective_knowledge_selection", selection),
        ("world_valid_time", world),
    ]))
}
/// Construct a frozen manifest; does not claim a committed source transaction.
pub fn create_chronology_json(
    input: &JsonValue,
    _context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let snapshot = required(input, "source_snapshot_ref")?;
    let request = required(input, "selection_request")?;
    validate_attributed(get(request, "attributed_knowledge_as_of"))?;
    let claims = required(input, "claim_revisions")?;
    let members = claims.as_array().ok_or(TemporalError::MalformedRequest(
        "claim revisions array required",
    ))?;
    if members.len() > budget.max_members {
        return Err(TemporalError::ResourceExhausted("chronology members"));
    }
    for (i, m) in members.iter().enumerate() {
        if m.is_null() || members[..i].iter().any(|v| same_json(v, m)) {
            return Err(TemporalError::MalformedRequest(
                "exact unique member revisions required",
            ));
        }
    }
    let selection = obj(vec![
        (
            "system_state",
            concrete_state(required(request, "system_state")?, snapshot)?,
        ),
        (
            "attributed_knowledge_as_of",
            get(request, "attributed_knowledge_as_of").clone(),
        ),
        (
            "source_publication_as_of",
            get(request, "source_publication_as_of").clone(),
        ),
        (
            "source_snapshot_ref",
            obj(vec![(
                "snapshot_id",
                required(snapshot, "snapshot_id")?.clone(),
            )]),
        ),
        (
            "source_revision_fence",
            required(snapshot, "revision_fence")?.clone(),
        ),
        (
            "evidence_policy_version",
            required(input, "evidence_policy_version")?.clone(),
        ),
        (
            "selection_policy_version",
            required(input, "selection_policy_version")?.clone(),
        ),
        ("member_claim_revisions", claims.clone()),
    ]);
    Ok(obj(vec![
        ("status", text("ChronologyVersionCreated")),
        ("frozen_knowledge_selection", selection),
        ("persisted", JsonValue::Bool(false)),
    ]))
}
