//! Shared exact projection mechanics for maintained PG Growth owners.
//!
//! Owner adapters supply their family-specific metadata and Claim paths; this
//! module binds those semantics to the exact sealed write bytes and selected
//! source profile inputs used by the common PG writer.

use crate::source_cohort::SourceRevisionWrite;
use crate::source_command::{self as cmd, SourceFile};
use crate::{DurableError, DurableResult};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath};

pub(crate) struct GrowthRecordProjection {
    pub(crate) kind: String,
    pub(crate) path: String,
    pub(crate) record: JsonValue,
    pub(crate) schema_ref: Option<String>,
}

pub(crate) fn owner_contributions(
    writes: &[SourceRevisionWrite],
    context_files: &[SourceFile],
    records: &[GrowthRecordProjection],
    event_path: &str,
    claim_path: &str,
    claim_owner_schema: &str,
) -> DurableResult<BTreeMap<String, JsonValue>> {
    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt("PG Growth owner write count differs"));
    }
    let mut by_path = BTreeMap::<&str, &SourceRevisionWrite>::new();
    for write in writes {
        let parsed = RelativePath::parse(&write.subject)
            .map_err(|_| DurableError::Corrupt("PG Growth write path"))?;
        if parsed.as_str() != write.subject
            || !write.subject.starts_with("ToS/source-witnesses/")
            || by_path.insert(&write.subject, write).is_some()
        {
            return Err(DurableError::Corrupt("PG Growth owner write path differs"));
        }
    }
    if !by_path.contains_key(event_path) || !by_path.contains_key(claim_path) {
        return Err(DurableError::Corrupt(
            "PG Growth owner evidence write absent",
        ));
    }

    let (base_source_profiles, full_source_profiles) = source_profiles(context_files)?;
    let mut projected_records = BTreeMap::<String, (String, JsonValue)>::new();
    let mut seen_record_paths = BTreeSet::new();
    for record in records {
        let write = by_path
            .get(record.path.as_str())
            .copied()
            .ok_or(DurableError::Corrupt("PG Growth metadata write absent"))?;
        let exact = cmd::parse(&write.exact_bytes).map_err(DurableError::Source)?;
        if &exact != &record.record
            || cmd::text(&exact, "record_type").map_err(DurableError::Source)? != record.kind
            || !crate::source_claims::NATIVE_CATALOG_KINDS.contains(&record.kind.as_str())
            || !seen_record_paths.insert(record.path.as_str())
        {
            return Err(DurableError::Corrupt(
                "PG Growth metadata projection differs",
            ));
        }
        let row = crate::source_claims::pg_catalogue_record_value(
            &exact,
            &record.path,
            record.schema_ref.as_deref(),
        )
        .map_err(DurableError::Source)?;
        if projected_records
            .insert(record.path.clone(), (record.kind.clone(), row))
            .is_some()
        {
            return Err(DurableError::Corrupt("PG Growth metadata path repeats"));
        }
    }

    let events = evidence_projection(event_path, &by_path[event_path].exact_bytes, "event_id")?;
    let claim_owner = claim_owner_projection(
        writes,
        &by_path[claim_path].exact_bytes,
        claim_path,
        claim_owner_schema,
    )?;

    let mut contributions = BTreeMap::new();
    let mut package_form_ids = BTreeSet::new();
    for write in writes {
        let path = write.subject.as_str();
        let mut records = BTreeMap::new();
        for kind in crate::source_claims::MIXED_INVENTORY_RECORD_KINDS {
            let rows = projected_records
                .get(path)
                .filter(|(record_kind, _)| record_kind.as_str() == *kind)
                .map(|(_, row)| vec![row.clone()])
                .unwrap_or_default();
            records.insert(JsonString::from_utf8(kind), JsonValue::Array(rows));
        }
        let event_rows = if path == event_path {
            events.clone()
        } else {
            empty_object()
        };
        let form = if path.ends_with(".human-forms.json") {
            let projection = form_projection(&write.exact_bytes)?;
            for form_id in projection
                .object_get("form_ids")
                .and_then(JsonValue::as_array)
                .ok_or(DurableError::Corrupt(
                    "PG Growth form projection is malformed",
                ))?
            {
                let id = form_id.as_str().ok_or(DurableError::Corrupt(
                    "PG Growth form identity is malformed",
                ))?;
                if !package_form_ids.insert(id.to_owned()) {
                    return Err(DurableError::Corrupt(
                        "PG Growth form identity repeats across package members",
                    ));
                }
            }
            projection
        } else {
            JsonValue::Null
        };
        let owner = if path == claim_path {
            claim_owner.clone()
        } else {
            JsonValue::Null
        };
        let contribution = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_pg_source_inventory_member_contribution_v1"),
            ),
            ("records", JsonValue::Object(records)),
            (
                "source_profiles",
                if projected_records.contains_key(path)
                    || path == claim_path
                    || path.ends_with(".human-forms.json")
                {
                    full_source_profiles.clone()
                } else {
                    base_source_profiles.clone()
                },
            ),
            ("events", event_rows),
            ("anchors", empty_object()),
            ("form", form),
            ("claim_owner", owner),
        ]);
        if contributions
            .insert(path.to_owned(), contribution)
            .is_some()
        {
            return Err(DurableError::Corrupt("PG Growth contribution path repeats"));
        }
    }
    if contributions.len() != writes.len() {
        return Err(DurableError::Corrupt(
            "PG Growth contribution coverage differs",
        ));
    }
    Ok(contributions)
}

