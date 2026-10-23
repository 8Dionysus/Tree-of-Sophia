//! Pure verification of a supplied source wrapper. Neither source membership,
//! storage, writer authority nor conversion-policy admission is established.
use super::*;
use crate::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, FoundationErrorCode, JsonLimits,
    JsonMode, RecordVersion, RelativePath, SourceRevision, StableId, canonical_count_v1,
    canonical_digest_v1, parse_json,
};

fn invalid(message: &'static str) -> TemporalError {
    TemporalError::MalformedRequest(message)
}
fn exhausted() -> TemporalError {
    TemporalError::ResourceExhausted("stored crosswalk budget")
}
fn json_error(error: FoundationError) -> TemporalError {
    if error.code == FoundationErrorCode::BudgetExceeded {
        exhausted()
    } else {
        invalid("invalid stored crosswalk JSON")
    }
}
fn exact(value: &JsonValue, keys: &[&str]) -> Result<(), TemporalError> {
    if !value.as_object().is_some_and(|v| {
        v.len() == keys.len() && keys.iter().all(|key| value.object_get(key).is_some())
    }) {
        return Err(invalid("stored crosswalk object fields"));
    }
    Ok(())
}
fn nonempty<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str, TemporalError> {
    string(value, key)
        .filter(|v| !v.trim().is_empty())
        .ok_or(invalid("stored crosswalk nonempty string required"))
}
fn digest(value: &JsonValue) -> Result<Digest256, TemporalError> {
    Digest256::from_prefixed(value.as_str().ok_or(invalid("digest required"))?)
        .map_err(|_| invalid("invalid prefixed digest"))
}
fn reference(value: &JsonValue) -> Result<ExactRecordRef, TemporalError> {
    exact(value, &["id", "version", "digest"])?;
    Ok(ExactRecordRef {
        id: StableId::parse(nonempty(value, "id")?).map_err(|_| invalid("invalid source id"))?,
        version: RecordVersion::new(
            field(value, "version")
                .as_u64()
                .ok_or(invalid("source version required"))?,
        )
        .map_err(|_| invalid("invalid source version"))?,
        digest: digest(field(value, "digest"))?,
    })
}
fn raw_reference(value: &JsonValue) -> Result<(RelativePath, Digest256, u64), TemporalError> {
    exact(value, &["path", "sha256", "size_bytes"])?;
    Ok((
        RelativePath::parse(nonempty(value, "path")?)
            .map_err(|_| invalid("invalid raw member path"))?,
        digest(field(value, "sha256"))?,
        field(value, "size_bytes")
            .as_u64()
            .ok_or(invalid("raw byte size required"))?,
    ))
}
fn source_id(id: &str) -> bool {
    id.strip_prefix("tos.temporal-crosswalk.")
        .is_some_and(|tail| {
            !tail.is_empty()
                && tail.split(['.', '-']).all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                })
        })
}
fn metadata(root: &JsonValue, owner: &ExactRecordRef) -> Result<(), TemporalError> {
    let fields = root.as_object().ok_or(invalid("source wrapper required"))?;
    let allowed = [
        "schema_version",
        "record_type",
        "record_id",
        "record_version",
        "preferred_label",
        "identity_status",
        "source_refs",
        "external_identifiers",
        "same_as_posture",
        "visibility",
        "variant_labels",
        "field_languages",
        "notes",
        "supersedes_ref",
        "extensions",
        "semantic_content",
    ];
    if fields
        .iter()
        .any(|(key, _)| !key.as_str().is_some_and(|key| allowed.contains(&key)))
    {
        return Err(invalid("unknown source wrapper field"));
    }
    if string(root, "record_type") != Some("temporal-crosswalk")
        || string(root, "record_id") != Some(owner.id.as_str())
        || !source_id(owner.id.as_str())
        || field(root, "record_version").as_u64() != Some(owner.version.get())
        || !matches!(
            string(root, "identity_status"),
            Some("provisional" | "verified" | "disputed" | "superseded")
        )
        || !matches!(
            string(root, "visibility"),
            Some(
                "local_only"
                    | "research_group"
                    | "permission_requested"
                    | "public_metadata_only"
                    | "public"
            )
        )
        || !matches!(
            string(root, "same_as_posture"),
            Some(
                "not_assessed"
                    | "no_equivalence_claim"
                    | "candidate_equivalence"
                    | "reviewed_equivalence"
                    | "reviewed_distinct"
            )
        )
    {
        return Err(invalid("source wrapper identity or metadata differs"));
    }
    nonempty(root, "preferred_label")?;
    let refs = field(root, "source_refs")
        .as_array()
        .ok_or(invalid("source refs required"))?;
    if refs.is_empty()
        || refs
            .iter()
            .any(|v| !v.as_str().is_some_and(|s| !s.is_empty()))
    {
        return Err(invalid("source refs must be nonempty strings"));
    }
    if field(root, "external_identifiers").as_array().is_none() {
        return Err(invalid("external identifiers array required"));
    }
    // This codec is not the source owner's schema worker. Optional descriptive
    // metadata remains intact; source admission validates its complete schema.
    Ok(())
}
fn payload(
    root: &JsonValue,
    owner: &ExactRecordRef,
    budget: TemporalBudget,
) -> Result<Option<&'static str>, TemporalError> {
    let p = field(root, "semantic_content");
    if p.as_object().is_none() {
        return Err(invalid("typed semantic_content required"));
    }
    if string(p, "profile") != Some("tos_stored_finite_crosswalk_v1") {
        return Ok(Some("UnsupportedStoredCrosswalkProfile"));
    }
    if string(p, "rule_profile") != Some("finite-mapping-v1") {
        return Ok(Some("UnsupportedCrosswalkRuleProfile"));
    }
    exact(
        p,
        &[
            "profile",
            "source_trs",
            "target_trs",
            "rule_profile",
            "rules_version",
            "applicability",
            "rows",
            "predecessor_source_record_ref",
            "provenance",
        ],
    )?;
    for key in ["source_trs", "target_trs"] {
        let axis = field(p, key);
        exact(axis, &["id", "version"])?;
        nonempty(axis, "id")?;
        nonempty(axis, "version")?;
    }
    nonempty(p, "rules_version")?;
    let a = field(p, "applicability");
    exact(a, &["min", "max", "unit", "origin"])?;
    let lo = integer(field(a, "min")).ok_or(invalid("integer applicability min required"))?;
    let hi = integer(field(a, "max")).ok_or(invalid("integer applicability max required"))?;
    if lo > hi {
        return Err(invalid("inverted applicability"));
    }
    nonempty(a, "unit")?;
    nonempty(a, "origin")?;
    let rows = field(p, "rows")
        .as_object()
        .ok_or(invalid("finite rows object required"))?;
    if rows.len() > budget.max_pairings || rows.len() > budget.max_members {
        return Err(exhausted());
    }
    for (key, value) in rows {
        let spelling = key.as_str().ok_or(invalid("integer row key required"))?;
        let key: i64 = spelling
            .parse()
            .map_err(|_| invalid("integer row key required"))?;
        if key.to_string() != spelling || key < lo || key > hi || integer(value).is_none() {
            return Err(invalid("noncanonical or out-of-coverage finite row"));
        }
    }
    if !field(p, "provenance")
        .as_object()
        .is_some_and(|v| !v.is_empty())
    {
        return Err(invalid("nonempty provenance object required"));
    }
    let prior = field(p, "predecessor_source_record_ref");
    if owner.version.get() == 1 {
        if !prior.is_null() {
            return Err(invalid("first version has predecessor"));
        }
    } else {
        let prior = reference(prior)?;
        if prior.id != owner.id || prior.version.get().checked_add(1) != Some(owner.version.get()) {
            return Err(invalid("predecessor stream or version differs"));
        }
    }
    Ok(None)
}

