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

#[derive(Clone, Copy)]
struct GrowthSourceWrite<'a> {
    subject: &'a str,
    exact_bytes: &'a [u8],
}

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
    let writes = writes
        .iter()
        .map(|write| GrowthSourceWrite {
            subject: &write.subject,
            exact_bytes: &write.exact_bytes,
        })
        .collect::<Vec<_>>();
    owner_contributions_from_source(
        &writes,
        context_files,
        records,
        event_path,
        claim_path,
        claim_owner_schema,
    )
}

fn owner_contributions_from_source(
    writes: &[GrowthSourceWrite<'_>],
    context_files: &[SourceFile],
    records: &[GrowthRecordProjection],
    event_path: &str,
    claim_path: &str,
    claim_owner_schema: &str,
) -> DurableResult<BTreeMap<String, JsonValue>> {
    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt("PG Growth owner write count differs"));
    }
    let mut by_path = BTreeMap::<&str, &GrowthSourceWrite<'_>>::new();
    for write in writes {
        let parsed = RelativePath::parse(write.subject)
            .map_err(|_| DurableError::Corrupt("PG Growth write path"))?;
        if parsed.as_str() != write.subject
            || !write.subject.starts_with("ToS/source-witnesses/")
            || by_path.insert(write.subject, write).is_some()
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
        let exact = cmd::parse(write.exact_bytes).map_err(DurableError::Source)?;
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
        let path = write.subject;
        let mut records = BTreeMap::<String, JsonValue>::new();
        for kind in crate::source_claims::MIXED_INVENTORY_RECORD_KINDS {
            let rows = projected_records
                .get(path)
                .filter(|(record_kind, _)| record_kind.as_str() == *kind)
                .map(|(_, row)| vec![row.clone()])
                .unwrap_or_default();
            records.insert((*kind).to_owned(), JsonValue::Array(rows));
        }
        let records = records
            .into_iter()
            .map(|(kind, rows)| (JsonString::from_utf8(&kind), rows))
            .collect::<Vec<_>>();
        let event_rows = if path == event_path {
            events.clone()
        } else {
            empty_object()
        };
        let form = if path.ends_with(".human-forms.json") {
            let projection = form_projection(write.exact_bytes)?;
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
            (
                "source_line",
                cmd::number(
                    u64::try_from(physical_line)
                        .ok()
                        .and_then(|line| line.checked_add(1))
                        .ok_or(DurableError::Invalid("PG Growth evidence line"))?,
                ),
            ),
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
    let form_ids =
        crate::source_forms::projection_form_ids(&value).map_err(DurableError::Source)?;
    Ok(cmd::object(vec![
        (
            "raw_sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(form_ids)),
    ]))
}

fn is_claim_form_member_name(basename: &str) -> bool {
    basename.starts_with("source-claims.") && basename.ends_with(".human-forms.json")
}

fn claim_owner_projection(
    writes: &[GrowthSourceWrite<'_>],
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
            _ if is_claim_form_member_name(basename) => Some("human-form-set"),
            _ => None,
        };
        let Some(role) = role else {
            continue;
        };
        *roles.entry(role).or_default() += 1;
        direct_members.push(cmd::object(vec![
            ("ref", cmd::string(write.subject)),
            ("role", cmd::string(role)),
            (
                "bytes",
                cmd::number(
                    u64::try_from(write.exact_bytes.len())
                        .map_err(|_| DurableError::Invalid("PG Growth owner member bytes"))?,
                ),
            ),
            (
                "sha256",
                cmd::string(&Digest256::of_bytes(write.exact_bytes).to_prefixed()),
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
    JsonValue::Object(Vec::new())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GrowthClaimHomeKind {
    Relation,
    WorkExpression,
    ExpressionEdition,
}

#[derive(Clone, Copy)]
struct CurrentGrowthClaimRoute {
    operation: &'static str,
    receipt_name: &'static str,
    receipt_schema: &'static str,
    owner_schema: &'static str,
    home_kind: GrowthClaimHomeKind,
}

fn growth_claim_home_kind(path: &str) -> Option<GrowthClaimHomeKind> {
    if RelativePath::parse(path).is_err() || !path.ends_with("/source-claims.jsonl") {
        return None;
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() == 5 && parts[..3] == ["ToS", "source-witnesses", "relations"] {
        return Some(GrowthClaimHomeKind::Relation);
    }
    if parts.len() >= 7
        && parts[..3] == ["ToS", "source-witnesses", "works"]
        && !parts[3..parts.len() - 3].is_empty()
        && parts[parts.len() - 3] == "expressions"
        && parts[parts.len() - 2] != ""
        && parts[parts.len() - 1] == "source-claims.jsonl"
    {
        return Some(GrowthClaimHomeKind::WorkExpression);
    }
    if parts.len() >= 9
        && parts[..3] == ["ToS", "source-witnesses", "works"]
        && !parts[3..parts.len() - 5].is_empty()
        && parts[parts.len() - 5] == "expressions"
        && parts[parts.len() - 4] != ""
        && parts[parts.len() - 3] == "editions"
        && parts[parts.len() - 2] != ""
        && parts[parts.len() - 1] == "source-claims.jsonl"
    {
        return Some(GrowthClaimHomeKind::ExpressionEdition);
    }
    None
}

/// Exact maintained path shapes for the five PG Growth Claim owners. Relation
/// homes are shared by CollectionMembership, ObjectLink, and
/// ExpressionResponsibility; Work/Edition homes retain variable Work depth.
pub(crate) fn is_growth_claim_stream_path(path: &str) -> bool {
    growth_claim_home_kind(path).is_some()
}

fn current_growth_claim_route(
    request_schema: &str,
    operation: &str,
    home_kind: GrowthClaimHomeKind,
) -> Option<CurrentGrowthClaimRoute> {
    let route = match (request_schema, operation) {
        ("tos_local_collection_membership_command_v1", "collection.work.attach") => {
            CurrentGrowthClaimRoute {
                operation: "collection.work.attach",
                receipt_name: "membership-attachment-receipt.json",
                receipt_schema: "tos_collection_membership_receipt_v1",
                owner_schema: "tos_managed_collection_membership_claim_owner_inventory_v1",
                home_kind: GrowthClaimHomeKind::Relation,
            }
        }
        ("tos_local_object_link_command_v1", "object.link.create") => CurrentGrowthClaimRoute {
            operation: "object.link.create",
            receipt_name: "object-link-creation-receipt.json",
            receipt_schema: "tos_object_link_receipt_v1",
            owner_schema: "tos_managed_object_link_claim_owner_inventory_v1",
            home_kind: GrowthClaimHomeKind::Relation,
        },
        ("tos_local_expression_responsibility_command_v1", "expression.responsibility.attach") => {
            CurrentGrowthClaimRoute {
                operation: "expression.responsibility.attach",
                receipt_name: "responsibility-attachment-receipt.json",
                receipt_schema: "tos_expression_responsibility_receipt_v1",
                owner_schema: "tos_managed_expression_responsibility_claim_owner_inventory_v1",
                home_kind: GrowthClaimHomeKind::Relation,
            }
        }
        ("tos_local_work_expression_command_v1", "work.expression.create") => {
            CurrentGrowthClaimRoute {
                operation: "work.expression.create",
                receipt_name: "work-expression-receipt.json",
                receipt_schema: "tos_work_expression_receipt_v1",
                owner_schema: "tos_managed_work_expression_claim_owner_inventory_v1",
                home_kind: GrowthClaimHomeKind::WorkExpression,
            }
        }
        ("tos_local_expression_edition_command_v1", "expression.edition.create") => {
            CurrentGrowthClaimRoute {
                operation: "expression.edition.create",
                receipt_name: "expression-edition-receipt.json",
                receipt_schema: "tos_expression_edition_receipt_v1",
                owner_schema: "tos_managed_expression_edition_claim_owner_inventory_v1",
                home_kind: GrowthClaimHomeKind::ExpressionEdition,
            }
        }
        _ => return None,
    };
    (route.home_kind == home_kind).then_some(route)
}

pub(crate) fn current_growth_claim_receipt_name(
    claim_path: &str,
    request_raw: &[u8],
) -> DurableResult<Option<&'static str>> {
    let Some(home_kind) = growth_claim_home_kind(claim_path) else {
        return Ok(None);
    };
    let request = cmd::parse(request_raw).map_err(DurableError::Source)?;
    let request_schema = cmd::text(&request, "schema_version").map_err(DurableError::Source)?;
    let operation = cmd::text(&request, "operation").map_err(DurableError::Source)?;
    let Some(route) = current_growth_claim_route(request_schema, operation, home_kind) else {
        if home_kind == GrowthClaimHomeKind::Relation {
            return Ok(None);
        }
        return Err(DurableError::Corrupt(
            "PG Growth current request route differs",
        ));
    };
    Ok(Some(route.receipt_name))
}

/// Rebuild a current Growth Claim contribution from its exact retained owner
/// package. This is a source-current projection adapter, not a commit receipt;
/// the owner schema and operation must agree with the direct request and typed
/// receipt at the exact package home.
pub(crate) fn current_owner_contributions(
    context: &crate::source_command::CommandContext,
    claim_path: &str,
) -> DurableResult<Option<BTreeMap<String, JsonValue>>> {
    let Some(home_kind) = growth_claim_home_kind(claim_path) else {
        return Ok(None);
    };
    let home = claim_path
        .strip_suffix("/source-claims.jsonl")
        .ok_or(DurableError::Corrupt("PG Growth current Claim path"))?;
    let request_path = format!("{home}/source-create-request.json");
    let Some(request_file) = context
        .files
        .iter()
        .find(|file| file.path.as_str() == request_path)
    else {
        if home_kind == GrowthClaimHomeKind::Relation {
            return Ok(None);
        }
        return Err(DurableError::Corrupt("PG Growth current request absent"));
    };
    let request = cmd::parse(&request_file.raw).map_err(DurableError::Source)?;
    let request_schema = cmd::text(&request, "schema_version").map_err(DurableError::Source)?;
    let operation = cmd::text(&request, "operation").map_err(DurableError::Source)?;
    let Some(route) = current_growth_claim_route(request_schema, operation, home_kind) else {
        // A relation home may be the separate Global Claims owner. Nested
        // Work/Edition shapes are Growth-only and fail closed on a mismatch.
        if home_kind == GrowthClaimHomeKind::Relation {
            return Ok(None);
        }
        return Err(DurableError::Corrupt(
            "PG Growth current request route differs",
        ));
    };
    let receipt_path = format!("{home}/{}", route.receipt_name);
    let receipt_file = context
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .ok_or(DurableError::Corrupt(
            "PG Growth current typed receipt absent",
        ))?;
    let receipt = cmd::parse(&receipt_file.raw).map_err(DurableError::Source)?;
    validate_current_growth_receipt_binding(&request, &receipt, route)?;
    let writes = context
        .files
        .iter()
        .filter(|file| {
            file.path
                .as_str()
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                == Some(home)
        })
        .map(|file| GrowthSourceWrite {
            subject: file.path.as_str(),
            exact_bytes: &file.raw,
        })
        .collect::<Vec<_>>();
    let event_path = format!("{home}/source-create-provenance.jsonl");
    owner_contributions_from_source(
        &writes,
        &context.files,
        &[],
        &event_path,
        claim_path,
        route.owner_schema,
    )
    .map(Some)
}

fn validate_current_growth_receipt_binding(
    request: &JsonValue,
    receipt: &JsonValue,
    route: CurrentGrowthClaimRoute,
) -> DurableResult<()> {
    let request_digest = cmd::record_digest(request)
        .map_err(DurableError::Source)?
        .to_prefixed();
    if cmd::text(receipt, "schema_version").map_err(DurableError::Source)? != route.receipt_schema
        || cmd::text(receipt, "operation").map_err(DurableError::Source)? != route.operation
        || cmd::text(receipt, "command_id").map_err(DurableError::Source)?
            != cmd::text(request, "command_id").map_err(DurableError::Source)?
        || cmd::text(receipt, "request_digest").map_err(DurableError::Source)? != request_digest
    {
        return Err(DurableError::Corrupt(
            "PG Growth current receipt request binding differs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod current_claim_route_tests {
    use super::cmd;
    use super::{
        GrowthClaimHomeKind, current_growth_claim_receipt_name, current_growth_claim_route,
        growth_claim_home_kind, validate_current_growth_receipt_binding,
    };

    #[test]
    fn maintained_growth_claim_routes_bind_typed_owner_and_variable_work_depth() {
        let cases = [
            (
                "ToS/source-witnesses/relations/membership/source-claims.jsonl",
                "tos_local_collection_membership_command_v1",
                "collection.work.attach",
                GrowthClaimHomeKind::Relation,
                "tos_managed_collection_membership_claim_owner_inventory_v1",
                "membership-attachment-receipt.json",
            ),
            (
                "ToS/source-witnesses/relations/object-link/source-claims.jsonl",
                "tos_local_object_link_command_v1",
                "object.link.create",
                GrowthClaimHomeKind::Relation,
                "tos_managed_object_link_claim_owner_inventory_v1",
                "object-link-creation-receipt.json",
            ),
            (
                "ToS/source-witnesses/relations/responsibility/source-claims.jsonl",
                "tos_local_expression_responsibility_command_v1",
                "expression.responsibility.attach",
                GrowthClaimHomeKind::Relation,
                "tos_managed_expression_responsibility_claim_owner_inventory_v1",
                "responsibility-attachment-receipt.json",
            ),
            (
                "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.jsonl",
                "tos_local_work_expression_command_v1",
                "work.expression.create",
                GrowthClaimHomeKind::WorkExpression,
                "tos_managed_work_expression_claim_owner_inventory_v1",
                "work-expression-receipt.json",
            ),
            (
                "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/source-claims.jsonl",
                "tos_local_expression_edition_command_v1",
                "expression.edition.create",
                GrowthClaimHomeKind::ExpressionEdition,
                "tos_managed_expression_edition_claim_owner_inventory_v1",
                "expression-edition-receipt.json",
            ),
        ];
        for (path, request_schema, operation, kind, owner_schema, receipt_name) in cases {
            assert_eq!(growth_claim_home_kind(path), Some(kind));
            let route = current_growth_claim_route(request_schema, operation, kind).unwrap();
            assert_eq!(route.owner_schema, owner_schema);
            assert_eq!(route.receipt_name, receipt_name);
            assert!(route.receipt_schema.ends_with("_receipt_v1"));
            let request =
                format!(r#"{{"schema_version":"{request_schema}","operation":"{operation}"}}"#);
            assert_eq!(
                current_growth_claim_receipt_name(path, request.as_bytes()).unwrap(),
                Some(receipt_name)
            );
            assert!(current_growth_claim_route(request_schema, "claims.create", kind).is_none());
        }
        assert!(
            growth_claim_home_kind(
                "ToS/source-witnesses/works/tree-of-sophia/expressions/en/source-claims.jsonl"
            )
            .is_some()
        );
        assert!(growth_claim_home_kind(
            "ToS/source-witnesses/works/tree-of-sophia/expressions/en/editions/e/items/i/source-claims.jsonl"
        )
        .is_none());
        assert!(
            growth_claim_home_kind(
                "ToS/source-witnesses/items/tree-of-sophia/i/source-claims.jsonl"
            )
            .is_none()
        );
    }

    #[test]
    fn growth_claim_owner_direct_members_exclude_entity_form_sets() {
        let home = "ToS/source-witnesses/works/tree-of-sophia/expression/en";
        let stream_path = format!("{home}/source-claims.jsonl");
        let claim_form_path = format!("{home}/source-claims.{}.human-forms.json", "0".repeat(64));
        let entity_form_path = format!("{home}/expression.human-forms.json");
        let request_path = format!("{home}/source-create-request.json");
        let environment_path = format!("{home}/source-create-environment.json");
        let provenance_path = format!("{home}/source-create-provenance.jsonl");
        let receipt_path = format!("{home}/work-expression-receipt.json");
        let stream = br#"{"claim_id":"tos.claim.example"}"#;
        let empty = b"{}";
        let writes = [
            super::GrowthSourceWrite {
                subject: &stream_path,
                exact_bytes: stream,
            },
            super::GrowthSourceWrite {
                subject: &request_path,
                exact_bytes: empty,
            },
            super::GrowthSourceWrite {
                subject: &environment_path,
                exact_bytes: empty,
            },
            super::GrowthSourceWrite {
                subject: &provenance_path,
                exact_bytes: empty,
            },
            super::GrowthSourceWrite {
                subject: &receipt_path,
                exact_bytes: empty,
            },
            super::GrowthSourceWrite {
                subject: &claim_form_path,
                exact_bytes: empty,
            },
            super::GrowthSourceWrite {
                subject: &entity_form_path,
                exact_bytes: empty,
            },
        ];
        let owner = super::claim_owner_projection(
            &writes,
            stream,
            &stream_path,
            "tos_managed_work_expression_claim_owner_inventory_v1",
        )
        .unwrap();
        let members = cmd::field(&owner, "direct_members")
            .unwrap()
            .as_array()
            .unwrap();
        let refs = members
            .iter()
            .map(|row| cmd::text(row, "ref").unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(members.len(), 6);
        assert!(refs.contains(claim_form_path.as_str()));
        assert!(!refs.contains(entity_form_path.as_str()));
    }

    #[test]
    fn maintained_growth_receipts_bind_canonical_request_digest_and_command_id() {
        let cases = [
            (
                "tos_local_collection_membership_command_v1",
                "collection.work.attach",
                "tos_collection_membership_receipt_v1",
                GrowthClaimHomeKind::Relation,
            ),
            (
                "tos_local_object_link_command_v1",
                "object.link.create",
                "tos_object_link_receipt_v1",
                GrowthClaimHomeKind::Relation,
            ),
            (
                "tos_local_expression_responsibility_command_v1",
                "expression.responsibility.attach",
                "tos_expression_responsibility_receipt_v1",
                GrowthClaimHomeKind::Relation,
            ),
            (
                "tos_local_work_expression_command_v1",
                "work.expression.create",
                "tos_work_expression_receipt_v1",
                GrowthClaimHomeKind::WorkExpression,
            ),
            (
                "tos_local_expression_edition_command_v1",
                "expression.edition.create",
                "tos_expression_edition_receipt_v1",
                GrowthClaimHomeKind::ExpressionEdition,
            ),
        ];
        for (request_schema, operation, receipt_schema, home_kind) in cases {
            let route = current_growth_claim_route(request_schema, operation, home_kind).unwrap();
            assert_eq!(route.receipt_schema, receipt_schema);
            let request = cmd::object(vec![
                ("schema_version", cmd::string(request_schema)),
                ("operation", cmd::string(operation)),
                ("command_id", cmd::string("fixture-command")),
            ]);
            let request_digest = cmd::record_digest(&request).unwrap().to_prefixed();
            let receipt = |command_id: &str, digest: &str| {
                cmd::object(vec![
                    ("schema_version", cmd::string(receipt_schema)),
                    ("operation", cmd::string(operation)),
                    ("command_id", cmd::string(command_id)),
                    ("request_digest", cmd::string(digest)),
                ])
            };
            assert!(
                validate_current_growth_receipt_binding(
                    &request,
                    &receipt("fixture-command", &request_digest),
                    route,
                )
                .is_ok()
            );
            assert!(
                validate_current_growth_receipt_binding(
                    &request,
                    &receipt("other-command", &request_digest),
                    route,
                )
                .is_err()
            );
            assert!(
                validate_current_growth_receipt_binding(
                    &request,
                    &receipt("fixture-command", "sha256:wrong"),
                    route,
                )
                .is_err()
            );
        }
    }
}