fn source_profiles(context_files: &[SourceFile]) -> DurableResult<(JsonValue, JsonValue)> {
    let mut base = BTreeMap::<String, String>::new();
    let mut full = BTreeMap::<String, String>::new();
    for file in context_files {
        let path = file.path.as_str();
        let digest = Digest256::of_bytes(&file.raw).to_hex();
        if path == "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            || path == "ToS/contracts/semantic-entity-type-registry.schema.json"
        {
            base.insert(path.to_owned(), digest.clone());
            full.insert(path.to_owned(), digest);
        } else if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            if full.insert(path.to_owned(), digest).is_some() {
                return Err(DurableError::Corrupt("PG Growth source profile repeats"));
            }
        }
    }
    if base.len() != 2 {
        return Err(DurableError::Corrupt(
            "PG Growth base source profile is incomplete",
        ));
    }
    let as_json = |profiles: BTreeMap<String, String>| {
        JsonValue::Object(
            profiles
                .into_iter()
                .map(|(path, digest)| (JsonString::from_utf8(&path), cmd::string(&digest)))
                .collect(),
        )
    };
    Ok((as_json(base), as_json(full)))
}

fn evidence_projection(path: &str, raw: &[u8], id_field: &str) -> DurableResult<JsonValue> {
    let mut entries = BTreeMap::new();
    for (physical_line, line) in raw.split(|byte| *byte == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let payload = cmd::parse(line).map_err(DurableError::Source)?;
        let id = cmd::text(&payload, id_field)
            .map_err(DurableError::Source)?
            .to_owned();
        if id.trim().is_empty() {
            return Err(DurableError::Corrupt(
                "PG Growth evidence identity is empty",
            ));
        }
        let digest = cmd::record_digest(&payload).map_err(DurableError::Source)?;
        let value = cmd::object(vec![
            ("payload", payload),
            ("source_ref", cmd::string(path)),
            ("source_line", cmd::number(physical_line + 1)),
            ("source_sha256", cmd::string(&digest.to_hex())),
        ]);
        if entries.insert(id, value).is_some() {
            return Err(DurableError::Corrupt("PG Growth evidence identity repeats"));
        }
    }
    if entries.is_empty() {
        return Err(DurableError::Corrupt("PG Growth evidence stream is empty"));
    }
    Ok(JsonValue::Object(
        entries
            .into_iter()
            .map(|(id, value)| (JsonString::from_utf8(&id), value))
            .collect(),
    ))
}