/// Verify only the supplied wrapper and its distinct raw/canonical identities.
/// The caller's real source reader and policy owner retain their own authority.
pub fn decode_stored_crosswalk_record(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    if budget.max_expression_bytes == 0 || budget.max_members == 0 || budget.max_pairings == 0 {
        return Err(exhausted());
    }
    let limits = JsonLimits {
        max_bytes: budget.max_expression_bytes,
        max_depth: 32,
        max_visits: budget.max_members,
        max_integer_digits: 20,
    };
    let count = |v: &JsonValue| {
        canonical_count_v1(v, CanonicalProfile::SourceRecordDigestV1, limits).map_err(json_error)
    };
    let input_bytes = count(input)?;
    let context_bytes = count(context)?;
    if context.as_object().is_none() {
        return Err(invalid("context object required"));
    }
    exact(
        input,
        &[
            "schema_version",
            "record",
            "requested_source_record_ref",
            "requested_raw_member_ref",
            "expected_source_revision",
        ],
    )?;
    if string(input, "schema_version") != Some("tos_stored_crosswalk_decode_request_v1") {
        return Err(invalid("stored crosswalk request schema"));
    }
    let env = field(input, "record");
    exact(
        env,
        &[
            "source_record_ref",
            "raw_member_ref",
            "source_revision",
            "record_bytes",
        ],
    )?;
    let owner = reference(field(env, "source_record_ref"))?;
    let raw_ref = raw_reference(field(env, "raw_member_ref"))?;
    if owner != reference(field(input, "requested_source_record_ref"))?
        || raw_ref != raw_reference(field(input, "requested_raw_member_ref"))?
        || SourceRevision(digest(field(env, "source_revision"))?)
            != SourceRevision(digest(field(input, "expected_source_revision"))?)
    {
        return Err(invalid("requested source/raw reference or fence differs"));
    }
    let raw = string(env, "record_bytes").ok_or(invalid("UTF8 wrapper bytes required"))?;
    if raw.len() > budget.max_expression_bytes {
        return Err(exhausted());
    }
    if u64::try_from(raw.len()).ok() != Some(raw_ref.2)
        || Digest256::of_bytes(raw.as_bytes()) != raw_ref.1
    {
        return Err(invalid("raw wrapper digest or size differs"));
    }
    let document =
        parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits).map_err(json_error)?;
    let root = document.root();
    if canonical_digest_v1(root, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(json_error)?
        != owner.digest
    {
        return Err(invalid("canonical source record digest differs"));
    }
    let unsupported = if string(root, "schema_version") != Some("tos_temporal_crosswalk_record_v1")
    {
        Some("UnsupportedStoredCrosswalkSchema")
    } else {
        metadata(root, &owner)?;
        payload(root, &owner, budget)?
    };
    let mut out = obj(vec![
        (
            "schema_version",
            text("tos_stored_crosswalk_decode_result_v1"),
        ),
        (
            "status",
            text(if unsupported.is_some() {
                "Unsupported"
            } else {
                "DecodedCrosswalkSourceWrapper"
            }),
        ),
        ("source_record_ref", field(env, "source_record_ref").clone()),
        ("raw_member_ref", field(env, "raw_member_ref").clone()),
        ("source_revision", field(env, "source_revision").clone()),
        ("authentication", text("SuppliedBytesOnly")),
        ("conversion_policy_admission", text("NotEstablished")),
        ("writer_performed", JsonValue::Bool(false)),
    ]);
    if let Some(reason) = unsupported {
        put(&mut out, "reason", text(reason));
    }
    // Admit full retained evidence before cloning. Fixed headroom covers new
    // field names/punctuation; final closed JSON visitor checks the exact result.
    let required = [
        count(&out)?,
        input_bytes,
        context_bytes,
        count(root)?,
        count(field(root, "semantic_content"))?,
        128,
    ]
    .into_iter()
    .try_fold(0usize, |n, v| n.checked_add(v))
    .ok_or_else(exhausted)?;
    if required > budget.max_expression_bytes {
        return Err(exhausted());
    }
    put(&mut out, "source_record", root.clone());
    put(
        &mut out,
        "semantic_content",
        field(root, "semantic_content").clone(),
    );
    put(&mut out, "preserved_input", input.clone());
    put(&mut out, "preserved_context", context.clone());
    count(&out)?;
    Ok(out)
}
