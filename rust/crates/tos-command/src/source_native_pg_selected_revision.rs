//! Bounded PG selection for maintained non-Work selected record revisions.
//!
//! This module is deliberately a thin adapter over the source revision owner:
//! it selects the owner-declared record package, forms, publication evidence,
//! history, archives, and schema closure from one PG read epoch, then asks the
//! existing semantic preparer to build the proposal.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_revisions::{
    self, PgRecordRevisionIdentity, ReadonlyRecordFiles, RevisionFamily,
};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{RelativePath, SourceRevision};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};

struct NativeRoute {
    subtree: &'static str,
    basename: &'static str,
    id_field: &'static str,
    schema_version: &'static str,
    schema_ref: &'static str,
}

fn native_route(config: &tos_foundation::JsonValue) -> SourceCommandResult<NativeRoute> {
    let record_type = cmd::text(config, "record_type")?;
    let schema_version = cmd::text(config, "record_schema_version")?;
    let route = match (record_type, schema_version) {
        ("artifact", "tos_artifact_source_witness_v1") => NativeRoute {
            subtree: "artifacts",
            basename: "artifact-witness.json",
            id_field: "artifact_id",
            schema_version: "tos_artifact_source_witness_v1",
            schema_ref: "ToS/contracts/artifact-source-witness.schema.json",
        },
        ("artifact", "tos_artifact_source_witness_v2") => NativeRoute {
            subtree: "artifacts",
            basename: "artifact-witness.json",
            id_field: "artifact_id",
            schema_version: "tos_artifact_source_witness_v2",
            schema_ref: "ToS/contracts/artifact-source-witness-v2.schema.json",
        },
        ("composite", "tos_scholarly_composite_witness_v1") => NativeRoute {
            subtree: "scholarly-composites",
            basename: "composite-witness.json",
            id_field: "composite_id",
            schema_version: "tos_scholarly_composite_witness_v1",
            schema_ref: "ToS/contracts/scholarly-composite-witness.schema.json",
        },
        ("link", "tos_source_link_v1") => NativeRoute {
            subtree: "links",
            basename: "link.json",
            id_field: "record_id",
            schema_version: "tos_source_link_v1",
            schema_ref: "ToS/contracts/source-link.schema.json",
        },
        _ => return Err(SourceCommandError::Denied("exact native record schema")),
    };
    let source_path = cmd::text(config, "source_path")?;
    let expected_prefix = format!("ToS/source-witnesses/{}/", route.subtree);
    let path_parts = source_path.split('/').collect::<Vec<_>>();
    if !source_path.starts_with(&expected_prefix)
        || source_path.rsplit('/').next() != Some(route.basename)
        || path_parts.len() < 5
        || path_parts.iter().any(|part| part.starts_with('.'))
    {
        return Err(SourceCommandError::Denied(
            "exact native owner path or schema",
        ));
    }
    let record_id = cmd::text(config, "record_id")?;
    let prefix = match record_type {
        "artifact" => "tos.artifact.",
        "composite" => "tos.composite.",
        "link" => "tos.link.",
        _ => return Err(SourceCommandError::Denied("native revision record type")),
    };
    if !source_revisions::valid_id(record_id, prefix, false) {
        return Err(SourceCommandError::Denied("native source identity"));
    }
    Ok(route)
}

fn require_selected_file(context: &cmd::CommandContext, path: &str) -> SourceCommandResult<()> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("selected revision source path"))?;
    if context.files.iter().any(|file| file.path == path) {
        Ok(())
    } else {
        Err(SourceCommandError::Conflict(
            "selected revision schema or source member absent",
        ))
    }
}

/// Select and semantically prepare one non-Work selected revision from the
/// caller's held PG read epoch. `transport` is the epoch-bound source reader;
/// the resulting prepared value retains the same opaque PG identity for the
/// shared durable writer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    invocation: &Value,
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    source_path: &str,
    base_revision: SourceRevision,
    transport: &mut impl ReadonlyRecordFiles,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    identity: PgRecordRevisionIdentity,
    revision_observation: Option<crate::source_pg_work_revision::Observation>,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<super::revisions::PreparedPgRecordRevision> {
    if components.capture() != software.selection() {
        return Err(SourceCommandError::Conflict(
            "selected software capture components differ",
        ));
    }
    let config = cmd::parse(configuration_raw)?;
    let family = RevisionFamily::parse(cmd::text(&config, "schema_version")?)?;
    if !matches!(
        family,
        RevisionFamily::CorpusSelectedV2
            | RevisionFamily::CorpusSelectedV3
            | RevisionFamily::NativeSelected
    ) {
        return Err(SourceCommandError::Unsupported(
            "PG selected record revision family",
        ));
    }
    if cmd::text(&config, "source_path")? != source_path {
        return Err(SourceCommandError::Conflict(
            "selected revision source path differs from protected owner",
        ));
    }
    let source = RelativePath::parse(source_path)
        .map_err(|_| SourceCommandError::Invalid("selected revision source path"))?;
    if !source.as_str().starts_with("ToS/source-witnesses/") {
        return Err(SourceCommandError::Denied(
            "selected revision outside public witness tree",
        ));
    }
    let request = cmd::parse(request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(&request, "operation")? != "record.revise"
    {
        return Err(SourceCommandError::Unsupported(
            "PG selected revision requires record.revise",
        ));
    }

    let native = if family == RevisionFamily::NativeSelected {
        Some(native_route(&config)?)
    } else {
        if cmd::text(&config, "record_type")? == "work" {
            return Err(SourceCommandError::Unsupported(
                "PG Work revision uses its dedicated owner route",
            ));
        }
        None
    };
    let context = super::revisions::context_selected_record_from_transport_with_limits(
        configuration_raw,
        request_raw,
        recorded_at,
        source_path,
        base_revision,
        transport,
        software,
        components,
        max_members,
        max_total_bytes,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
    context.check()?;

    let record_path = RelativePath::parse(source_path)
        .map_err(|_| SourceCommandError::Invalid("selected revision source path"))?;
    let record_file = context
        .files
        .iter()
        .find(|file| file.path == record_path)
        .ok_or(SourceCommandError::Conflict(
            "selected revision record absent",
        ))?;
    let record = cmd::parse(&record_file.raw)?;
    if let Some(route) = &native {
        if cmd::text(&record, "schema_version")? != route.schema_version
            || cmd::text(&record, route.id_field)? != cmd::text(&config, "record_id")?
        {
            return Err(SourceCommandError::Conflict(
                "selected native record differs from protected owner identity",
            ));
        }
        for schema in [
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            route.schema_ref,
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ] {
            require_selected_file(&context, schema)?;
        }
    } else {
        if cmd::text(&record, "record_id")? != cmd::text(&config, "record_id")?
            || cmd::text(&record, "record_type")? != cmd::text(&config, "record_type")?
        {
            return Err(SourceCommandError::Conflict(
                "selected corpus record differs from protected owner identity",
            ));
        }
        for schema in [
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ] {
            require_selected_file(&context, schema)?;
        }
    }

    let prepared = super::revisions::prepare_pg_record_revision_with_work(
        invocation,
        context,
        identity,
        revision_observation,
        deadline,
        cancelled,
    )?;
    let expected_family_tag = family.pg_family_tag(&config)?;
    if prepared.identity() != identity
        || prepared.family_tag() != expected_family_tag
        || prepared.legacy_family_encoding()
    {
        return Err(SourceCommandError::Conflict(
            "selected revision durable family binding differs",
        ));
    }
    Ok(prepared)
}