fn form_projection(raw: &[u8]) -> DurableResult<JsonValue> {
    let value = cmd::parse(raw).map_err(DurableError::Source)?;
    let mut form_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for section in ["forms", "prior_forms"] {
        let rows = cmd::array(&value, section).map_err(DurableError::Source)?;
        for form in rows {
            let id = cmd::text(form, "form_id")
                .map_err(DurableError::Source)?
                .to_owned();
            if id.is_empty() {
                return Err(DurableError::Corrupt("PG Growth form identity repeats"));
            }
            if seen.insert(id.clone()) {
                form_ids.push(cmd::string(&id));
            }
        }
    }
    Ok(cmd::object(vec![
        (
            "raw_sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(form_ids)),
    ]))
}

fn claim_owner_projection(
    writes: &[SourceRevisionWrite],
    stream_raw: &[u8],
    source_path: &str,
    schema_version: &str,
) -> DurableResult<JsonValue> {
    let home = source_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Corrupt("PG Growth Claim home is absent"))?;
    let claims = json_lines(stream_raw)?;
    if claims.is_empty() {
        return Err(DurableError::Corrupt("PG Growth Claim stream is empty"));
    }
    let mut direct_members = Vec::new();
    let mut roles = BTreeMap::<&'static str, usize>::new();
    for write in writes {
        let Some((parent, basename)) = write.subject.rsplit_once('/') else {
            continue;
        };
        if parent != home {
            continue;
        }
        let role = match basename {
            "source-claims.jsonl" => Some("claim-stream"),
            "source-create-request.json" => Some("creation-request"),
            "source-create-environment.json" => Some("creation-environment"),
            "source-create-provenance.jsonl" => Some("creation-event"),
            "source-create-receipt.json"
            | "work-expression-receipt.json"
            | "object-link-creation-receipt.json"
            | "expression-edition-receipt.json"
            | "responsibility-attachment-receipt.json"
            | "membership-attachment-receipt.json" => Some("creation-receipt"),
            "claim-revision-history.json" => Some("revision-history"),
            _ if basename.ends_with(".human-forms.json") => Some("human-form-set"),
            _ => None,
        };
        let Some(role) = role else {
            continue;
        };
        *roles.entry(role).or_default() += 1;
        direct_members.push(cmd::object(vec![
            ("ref", cmd::string(&write.subject)),
            ("role", cmd::string(role)),
            ("bytes", cmd::number(write.exact_bytes.len())),
            (
                "sha256",
                cmd::string(&Digest256::of_bytes(&write.exact_bytes).to_prefixed()),
            ),
        ]));
    }
    direct_members.sort_by(|left, right| {
        cmd::text(left, "ref")
            .unwrap_or_default()
            .as_bytes()
            .cmp(cmd::text(right, "ref").unwrap_or_default().as_bytes())
    });
    for role in [
        "claim-stream",
        "creation-request",
        "creation-environment",
        "creation-event",
        "creation-receipt",
    ] {
        if roles.get(role).copied() != Some(1) {
            return Err(DurableError::Corrupt(
                "PG Growth Claim owner member set differs",
            ));
        }
    }
    if roles.get("human-form-set").copied().unwrap_or_default() == 0 {
        return Err(DurableError::Corrupt("PG Growth Claim form set is absent"));
    }
    let core = cmd::object(vec![
        ("schema_version", cmd::string(schema_version)),
        ("package_home", cmd::string(home)),
        ("source_path", cmd::string(source_path)),
        (
            "stream_sha256",
            cmd::string(&Digest256::of_bytes(stream_raw).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        ("claims", JsonValue::Array(claims)),
    ]);
    let closure = cmd::record_digest(&core).map_err(DurableError::Source)?;
    let mut owner = core;
    cmd::set(
        &mut owner,
        "closure_sha256",
        cmd::string(&closure.to_prefixed()),
    )
    .map_err(DurableError::Source)?;
    Ok(owner)
}

fn json_lines(raw: &[u8]) -> DurableResult<Vec<JsonValue>> {
    let mut rows = Vec::new();
    for line in raw.split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        rows.push(cmd::parse(line).map_err(DurableError::Source)?);
    }
    Ok(rows)
}

fn empty_object() -> JsonValue {
    JsonValue::Object(BTreeMap::new())
}
