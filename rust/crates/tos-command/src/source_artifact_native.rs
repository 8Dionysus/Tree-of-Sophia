//! Artifact creation rules over the already selected source cut.
//! Existing owner inputs are observed; no rights/discovery decision is issued.
use crate::source_command::{self as cmd, *};
use crate::source_revisions as revisions;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

pub(crate) const SCHEMA: &str = "ToS/contracts/artifact-source-witness-v2.schema.json";
const INPUTS: [&str; 3] = ["rights_ref", "discovery_ref", "research_ref"];
fn public_path(value: &str) -> SourceCommandResult<RelativePath> {
    let path = RelativePath::parse(value)
        .map_err(|_| SourceCommandError::Denied("Artifact public input path"))?;
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() < 3
        || parts[0] != "ToS"
        || parts.iter().any(|p| {
            p.starts_with('.') || ["payload", "local-content", "owner-local", "catalog"].contains(p)
        })
    {
        return Err(SourceCommandError::Denied(
            "Artifact private or derived input",
        ));
    }
    Ok(path)
}
pub(crate) fn configuration(config: &JsonValue) -> SourceCommandResult<()> {
    let path = cmd::text(config, "source_path")?;
    public_path(path)?;
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() != 7
        || parts[..3] != ["ToS", "source-witnesses", "artifacts"]
        || parts[6] != "artifact-witness.json"
        || parts[3..6].iter().any(|p| p.eq_ignore_ascii_case("cdli"))
        || !revisions::valid_id(cmd::text(config, "record_id")?, "tos.artifact.", false)
    {
        return Err(SourceCommandError::Denied(
            "Artifact provider-independent identity home",
        ));
    }
    bindings(cmd::field(config, "source_bindings")?)
}
fn bindings(value: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(value, &INPUTS)?;
    let mut seen = BTreeSet::new();
    for field in INPUTS {
        let binding = cmd::field(value, field)?;
        cmd::exact_keys(binding, &["ref", "sha256"])?;
        let reference = cmd::text(binding, "ref")?;
        public_path(reference)?;
        let digest = cmd::text(binding, "sha256")?;
        if !seen.insert(reference)
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(SourceCommandError::Denied(
                "Artifact distinct exact byte bindings",
            ));
        }
        if field == "discovery_ref"
            && !reference.starts_with("ToS/source-witnesses/discovery/runs/")
        {
            return Err(SourceCommandError::Denied("Artifact discovery owner route"));
        }
    }
    Ok(())
}
pub(crate) fn initial(
    ctx: &CommandContext,
    config: &JsonValue,
    record: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<String>> {
    let selected = cmd::field(config, "source_bindings")?;
    bindings(selected)?;
    let maker = cmd::object(vec![
        ("maker_type", cmd::field(config, "maker_type")?.clone()),
        ("agent_ref", cmd::field(config, "principal_id")?.clone()),
        ("human_review_performed", JsonValue::Bool(false)),
    ]);
    if cmd::text(record, "schema_version")? != "tos_artifact_source_witness_v2"
        || cmd::text(record, "artifact_id")? != cmd::text(config, "record_id")?
        || cmd::integer(record, "record_version")? != 1
        || cmd::text(cmd::field(record, "authority")?, "review_status")? != "unreviewed"
        || !cmd::same(cmd::field(record, "maker")?, &maker)?
        || cmd::text(record, "provenance_event_ref")? != cmd::text(config, "provenance_event_id")?
        || !cmd::array(record, "philosophy_planting_refs")?.is_empty()
    {
        return Err(SourceCommandError::Denied(
            "Artifact unreviewed initial delegated identity",
        ));
    }
    let mut stack = vec![record];
    while let Some(value) = stack.pop() {
        if let Some(rows) = value.as_object() {
            for (key, child) in rows {
                if [
                    "text",
                    "source_text",
                    "transliteration",
                    "translation",
                    "image_data",
                    "line_art_data",
                    "payload",
                ]
                .contains(&key.as_str().unwrap_or(""))
                {
                    return Err(SourceCommandError::Denied("Artifact content payload"));
                }
                stack.push(child);
            }
        } else if let Some(rows) = value.as_array() {
            stack.extend(rows);
        } else if let Some(text) = value.as_str() {
            if ["/srv/", "/home/", "/tmp/", "/var/tmp/"]
                .iter()
                .any(|p| text.starts_with(p))
            {
                return Err(SourceCommandError::Denied(
                    "Artifact local storage disclosure",
                ));
            }
        }
    }
    revisions::schema(
        worker,
        deadline,
        cancelled,
        ctx,
        &[SCHEMA.into()],
        SCHEMA,
        record,
    )?;
    let mut resources = vec![SCHEMA.into()];
    let mut inputs = Vec::new();
    for field in INPUTS {
        let binding = cmd::field(selected, field)?;
        let reference = cmd::text(binding, "ref")?;
        if cmd::text(record, field)? != reference {
            return Err(SourceCommandError::Denied(
                "Artifact input reference changed",
            ));
        }
        let raw = ctx
            .file(&public_path(reference)?)?
            .ok_or(SourceCommandError::Conflict(
                "Artifact selected input unavailable",
            ))?;
        if Digest256::of_bytes(raw).to_hex() != cmd::text(binding, "sha256")? {
            return Err(SourceCommandError::Conflict(
                "Artifact selected input bytes stale",
            ));
        }
        if field != "research_ref" {
            let schema = if field == "rights_ref" {
                "ToS/contracts/rights-record.schema.json"
            } else {
                "ToS/contracts/material-discovery-record.schema.json"
            };
            let value = cmd::parse(raw)?;
            revisions::schema(
                worker,
                deadline,
                cancelled,
                ctx,
                &[schema.into()],
                schema,
                &value,
            )?;
            resources.push(schema.into());
            inputs.push(value);
        }
        resources.push(reference.into());
    }
    let rights = &inputs[0];
    let discovery = &inputs[1];
    let id = cmd::text(record, "artifact_id")?;
    if !cmd::array(rights, "scope_refs")?
        .iter()
        .any(|v| v.as_str() == Some(id))
        || cmd::text(rights, "visibility")? != "public_metadata_only"
        || cmd::text(rights, "redistribution_posture")? != "metadata_only"
    {
        return Err(SourceCommandError::Denied(
            "Artifact exact metadata-only rights scope",
        ));
    }
    let target = cmd::field(discovery, "target")?;
    if cmd::text(target, "target_kind")? != "artifact"
        || !cmd::array(target, "known_tos_refs")?
            .iter()
            .any(|v| v.as_str() == Some(id))
    {
        return Err(SourceCommandError::Denied(
            "Artifact discovery physical identity",
        ));
    }
    for fingerprint in cmd::array(
        cmd::field(record, "digital_catalog_record")?,
        "response_fingerprints",
    )? {
        if cmd::field(fingerprint, "captured")? != &JsonValue::Bool(true) {
            continue;
        }
        let mut matched = false;
        for channel in cmd::array(discovery, "channels")? {
            for result in cmd::array(channel, "results")? {
                let snapshot = cmd::field(result, "snapshot")?;
                let acquisition = cmd::field(result, "acquisition")?;
                if cmd::field(result, "result_url")? == cmd::field(fingerprint, "surface")?
                    && cmd::text(snapshot, "state")? == "captured"
                    && snapshot.object_get("sha256") == fingerprint.object_get("sha256")
                    && acquisition.object_get("downloaded") == Some(&JsonValue::Bool(true))
                    && acquisition.object_get("sha256") == fingerprint.object_get("sha256")
                    && acquisition.object_get("byte_size") == fingerprint.object_get("byte_size")
                {
                    matched = true;
                }
            }
        }
        if !matched {
            return Err(SourceCommandError::Invalid(
                "Artifact retained response discovery acquisition account",
            ));
        }
    }
    Ok(resources)
}
