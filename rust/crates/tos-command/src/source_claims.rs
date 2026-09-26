//! Source-owned Claim command mechanics. Selected bytes are proposals, never
//! admission. Schemas are executed by the actual bounded source-cut worker.
//! Unsupported grounding adapters fail closed without flattening their values.
use crate::source_command::*;
use crate::source_forms::{
    apply_form_changes, materialize_source_forms, metadata_subject, prepare_form_change,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

pub const CLAIM_STREAM: &str = "source-claims.jsonl";
pub const CLAIM_HISTORY: &str = "claim-revision-history.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const FIELDS: &[&str] = &[
    "qualifiers",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "supporting_quotes",
    "epistemic_status",
    "confidence",
];
const COMPOUND: &[&str] = &[
    "has_expression",
    "embodied_by",
    "exemplified_by",
    "translated_by",
    "contains_work",
    "described_by",
    "metadata_at",
    "downloadable_at",
    "rights_statement_at",
];

fn path(s: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(s).map_err(|_| SourceCommandError::Invalid("source path"))
}
fn selected<'a>(ctx: &'a CommandContext, s: &str) -> SourceCommandResult<&'a [u8]> {
    ctx.file(&path(s)?)?.ok_or(SourceCommandError::Unsupported(
        "exact source member not selected",
    ))
}
fn json_file(ctx: &CommandContext, s: &str) -> SourceCommandResult<JsonValue> {
    parse(selected(ctx, s)?)
}
fn allowed(config: &JsonValue, key: &str, value: &JsonValue) -> SourceCommandResult<bool> {
    for v in array(config, key)? {
        if same(v, value)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn grant(config: &JsonValue, key: &str, value: &JsonValue) -> SourceCommandResult<()> {
    if allowed(config, key, value)? {
        Ok(())
    } else {
        Err(SourceCommandError::Denied("current Claim scope"))
    }
}
fn rows(raw: &[u8]) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid("Claim stream budget"));
    }
    let mut records = BTreeMap::new();
    for line in raw.split(|b| *b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let record = parse(line)?;
        let id = text(&record, "claim_id")?.to_owned();
        if records.insert(id, record).is_some() {
            return Err(SourceCommandError::Conflict("duplicate Claim identity"));
        }
    }
    Ok(records)
}
/// Replace exactly one row, preserving every sibling byte, line ending and order.
pub fn replace_claim_row(raw: &[u8], revised: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    rows(raw)?;
    let id = text(revised, "claim_id")?;
    let mut found = false;
    let mut output = Vec::new();
    for line in raw.split_inclusive(|b| *b == b'\n') {
        if !line.iter().all(u8::is_ascii_whitespace) && text(&parse(line)?, "claim_id")? == id {
            if found {
                return Err(SourceCommandError::Conflict("repeated selected Claim"));
            }
            found = true;
            output.extend(canonical(revised)?);
            if line.ends_with(b"\r\n") {
                output.extend(b"\r\n")
            } else if line.ends_with(b"\n") {
                output.push(b'\n')
            }
        } else {
            output.extend(line)
        }
    }
    if !found {
        return Err(SourceCommandError::Conflict("selected Claim absent"));
    }
    Ok(output)
}
/// One predecessor-bound correction; richer qualifier objects remain intact.
pub fn advance_claim(
    record: &JsonValue,
    fields: &JsonValue,
    layer: Option<&JsonValue>,
) -> SourceCommandResult<JsonValue> {
    let members = fields
        .as_object()
        .ok_or(SourceCommandError::Invalid("Claim field patch"))?;
    if members.is_empty() {
        return Err(SourceCommandError::Invalid("empty Claim correction"));
    }
    for (k, _) in members {
        let k = k
            .as_str()
            .ok_or(SourceCommandError::Invalid("field name"))?;
        if !(if layer.is_some() {
            k == "assertion_layer"
        } else {
            FIELDS.contains(&k) || k == "object"
        }) {
            return Err(SourceCommandError::Denied("immutable Claim field"));
        }
    }
    if let Some(transition) = layer {
        exact_keys(transition, &["from", "to"])?;
        if text(transition, "from")? == text(transition, "to")?
            || text(transition, "from")? != text(record, "assertion_layer")?
            || text(transition, "to")? != text(fields, "assertion_layer")?
        {
            return Err(SourceCommandError::Denied(
                "exact predecessor layer transition",
            ));
        }
    }
    let mut revised = record.clone();
    for (k, v) in members {
        let key = k
            .as_str()
            .ok_or(SourceCommandError::Invalid("field name"))?;
        let value = if key == "qualifiers" {
            let mut q = record
                .object_get("qualifiers")
                .cloned()
                .unwrap_or_else(|| object(vec![]));
            for (key, value) in v
                .as_object()
                .ok_or(SourceCommandError::Invalid("qualifier patch object"))?
            {
                set(
                    &mut q,
                    key.as_str()
                        .ok_or(SourceCommandError::Invalid("qualifier name"))?,
                    value.clone(),
                )?
            }
            q
        } else {
            v.clone()
        };
        if key == "object" && (field(record, key)?.as_object().is_none() || v.as_object().is_none())
        {
            return Err(SourceCommandError::Denied(
                "Claim endpoint kind is immutable",
            ));
        }
        set(&mut revised, key, value)?;
    }
    if [
        "identity_transition_proposal",
        "subject_identity_transition_proposal",
    ]
    .contains(&text(record, "predicate")?)
    {
        for key in [
            "kind",
            "operation",
            "members",
            "predecessors",
            "successors",
            "mapping",
            "supersedes_proposal",
        ] {
            if !same(
                field(field(record, "object")?, key)?,
                field(field(&revised, "object")?, key)?,
            )? {
                return Err(SourceCommandError::Denied(
                    "identity proposal topology requires new assertion identity",
                ));
            }
        }
    }
    if same(record, &revised)? {
        return Err(SourceCommandError::Invalid(
            "Claim correction has no change",
        ));
    }
    let version = integer(record, "claim_version")?
        .checked_add(1)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or(SourceCommandError::Invalid("Claim version overflow"))?;
    set(&mut revised, "claim_version", number(version))?;
    Ok(revised)
}
fn family(schema: &str) -> SourceCommandResult<(String, bool, u8)> {
    for (prefix, create) in [
        ("tos_local_claim_create_owner_v", true),
        ("tos_local_claim_revision_owner_v", false),
    ] {
        if let Some(suffix) = schema.strip_prefix(prefix) {
            let version = suffix
                .parse::<u8>()
                .map_err(|_| SourceCommandError::Unsupported("Claim configuration version"))?;
            if (1..=4).contains(&version) {
                return Ok((
                    format!(
                        "public-claim-{}-v{version}",
                        if create { "create" } else { "revision" }
                    ),
                    create,
                    version,
                ));
            }
        }
    }
    for (schema_prefix, tag) in [
        (
            "tos_local_document_catalogue_date_",
            "document-catalogue-v1",
        ),
        ("tos_local_identity_proposal_", "identity"),
    ] {
        if let Some(rest) = schema.strip_prefix(schema_prefix) {
            for (suffix, create) in [("create_owner_v", true), ("revision_owner_v", false)] {
                if let Some(version) = rest.strip_prefix(suffix) {
                    if version == "1" || tag == "identity" && version == "2" {
                        return Ok((
                            format!(
                                "public-claim-{}-{}",
                                if create { "create" } else { "revision" },
                                if tag == "identity" {
                                    format!("identity-v{version}")
                                } else {
                                    tag.into()
                                }
                            ),
                            create,
                            if tag == "identity" { 6 } else { 5 },
                        ));
                    }
                }
            }
        }
    }
    if schema == "tos_local_claim_layer_revision_owner_v1" {
        return Ok(("public-claim-revision-layer-v1".into(), false, 7));
    }
    Err(SourceCommandError::Unsupported("Claim owner configuration"))
}
fn bounded_list(config: &JsonValue, key: &str, max: usize) -> SourceCommandResult<()> {
    let entries = array(config, key)?;
    if entries.len() > max {
        return Err(SourceCommandError::Invalid("Claim scope capacity"));
    }
    let mut seen = BTreeSet::new();
    for value in entries {
        if !seen.insert(canonical(value)?) {
            return Err(SourceCommandError::Invalid("repeated Claim scope value"));
        }
    }
    Ok(())
}
fn config(
    ctx: &CommandContext,
) -> SourceCommandResult<(JsonValue, RelativePath, String, bool, u8)> {
    ctx.check()?;
    if ctx.files.iter().any(|file| {
        file.path
            .as_str()
            .starts_with("ToS/source-witnesses/owner-local/")
    }) {
        return Err(SourceCommandError::Unsupported(
            "reserved owner-local namespace cannot enter public Claim source cut",
        ));
    }
    let c = parse(&ctx.configuration_raw)?;
    let (handler, create, version) = family(text(&c, "schema_version")?)?;
    if integer(&c, "uid")? != ctx.effective_uid
        || text(&c, "principal_id")?.trim().is_empty()
        || text(&c, "authority_ref")?.trim().is_empty()
    {
        return Err(SourceCommandError::Denied("Claim owner account"));
    }
    // Shared source command instant parser preserves actual UTC offsets.
    validate_expiry(text(&c, "expires_at")?, &ctx.recorded_at)?;
    let p = path(text(&c, "source_path")?)?;
    let parts: Vec<_> = p.as_str().split('/').collect();
    if parts.len() < 4
        || parts[..2] != ["ToS", "source-witnesses"]
        || parts.last() != Some(&CLAIM_STREAM)
        || parts
            .iter()
            .any(|p| ["catalog", "payload", "local-content"].contains(p))
        || create && (parts.len() != 5 || parts[2] != "relations")
    {
        return Err(SourceCommandError::Denied("Claim source stream path"));
    }
    let mut keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "source_root",
        "source_path",
        "authority_ref",
        "expires_at",
        "allowed_operations",
        "allowed_evidence_refs",
    ];
    if create {
        keys.extend([
            "maker_type",
            "provenance_event_id",
            "allowed_claim_ids",
            "allowed_subject_refs",
            "allowed_object_refs",
            "allowed_predicates",
        ]);
        bounded_list(&c, "allowed_claim_ids", 32)?;
        bounded_list(&c, "allowed_subject_refs", 128)?;
        bounded_list(&c, "allowed_predicates", 32)?;
    } else {
        keys.extend(["claim_id", "allowed_fields", "allowed_form_ids"]);
        bounded_list(&c, "allowed_fields", 8)?;
        bounded_list(&c, "allowed_form_ids", 32)?;
        if c.object_get("allowed_form_field_ids").is_some() {
            keys.push("allowed_form_field_ids")
        }
    }
    if (2..=6).contains(&version) {
        keys.push("allowed_object_values");
        bounded_list(&c, "allowed_object_values", 32)?;
        if !create {
            keys.push("allowed_object_refs")
        }
    }
    if version == 6 {
        keys.push("allowed_related_claim_refs");
        bounded_list(&c, "allowed_related_claim_refs", 33)?
    }
    if version == 7 {
        keys.push("allowed_layer_transitions");
        bounded_list(&c, "allowed_layer_transitions", 32)?
    }
    if !create {
        for value in array(&c, "allowed_fields")? {
            let field = value
                .as_str()
                .ok_or(SourceCommandError::Invalid("Claim field scope string"))?;
            if version == 7 {
                if field != "assertion_layer" {
                    return Err(SourceCommandError::Denied("layer-only owner field scope"));
                }
            } else if !FIELDS.contains(&field) && !(field == "object" && (2..=6).contains(&version))
            {
                return Err(SourceCommandError::Denied("immutable Claim field scope"));
            }
        }
    }
    exact_keys(&c, &keys)?;
    bounded_list(&c, "allowed_operations", 1)?;
    bounded_list(&c, "allowed_evidence_refs", 128)?;
    if create || (2..=6).contains(&version) {
        bounded_list(&c, "allowed_object_refs", 128)?
    }
    Ok((c, p, handler, create, version))
}
fn grammar(request: &JsonValue, create: bool, layer: bool) -> SourceCommandResult<()> {
    if text(request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid("Claim request schema"));
    }
    let op = text(request, "operation")?;
    let mut keys = vec!["schema_version", "operation"];
    match (create, op) {
        (_, "describe") => {}
        (true, "prepare-create" | "claims.create") => keys.push("claims"),
        (false, "prepare-revise" | "claim.revise") => {
            keys.extend(["fields", "forms", "reason"]);
            if layer {
                keys.push("layer_transition")
            }
        }
        (false, "inspect-version") => keys.push("source"),
        _ => return Err(SourceCommandError::Unsupported("Claim operation")),
    }
    if op == "claims.create" || op == "claim.revise" {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_revision",
            "expected_dependencies",
            "expected_inputs",
        ]);
        if !create {
            keys.push("expected_source")
        }
        let id = text(request, "command_id")?;
        if id.is_empty() || id.chars().count() > 256 {
            return Err(SourceCommandError::Invalid("Claim command identity"));
        }
    }
    exact_keys(request, &keys)
}
fn profile(ctx: &CommandContext, predicate: &str) -> SourceCommandResult<(JsonValue, JsonValue)> {
    let registry = json_file(ctx, RELATIONS)?;
    let mut found = None;
    for relation in array(&registry, "relations")? {
        for mapping in array(relation, "source_mappings")? {
            if mapping
                .object_get("source_graph")
                .and_then(JsonValue::as_str)
                == Some("source-claims")
                && mapping
                    .object_get("source_predicate_id")
                    .and_then(JsonValue::as_str)
                    == Some(predicate)
            {
                if found.is_some() {
                    return Err(SourceCommandError::Conflict("ambiguous Claim profile"));
                }
                found = Some((
                    relation.clone(),
                    field(relation, "source_claim_profile")?.clone(),
                ));
            }
        }
    }
    found.ok_or(SourceCommandError::Unsupported(
        "predicate has no selected source Claim profile",
    ))
}
fn schema_check(
    executor: &mut CutWorkerSchemaExecutor,
    path: &str,
    value: &JsonValue,
    contract: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    match executor.check(path, &canonical(value)?, contract, deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid("source Claim schema refused")),
        Err(_) => Err(SourceCommandError::Unsupported(
            "source Claim schema execution incomplete",
        )),
    }
}
fn claim_scope(
    config: &JsonValue,
    claim: &JsonValue,
    create: bool,
    version: u8,
) -> SourceCommandResult<()> {
    let predicate = text(claim, "predicate")?;
    if COMPOUND.contains(&predicate) {
        return Err(SourceCommandError::Unsupported(
            "Claim requires its compound source owner operation",
        ));
    }
    if create {
        for (key, field_name) in [
            ("allowed_claim_ids", "claim_id"),
            ("allowed_subject_refs", "subject_ref"),
            ("allowed_predicates", "predicate"),
        ] {
            grant(config, key, field(claim, field_name)?)?
        }
        let maker = field(claim, "maker")?;
        if text(maker, "agent_ref")? != text(config, "principal_id")?
            || text(maker, "maker_type")? != text(config, "maker_type")?
            || text(claim, "provenance_event_ref")? != text(config, "provenance_event_id")?
        {
            return Err(SourceCommandError::Denied(
                "Claim maker and provenance scope",
            ));
        }
    }
    let value = field(claim, "object")?;
    if value.as_str().is_some() {
        if create {
            grant(config, "allowed_object_refs", value)?
        }
    } else if create || (4..=6).contains(&version) {
        grant(config, "allowed_object_values", value)?
    } else if create {
        return Err(SourceCommandError::Denied("typed value scope"));
    }
    if create {
        for key in ["evidence_refs", "counterevidence_refs"] {
            if let Some(v) = claim.object_get(key) {
                for evidence in v
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("Claim evidence array"))?
                {
                    grant(config, "allowed_evidence_refs", evidence)?
                }
            }
        }
    }
    Ok(())
}
fn package(
    ctx: &CommandContext,
    p: &RelativePath,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent = p
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Claim parent"))?
        .0;
    let mut files = BTreeMap::new();
    for f in &ctx.files {
        if let Some(name) = f.path.as_str().strip_prefix(&format!("{parent}/")) {
            if !name.contains('/') {
                files.insert(name.into(), f.raw.clone());
            }
        }
    }
    if files.len() > 64 || files.values().map(Vec::len).sum::<usize>() > 8_388_608 {
        return Err(SourceCommandError::Invalid("Claim package budget"));
    }
    Ok(files)
}
fn refs(files: &BTreeMap<String, Vec<u8>>) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    tos_foundation::JsonString::from_utf8(name),
                    object(vec![
                        ("sha256", string(&Digest256::of_bytes(raw).to_prefixed())),
                        ("bytes", number(raw.len() as u64)),
                    ]),
                )
            })
            .collect(),
    )
}
fn revision(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    Ok(string(&record_digest(&refs(files))?.to_prefixed()))
}
fn form_name(id: &str) -> String {
    format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(id.as_bytes())
            .to_prefixed()
            .trim_start_matches("sha256:")
    )
}

/// Execute against the exact current selected cut and protected configuration.
/// The successful output has no production writer or authority lease.
pub fn run_claim_command(
    ctx: &CommandContext,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    let (config, p, handler, create, version) = config(ctx)?;
    if executor.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "Claim schema worker cut differs from command base",
        ));
    }

    let request = parse(&ctx.request_raw)?;
    grammar(&request, create, version == 7)?;
    let operation = text(&request, "operation")?;
    let digest = record_digest(&config)?.to_prefixed();
    for (source, contract) in [
        (
            RELATIONS,
            "ToS/contracts/semantic-relation-type-registry.schema.json",
        ),
        (
            ENTITIES,
            "ToS/contracts/semantic-entity-type-registry.schema.json",
        ),
    ] {
        schema_check(
            executor,
            source,
            &json_file(ctx, source)?,
            contract,
            deadline,
            cancelled,
        )?;
    }
    let files = package(ctx, &p)?;
    let mut response = object(vec![
        (
            "schema_version",
            string(if create {
                "tos_local_claim_create_result_v1"
            } else {
                "tos_local_claim_revision_result_v1"
            }),
        ),
        ("authentication", string("local-unix-account")),
        ("owner_configuration", string(&digest)),
        ("source_path", string(p.as_str())),
        (
            "allowed_operations",
            field(&config, "allowed_operations")?.clone(),
        ),
        ("grants_admission", JsonValue::Bool(false)),
        ("replayed", JsonValue::Bool(false)),
        ("receipt", JsonValue::Null),
    ]);
    if create {
        set(
            &mut response,
            "target_exists",
            JsonValue::Bool(!files.is_empty()),
        )?;
        let mut profiles = object(vec![]);
        for predicate in array(&config, "allowed_predicates")? {
            let predicate = predicate
                .as_str()
                .ok_or(SourceCommandError::Invalid("predicate scope"))?;
            set(&mut profiles, predicate, profile(ctx, predicate)?.1)?
        }
        set(&mut response, "source_claim_profiles", profiles)?;
        for key in [
            "allowed_claim_ids",
            "allowed_subject_refs",
            "allowed_object_refs",
            "allowed_evidence_refs",
            "allowed_object_values",
            "allowed_related_claim_refs",
        ] {
            if let Some(v) = config.object_get(key) {
                set(&mut response, key, v.clone())?
            }
        }
        if operation == "describe" {
            return ctx.plan(&handler, response, vec![], false);
        }
        grant(&config, "allowed_operations", &string("claims.create"))?;
        let claims = array(&request, "claims")?;
        if claims.is_empty() || claims.len() > 32 {
            return Err(SourceCommandError::Invalid("initial Claim batch capacity"));
        }
        let mut seen = BTreeSet::new();
        let mut raw = Vec::new();
        for claim in claims {
            claim_scope(&config, claim, true, version)?;
            if integer(claim, "claim_version")? != 1
                || claim
                    .object_get("assessment_refs")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|a| !a.is_empty())
            {
                return Err(SourceCommandError::Denied(
                    "initial Claim cannot revise or assess",
                ));
            }
            if !seen.insert(text(claim, "claim_id")?) {
                return Err(SourceCommandError::Conflict("duplicate initial Claim"));
            }
            validate_ground(ctx, &config, claim, version, executor, deadline, cancelled)?;
            raw.extend(canonical(claim)?);
            raw.push(b'\n');
        }
        let current_claims = selected_claim_ids(ctx)?;
        if seen.iter().any(|id| current_claims.contains(*id)) {
            return Err(SourceCommandError::Conflict(
                "initial Claim identity exists in current selected source",
            ));
        }
        for claim in claims {
            for alternative in claim
                .object_get("alternative_claim_refs")
                .and_then(JsonValue::as_array)
                .unwrap_or(&[])
            {
                let id = alternative
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("alternative Claim identity"))?;
                if !seen.contains(id) && !current_claims.contains(id) {
                    return Err(SourceCommandError::Unsupported(
                        "alternative Claim source not selected",
                    ));
                }
            }
        }
        if !files.is_empty() {
            return Err(SourceCommandError::Unsupported(
                "creation replay requires complete retained provenance and predecessor closure",
            ));
        }
        let dependencies = dependencies(ctx)?;
        set(&mut response, "expected_dependencies", dependencies.clone())?;
        set(&mut response, "source_bindings", bindings(ctx)?)?;
        set(
            &mut response,
            "prepared_files",
            refs(&BTreeMap::from([(CLAIM_STREAM.into(), raw.clone())])),
        )?;
        if operation == "claims.create" {
            return Err(SourceCommandError::Unsupported(
                "Claim creation serialization provenance event/environment capture is not yet ported",
            ));
        }
        return ctx.plan(
            &handler,
            response,
            vec![SourceChange {
                path: p,
                before: None,
                after: Some(raw),
            }],
            false,
        );
    }
    let stream = files
        .get(CLAIM_STREAM)
        .ok_or(SourceCommandError::Conflict("Claim source package absent"))?;
    let records = rows(stream)?;
    let id = text(&config, "claim_id")?;
    let record = records
        .get(id)
        .ok_or(SourceCommandError::Conflict("delegated Claim absent"))?;
    let history = verify_claim_history(ctx, &config, &p, &files)?;
    set(&mut response, "source", metadata_subject(record)?)?;
    set(&mut response, "revision", revision(&files)?)?;
    if operation == "describe" {
        return ctx.plan(&handler, response, vec![], false);
    }
    if operation == "inspect-version" {
        for receipt in array(&history, "receipts")? {
            if same(
                field(receipt, "previous_source")?,
                field(&request, "source")?,
            )? && text(field(receipt, "previous_source")?, "id")? == id
            {
                let archived = archive(ctx, &config, receipt)?;
                set(
                    &mut response,
                    "record",
                    rows(&archived[CLAIM_STREAM])?
                        .get(id)
                        .ok_or(SourceCommandError::Conflict("archived Claim absent"))?
                        .clone(),
                )?;
                set(
                    &mut response,
                    "inspected_source",
                    field(&request, "source")?.clone(),
                )?;
                set(&mut response, "files", refs(&archived))?;
                return ctx.plan(&handler, response, vec![], false);
            }
        }
        return Err(SourceCommandError::Conflict(
            "exact Claim version not retained",
        ));
    }
    grant(&config, "allowed_operations", &string("claim.revise"))?;
    let fields = field(&request, "fields")?;
    for (key, _) in fields
        .as_object()
        .ok_or(SourceCommandError::Invalid("Claim field patch"))?
    {
        grant(&config, "allowed_fields", &JsonValue::String(key.clone()))?
    }
    if text(&request, "reason")?.trim().is_empty()
        || text(&request, "reason")?.chars().count() > 4096
    {
        return Err(SourceCommandError::Invalid(
            "bounded authored correction reason",
        ));
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = fields.object_get(key) {
            for value in values.as_array().ok_or(SourceCommandError::Invalid(
                "Claim evidence correction array",
            ))? {
                grant(&config, "allowed_evidence_refs", value)?
            }
        }
    }
    if let Some(value) = fields.object_get("object") {
        grant(&config, "allowed_object_values", value)?;
    }
    let layer = if version == 7 {
        let v = field(&request, "layer_transition")?;
        grant(&config, "allowed_layer_transitions", v)?;
        Some(v)
    } else {
        None
    };
    claim_scope(&config, record, false, version)?;
    if operation == "claim.revise" {
        for receipt in array(&history, "receipts")? {
            if text(receipt, "command_id")? != text(&request, "command_id")? {
                continue;
            }
            if text(receipt, "request_digest")? != record_digest(&request)?.to_prefixed()
                || text(receipt, "owner_configuration")? != digest
                || text(receipt, "principal_id")? != text(&config, "principal_id")?
                || text(receipt, "authority_ref")? != text(&config, "authority_ref")?
                || text(&request, "expected_configuration")? != digest
            {
                return Err(SourceCommandError::Conflict(
                    "Claim command identity reused or delegation changed",
                ));
            }
            for selector in array(&request, "forms")? {
                grant(&config, "allowed_form_ids", field(selector, "form_id")?)?;
                if config.object_get("allowed_form_field_ids").is_some() {
                    grant(
                        &config,
                        "allowed_form_field_ids",
                        field(selector, "field_id")?,
                    )?
                } else if text(selector, "field_id")? != "claim.statement" {
                    return Err(SourceCommandError::Denied("Claim form field scope"));
                }
            }
            validate_ground(ctx, &config, record, version, executor, deadline, cancelled)?;
            let retained = archive(ctx, &config, receipt)?;
            let predecessors = rows(&retained[CLAIM_STREAM])?;
            let predecessor = predecessors.get(id).ok_or(SourceCommandError::Conflict(
                "Claim retry predecessor absent",
            ))?;
            let successor = advance_claim(predecessor, fields, layer)?;
            claim_scope(&config, &successor, false, version)?;
            validate_ground(
                ctx, &config, &successor, version, executor, deadline, cancelled,
            )?;
            set(&mut response, "receipt", receipt.clone())?;
            set(&mut response, "replayed", JsonValue::Bool(true))?;
            return ctx.plan(&handler, response, vec![], true);
        }
    }
    let revised = advance_claim(record, fields, layer)?;
    claim_scope(&config, &revised, false, version)?;
    validate_ground(
        ctx, &config, &revised, version, executor, deadline, cancelled,
    )?;
    let forms = array(&request, "forms")?;
    if forms.is_empty() || forms.len() > 32 {
        return Err(SourceCommandError::Invalid("Claim form selections"));
    }
    let formname = form_name(id);
    let current = files.get(&formname).map(|raw| parse(raw)).transpose()?;
    let mut changes = Vec::new();
    let mut selected_forms = BTreeSet::new();
    let mut statement = false;
    for selection in forms {
        exact_keys(selection, &["form_id", "field_id"])?;
        let form_id = text(selection, "form_id")?;
        let field_id = text(selection, "field_id")?;
        grant(&config, "allowed_form_ids", &string(form_id))?;
        if config.object_get("allowed_form_field_ids").is_some() {
            grant(&config, "allowed_form_field_ids", &string(field_id))?
        } else if !["claim.statement"].contains(&field_id) {
            return Err(SourceCommandError::Denied("Claim form field scope"));
        }
        if !selected_forms.insert(form_id) {
            return Err(SourceCommandError::Conflict("repeated Claim form"));
        }
        statement |= field_id == "claim.statement";
        changes.push(prepare_form_change(
            &revised,
            current.as_ref(),
            text(&config, "principal_id")?,
            form_id,
            field_id,
        )?);
    }
    if !statement {
        return Err(SourceCommandError::Denied(
            "Claim correction requires complete statement",
        ));
    }
    if let Some(current) = &current {
        for f in array(current, "forms")? {
            if !selected_forms.contains(text(f, "form_id")?) {
                return Err(SourceCommandError::Denied(
                    "Claim correction must rebind every current form",
                ));
            }
        }
    }
    for (name, raw) in &files {
        if name.ends_with(".human-forms.json") && *name != formname {
            let sibling = parse(raw)?;
            for key in ["forms", "prior_forms"] {
                for form in array(&sibling, key)? {
                    if selected_forms.contains(text(form, "form_id")?) {
                        return Err(SourceCommandError::Conflict(
                            "sibling Claim owns selected form identity",
                        ));
                    }
                }
            }
        }
    }
    let payload = apply_form_changes(current.as_ref(), &metadata_subject(&revised)?, &changes)?;
    let views = materialize_source_forms(&revised, &payload)?;
    if views.iter().any(|v| text(v, "state").ok() != Some("ready")) {
        return Err(SourceCommandError::Invalid(
            "Claim forms must materialize ready",
        ));
    }
    let formrefs: Vec<_> = changes
        .iter()
        .map(|change| reference(field(change, "form")?, "form_id", "form_version"))
        .collect::<SourceCommandResult<_>>()?;
    set(
        &mut response,
        "prepared_source",
        metadata_subject(&revised)?,
    )?;
    set(
        &mut response,
        "prepared_forms",
        JsonValue::Array(formrefs.clone()),
    )?;
    set(
        &mut response,
        "prepared_materializations",
        JsonValue::Array(views),
    )?;
    set(&mut response, "expected_dependencies", dependencies(ctx)?)?;
    set(&mut response, "source_bindings", bindings(ctx)?)?;
    if operation == "claim.revise" {
        if !same(field(&request, "expected_configuration")?, &string(&digest))?
            || !same(
                field(&request, "expected_source")?,
                &metadata_subject(record)?,
            )?
            || !same(field(&request, "expected_revision")?, &revision(&files)?)?
            || !same(
                field(&request, "expected_dependencies")?,
                &dependencies(ctx)?,
            )?
            || !same(field(&request, "expected_inputs")?, &bindings(ctx)?)?
        {
            return Err(SourceCommandError::Conflict(
                "Claim prepared source/dependencies are stale",
            ));
        }
        if array(&history, "receipts")?.len() >= 128 {
            return Err(SourceCommandError::Invalid("Claim history capacity"));
        }
        let previous_revision = revision(&files)?;
        let archive_path = format!(
            "ToS/source-witnesses/.record-revisions/{}-{}",
            Digest256::of_bytes(id.as_bytes())
                .to_prefixed()
                .trim_start_matches("sha256:"),
            previous_revision
                .as_str()
                .ok_or(SourceCommandError::Invalid("package revision"))?
                .trim_start_matches("sha256:")
        );
        let mut field_names = fields
            .as_object()
            .ok_or(SourceCommandError::Invalid("field patch"))?
            .iter()
            .map(|(k, _)| k.as_str().unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        field_names.sort();
        let changed_fields = JsonValue::Array(field_names.iter().map(|k| string(k)).collect());
        let receipt = object(vec![
            ("command_id", field(&request, "command_id")?.clone()),
            (
                "request_digest",
                string(&record_digest(&request)?.to_prefixed()),
            ),
            ("principal_id", field(&config, "principal_id")?.clone()),
            ("authority_ref", field(&config, "authority_ref")?.clone()),
            ("owner_configuration", string(&digest)),
            ("recorded_at", string(&ctx.recorded_at)),
            ("reason", field(&request, "reason")?.clone()),
            ("previous_source", metadata_subject(record)?),
            ("source", metadata_subject(&revised)?),
            ("previous_revision", previous_revision.clone()),
            ("archive_path", string(&archive_path)),
            ("dependencies", dependencies(ctx)?),
            ("source_bindings", bindings(ctx)?),
            ("changed_fields", changed_fields),
            ("forms", JsonValue::Array(formrefs)),
            ("grants_admission", JsonValue::Bool(false)),
            ("request", request.clone()),
        ]);
        let manifest = object(vec![
            ("schema_version", string("tos_source_package_archive_v1")),
            ("source_path", string(p.as_str())),
            ("source", metadata_subject(record)?),
            ("revision", previous_revision),
            ("files", refs(&files)),
        ]);
        let mut output = Vec::new();
        for (name, raw) in &files {
            let path = path(&format!("{archive_path}/{name}"))?;
            if ctx.file(&path)?.is_some() {
                return Err(SourceCommandError::Conflict(
                    "Claim predecessor archive path occupied",
                ));
            }
            output.push(SourceChange {
                path,
                before: None,
                after: Some(raw.clone()),
            });
        }
        let manifest_path = path(&format!("{archive_path}/manifest.json"))?;
        if ctx.file(&manifest_path)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "Claim predecessor manifest path occupied",
            ));
        }
        output.push(SourceChange {
            path: manifest_path,
            before: None,
            after: Some(published(&manifest)?),
        });
        let mut retained = history.clone();
        let mut receipts = array(&retained, "receipts")?.to_vec();
        receipts.push(receipt.clone());
        set(&mut retained, "receipts", JsonValue::Array(receipts))?;
        let parent = p
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim parent"))?
            .0;
        output.push(SourceChange {
            path: p.clone(),
            before: Some(Digest256::of_bytes(stream)),
            after: Some(replace_claim_row(stream, &revised)?),
        });
        output.push(SourceChange {
            path: path(&format!("{parent}/{formname}"))?,
            before: files.get(&formname).map(|r| Digest256::of_bytes(r)),
            after: Some(published(&payload)?),
        });
        output.push(SourceChange {
            path: path(&format!("{parent}/{CLAIM_HISTORY}"))?,
            before: files.get(CLAIM_HISTORY).map(|r| Digest256::of_bytes(r)),
            after: Some(published(&retained)?),
        });
        set(&mut response, "receipt", receipt)?;
        return ctx.plan(&handler, response, output, false);
    }
    let parent = p.as_str().rsplit_once('/').unwrap().0;
    ctx.plan(
        &handler,
        response,
        vec![
            SourceChange {
                path: p.clone(),
                before: Some(Digest256::of_bytes(stream)),
                after: Some(replace_claim_row(stream, &revised)?),
            },
            SourceChange {
                path: path(&format!("{parent}/{formname}"))?,
                before: files.get(&formname).map(|r| Digest256::of_bytes(r)),
                after: Some(published(&payload)?),
            },
        ],
        false,
    )
}
fn dependencies(ctx: &CommandContext) -> SourceCommandResult<JsonValue> {
    // Rust command input closure; deliberately a separate digest contract from
    // Python's directory-scanning legacy snapshot. Includes exact raw bindings.
    Ok(string(&record_digest(&bindings(ctx)?)?.to_prefixed()))
}
fn bindings(ctx: &CommandContext) -> SourceCommandResult<JsonValue> {
    let mut files = BTreeMap::new();
    for f in &ctx.files {
        files.insert(f.path.as_str().to_owned(), f.raw.clone());
    }
    Ok(object(vec![
        (
            "schema_version",
            string("tos_rust_claim_selected_inputs_v1"),
        ),
        ("selected_files", refs(&files)),
    ]))
}
fn validate_ground(
    ctx: &CommandContext,
    config: &JsonValue,
    claim: &JsonValue,
    version: u8,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if !["public", "public_metadata_only"].contains(&text(claim, "visibility")?)
        || text(claim, "claim_type")? != "relation"
        || text(claim, "claim_id")? == text(claim, "subject_ref")?
        || claim.object_get("object").and_then(JsonValue::as_str)
            == claim.object_get("claim_id").and_then(JsonValue::as_str)
    {
        return Err(SourceCommandError::Denied(
            "public Claim identity and visibility profile",
        ));
    }
    let (relation, profile) = profile(ctx, text(claim, "predicate")?)?;
    let reader = text(&profile, "reader")?;
    if ![
        "semantic-relation-v1",
        "identity-relation-v1",
        "historical-temporal-v1",
        "document-catalogue-temporal-v1",
        "structured-value-v1",
        "structured-reference-value-v1",
        "identity-transition-v1",
        "identity-transition-v2",
    ]
    .contains(&reader)
    {
        return Err(SourceCommandError::Unsupported(
            "declared Claim reader adapter",
        ));
    }
    if reader == "document-catalogue-temporal-v1" && version != 5
        || version == 5 && reader != "document-catalogue-temporal-v1"
        || reader == "structured-reference-value-v1" && version != 4
        || reader == "structured-value-v1" && version < 3
    {
        return Err(SourceCommandError::Denied(
            "Claim reader needs separate value grant",
        ));
    }
    grant(
        &profile,
        "assertion_layers",
        field(claim, "assertion_layer")?,
    )?;
    let route = array(&profile, "schemas")?
        .iter()
        .find(|route| route.object_get("schema_version") == claim.object_get("schema_version"))
        .ok_or(SourceCommandError::Unsupported("Claim schema route"))?;
    for contract in [
        text(route, "schema_ref")?,
        "ToS/contracts/source-claim-record.schema.json",
    ] {
        selected(ctx, contract)?;
        schema_check(
            executor,
            text(config, "source_path")?,
            claim,
            contract,
            deadline,
            cancelled,
        )?;
    }
    if let Some(display) = claim
        .object_get("qualifiers")
        .and_then(|q| q.object_get("display_fields"))
    {
        if display
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some("tos_claim_display_fields_v1")
        {
            schema_check(
                executor,
                text(config, "source_path")?,
                field(claim, "qualifiers")?,
                "ToS/contracts/claim-display-fields.schema.json",
                deadline,
                cancelled,
            )?
        }
    }
    if reader.starts_with("identity-transition-") {
        return validate_identity_ground(ctx, config, claim, reader, executor, deadline, cancelled);
    }
    let value = field(claim, "object")?;
    let mut endpoints = vec![(
        text(claim, "subject_ref")?.to_owned(),
        field(&relation, "domain_type_ids")?.clone(),
    )];
    if let Some(id) = value.as_str() {
        endpoints.push((id.into(), field(&relation, "range_type_ids")?.clone()));
    } else {
        if let Some(kind) = profile.object_get("value_kind") {
            if !same(kind, field(value, "kind")?)? {
                return Err(SourceCommandError::Invalid(
                    "Claim declared structured value kind",
                ));
            }
            schema_check(
                executor,
                text(config, "source_path")?,
                value,
                "ToS/contracts/source-structured-value.schema.json",
                deadline,
                cancelled,
            )?;
        }
        if reader == "structured-reference-value-v1" {
            let constraint = field(&profile, "object_reference_set")?;
            let members = array(value, "members")?;
            if members.len() < integer(constraint, "min_items")? as usize
                || members.len() > integer(constraint, "max_items")? as usize
            {
                return Err(SourceCommandError::Invalid("reference member capacity"));
            }
            let mut seen = BTreeSet::new();
            for member in members {
                grant(config, "allowed_object_refs", member)?;
                let id = member
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("reference member identity"))?;
                if !seen.insert(id) || id == text(claim, "claim_id")? {
                    return Err(SourceCommandError::Invalid(
                        "reference member identity repeats assertion",
                    ));
                }
                endpoints.push((id.into(), field(constraint, "member_type_ids")?.clone()));
            }
            if constraint.object_get("subject_is_member") == Some(&JsonValue::Bool(true))
                && !seen.contains(text(claim, "subject_ref")?)
            {
                return Err(SourceCommandError::Invalid(
                    "required subject member absent",
                ));
            }
            if let Some(adapter) = constraint.object_get("structure_adapter") {
                if adapter.as_str() != Some("scoped-members-v1") {
                    return Err(SourceCommandError::Unsupported(
                        "scoped member structure adapter",
                    ));
                }
                validate_member_order(claim)?;
                if let Some(adapter) = constraint.object_get("basis_adapter") {
                    if adapter.as_str() != Some("collection-membership-versions-v1") {
                        return Err(SourceCommandError::Unsupported(
                            "collection membership basis adapter",
                        ));
                    }
                    ground_collection_membership(ctx, claim)?;
                }

                schema_check(
                    executor,
                    text(config, "source_path")?,
                    value,
                    "ToS/contracts/scoped-member-structure.schema.json",
                    deadline,
                    cancelled,
                )?;
            }
        }
        if value.object_get("kind").and_then(JsonValue::as_str) == Some("relative-order") {
            let anchor = field(field(value, "relative")?, "anchor_ref")?;
            if config.object_get("allowed_object_refs").is_some() {
                grant(config, "allowed_object_refs", anchor)?;
            }
            endpoints.push((
                anchor
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("relative anchor identity"))?
                    .into(),
                JsonValue::Array(vec![string("tos.entity.historical-situation")]),
            ));
        }
        if reader == "document-catalogue-temporal-v1" {
            let attribution = field(field(claim, "qualifiers")?, "catalogue_attribution")?;
            if text(attribution, "field_role")? != "assigned-date"
                || !allowed(claim, "evidence_refs", field(attribution, "evidence_ref")?)?
                || !same(
                    field(attribution, "source_wording")?,
                    field(value, "source_wording")?,
                )?
            {
                return Err(SourceCommandError::Invalid(
                    "Document catalogue date must bind exact field role, evidence and wording",
                ));
            }
        }
        // Both actual selected Claim contracts transitively evaluate the
        // temporal/documentDate definition through their declared $ref. No
        // synthetic fragment schema or alternate value grammar is introduced.
    }
    let entities = json_file(ctx, ENTITIES)?;
    let types = array(&entities, "types")?;
    for (id, allowed_types) in endpoints {
        let (record, _loc) = find_record(ctx, &id)?;
        let record_type = text(&record, "record_type")?;
        let entity = types
            .iter()
            .find(|entry| {
                entry
                    .object_get("source_mappings")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|mappings| {
                        mappings.iter().any(|mapping| {
                            mapping
                                .object_get("source_graph")
                                .and_then(JsonValue::as_str)
                                == Some("source-claims")
                                && mapping
                                    .object_get("source_kind_id")
                                    .and_then(JsonValue::as_str)
                                    == Some(record_type)
                        })
                    })
            })
            .ok_or(SourceCommandError::Unsupported(
                "endpoint registry source-kind mapping",
            ))?;
        if let Some(profile) = entity.object_get("source_record_profile") {
            let route = array(profile, "schemas")?
                .iter()
                .find(|route| {
                    route.object_get("schema_version") == record.object_get("schema_version")
                })
                .ok_or(SourceCommandError::Unsupported(
                    "endpoint source schema route",
                ))?;
            schema_check(
                executor,
                &_loc,
                &record,
                text(route, "schema_ref")?,
                deadline,
                cancelled,
            )?;
        } else {
            let basename = _loc
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid("native source basename"))?;
            if ![
                "agent",
                "place",
                "organization",
                "work",
                "expression",
                "edition",
                "collection",
                "item",
            ]
            .contains(&record_type)
                || basename != format!("{record_type}.json")
                || text(&record, "schema_version")? != "tos_corpus_record_v1"
            {
                return Err(SourceCommandError::Unsupported(
                    "native endpoint source carrier adapter",
                ));
            }
            schema_check(
                executor,
                &_loc,
                &record,
                "ToS/contracts/corpus-record.schema.json",
                deadline,
                cancelled,
            )?;
        }
        let ancestry = ancestry(types, text(entity, "type_id")?)?;
        if !allowed_types
            .as_array()
            .ok_or(SourceCommandError::Invalid("domain/range type set"))?
            .iter()
            .any(|v| v.as_str().is_some_and(|v| ancestry.contains(v)))
        {
            return Err(SourceCommandError::Invalid(
                "Claim endpoint registry domain/range",
            ));
        }
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(refs) = claim.object_get(key) {
            for reference in refs
                .as_array()
                .ok_or(SourceCommandError::Invalid("evidence array"))?
            {
                let r = reference
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("evidence identity"))?;
                if r.starts_with("ToS/") {
                    let p = path(r)?;
                    if p.as_str()
                        .split('/')
                        .any(|s| ["payload", "local-content"].contains(&s))
                    {
                        return Err(SourceCommandError::Denied(
                            "Claim evidence excludes private payload",
                        ));
                    }
                    selected(ctx, r)?;
                } else {
                    if !has_record_evidence(ctx, r)? && !indexed_evidence(ctx, r)? {
                        return Err(SourceCommandError::Unsupported(
                            "evidence identity, anchor or provenance source not selected",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
fn find_record(ctx: &CommandContext, id: &str) -> SourceCommandResult<(JsonValue, String)> {
    let mut found = None;
    for f in &ctx.files {
        if !f.path.as_str().starts_with("ToS/source-witnesses/")
            || f.path.as_str().contains("/.record-revisions/")
        {
            continue;
        }
        for line in if f.path.as_str().ends_with(".jsonl") {
            f.raw.split(|b| *b == b'\n').collect::<Vec<_>>()
        } else {
            vec![f.raw.as_slice()]
        } {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let record = match parse(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if record.object_get("record_id").and_then(JsonValue::as_str) == Some(id) {
                if found.is_some() {
                    return Err(SourceCommandError::Conflict(
                        "duplicate selected source record identity",
                    ));
                }
                found = Some((record, f.path.as_str().into()));
            }
        }
    }
    found.ok_or(SourceCommandError::Unsupported(
        "Claim endpoint source metadata not selected",
    ))
}
fn ancestry(types: &[JsonValue], id: &str) -> SourceCommandResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    let mut active = BTreeSet::new();
    let mut stack = vec![(id.to_owned(), false)];
    while let Some((id, leaving)) = stack.pop() {
        if leaving {
            active.remove(&id);
            result.insert(id);
            continue;
        }
        if active.contains(&id) {
            return Err(SourceCommandError::Invalid(
                "cyclic endpoint source type ancestry",
            ));
        }
        if result.contains(&id) {
            continue;
        }
        if result.len() + active.len() > 4096 {
            return Err(SourceCommandError::Invalid("type ancestry budget"));
        }
        let entry = types
            .iter()
            .find(|entry| {
                entry.object_get("type_id").and_then(JsonValue::as_str) == Some(id.as_str())
            })
            .ok_or(SourceCommandError::Invalid("unknown endpoint source type"))?;
        active.insert(id.clone());
        stack.push((id, true));
        for parent in array(entry, "parent_type_ids")? {
            stack.push((
                parent
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("type parent identity"))?
                    .into(),
                false,
            ));
        }
    }
    Ok(result)
}
fn archive(
    ctx: &CommandContext,
    config: &JsonValue,
    receipt: &JsonValue,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let id = text(field(receipt, "previous_source")?, "id")?;
    let rev = text(receipt, "previous_revision")?;
    let expected = format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(id.as_bytes())
            .to_prefixed()
            .trim_start_matches("sha256:"),
        rev.strip_prefix("sha256:")
            .ok_or(SourceCommandError::Invalid("package revision digest"))?
    );
    if text(receipt, "archive_path")? != expected {
        return Err(SourceCommandError::Conflict(
            "archive path is not subject/revision derived",
        ));
    }
    let mut files = package(ctx, &path(&format!("{expected}/{CLAIM_STREAM}"))?)?;
    let manifest = parse(&files.remove("manifest.json").ok_or(
        SourceCommandError::Unsupported("retained Claim archive manifest not selected"),
    )?)?;
    exact_keys(
        &manifest,
        &[
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ],
    )?;
    if text(&manifest, "schema_version")? != "tos_source_package_archive_v1"
        || text(&manifest, "source_path")? != text(config, "source_path")?
        || !same(
            field(&manifest, "source")?,
            field(receipt, "previous_source")?,
        )?
        || !same(field(&manifest, "revision")?, &revision(&files)?)?
        || !same(field(&manifest, "files")?, &refs(&files))?
    {
        return Err(SourceCommandError::Conflict(
            "retained Claim archive bytes differ",
        ));
    }
    Ok(files)
}
/// Validate the complete shared stream sequence before selecting an old version.
pub fn verify_claim_history(
    ctx: &CommandContext,
    config: &JsonValue,
    p: &RelativePath,
    files: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<JsonValue> {
    let history = files
        .get(CLAIM_HISTORY)
        .map(|r| parse(r))
        .transpose()?
        .unwrap_or_else(|| {
            object(vec![
                ("schema_version", string("tos_claim_revision_history_v1")),
                ("source_path", string(p.as_str())),
                ("receipts", JsonValue::Array(vec![])),
            ])
        });
    exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    if text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || text(&history, "source_path")? != p.as_str()
        || array(&history, "receipts")?.len() > 128
    {
        return Err(SourceCommandError::Invalid("Claim revision history"));
    }
    let mut commands = BTreeSet::new();
    let mut expected: Option<Vec<u8>> = None;
    for receipt in array(&history, "receipts")? {
        exact_keys(
            receipt,
            &[
                "command_id",
                "request_digest",
                "principal_id",
                "authority_ref",
                "owner_configuration",
                "recorded_at",
                "reason",
                "previous_source",
                "source",
                "previous_revision",
                "archive_path",
                "dependencies",
                "source_bindings",
                "changed_fields",
                "forms",
                "grants_admission",
                "request",
            ],
        )?;
        let request = field(receipt, "request")?;
        grammar(
            request,
            false,
            request.object_get("layer_transition").is_some(),
        )?;
        let mut changed = field(request, "fields")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("retained Claim field patch"))?
            .iter()
            .map(|(key, _)| key.as_str().unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        changed.sort();
        if !same(
            field(receipt, "changed_fields")?,
            &JsonValue::Array(changed.iter().map(|s| string(s)).collect()),
        )? {
            return Err(SourceCommandError::Conflict(
                "retained changed fields differ",
            ));
        }

        if !commands.insert(text(receipt, "command_id")?)
            || text(request, "operation")? != "claim.revise"
            || text(receipt, "request_digest")? != record_digest(request)?.to_prefixed()
            || field(receipt, "grants_admission")? != &JsonValue::Bool(false)
        {
            return Err(SourceCommandError::Conflict(
                "broken Claim retained receipt",
            ));
        }
        for (left, right) in [
            ("command_id", "command_id"),
            ("previous_source", "expected_source"),
            ("previous_revision", "expected_revision"),
            ("owner_configuration", "expected_configuration"),
            ("dependencies", "expected_dependencies"),
            ("source_bindings", "expected_inputs"),
            ("reason", "reason"),
        ] {
            if !same(field(receipt, left)?, field(request, right)?)? {
                return Err(SourceCommandError::Conflict(
                    "retained Claim request differs",
                ));
            }
        }
        let retained = archive(ctx, config, receipt)?;
        let before = retained
            .get(CLAIM_STREAM)
            .ok_or(SourceCommandError::Conflict("archived stream absent"))?;
        if let Some(expected) = &expected {
            if before != expected {
                return Err(SourceCommandError::Conflict(
                    "Claim stream changed outside history",
                ));
            }
        } else if rows(before)?
            .values()
            .any(|v| integer(v, "claim_version").ok() != Some(1))
        {
            return Err(SourceCommandError::Conflict(
                "initial Claim history missing",
            ));
        }
        let id = text(field(receipt, "previous_source")?, "id")?;
        let records = rows(before)?;
        let previous = records
            .get(id)
            .ok_or(SourceCommandError::Conflict("retained predecessor absent"))?;
        if !same(
            &metadata_subject(previous)?,
            field(receipt, "previous_source")?,
        )? {
            return Err(SourceCommandError::Conflict("retained predecessor differs"));
        }
        let revised = advance_claim(
            previous,
            field(request, "fields")?,
            request.object_get("layer_transition"),
        )?;
        if !same(&metadata_subject(&revised)?, field(receipt, "source")?)? {
            return Err(SourceCommandError::Conflict("retained successor differs"));
        }
        expected = Some(replace_claim_row(before, &revised)?);
        // Full forms continuity requires re-rendering every retained selector.
        let name = form_name(id);
        let prior = retained.get(&name).map(|r| parse(r)).transpose()?;
        let selectors = array(request, "forms")?;
        if selectors.is_empty() || selectors.len() > 32 {
            return Err(SourceCommandError::Invalid("retained Claim form capacity"));
        }
        let mut selected_forms = BTreeSet::new();
        let mut statement = false;
        for selection in selectors {
            exact_keys(selection, &["form_id", "field_id"])?;
            if !selected_forms.insert(text(selection, "form_id")?) {
                return Err(SourceCommandError::Conflict("repeated retained Claim form"));
            }
            statement |= text(selection, "field_id")? == "claim.statement";
        }
        if !statement {
            return Err(SourceCommandError::Conflict(
                "retained Claim correction omitted statement",
            ));
        }
        if let Some(prior) = &prior {
            for form in array(prior, "forms")? {
                if !selected_forms.contains(text(form, "form_id")?) {
                    return Err(SourceCommandError::Conflict(
                        "retained Claim correction omitted prior form",
                    ));
                }
            }
        }
        let changes = array(request, "forms")?
            .iter()
            .map(|s| {
                prepare_form_change(
                    &revised,
                    prior.as_ref(),
                    text(receipt, "principal_id")?,
                    text(s, "form_id")?,
                    text(s, "field_id")?,
                )
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let restored = apply_form_changes(prior.as_ref(), &metadata_subject(&revised)?, &changes)?;
        let _ = materialize_source_forms(&revised, &restored)?;
        let refs_result = JsonValue::Array(
            changes
                .iter()
                .map(|c| reference(field(c, "form")?, "form_id", "form_version"))
                .collect::<SourceCommandResult<_>>()?,
        );
        if !same(&refs_result, field(receipt, "forms")?)? {
            return Err(SourceCommandError::Conflict(
                "retained Claim form results differ",
            ));
        }
        let current = parse(files.get(&name).ok_or(SourceCommandError::Conflict(
            "retained Claim form set absent",
        ))?)?;
        for wanted in array(receipt, "forms")? {
            let mut present = false;
            for key in ["forms", "prior_forms"] {
                for form in array(&current, key)? {
                    if same(&reference(form, "form_id", "form_version")?, wanted)? {
                        present = true
                    }
                }
            }
            if !present {
                return Err(SourceCommandError::Conflict(
                    "Claim result form no longer retained",
                ));
            }
        }
    }
    let current = files
        .get(CLAIM_STREAM)
        .ok_or(SourceCommandError::Conflict("current Claim stream absent"))?;
    if let Some(expected) = expected {
        if *current != expected {
            return Err(SourceCommandError::Conflict(
                "Claim stream differs from history head",
            ));
        }
    } else if rows(current)?
        .values()
        .any(|v| integer(v, "claim_version").ok() != Some(1))
    {
        return Err(SourceCommandError::Conflict(
            "noninitial Claim has no history",
        ));
    }
    Ok(history)
}

fn selected_claim_ids(ctx: &CommandContext) -> SourceCommandResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for file in &ctx.files {
        if file.path.as_str().starts_with("ToS/source-witnesses/")
            && file.path.as_str().ends_with("/source-claims.jsonl")
            && !file.path.as_str().contains("/.record-revisions/")
        {
            for id in rows(&file.raw)?.into_keys() {
                if !result.insert(id) {
                    return Err(SourceCommandError::Conflict(
                        "duplicate current selected Claim identity",
                    ));
                }
            }
        }
    }
    Ok(result)
}

fn indexed_evidence(ctx: &CommandContext, identity: &str) -> SourceCommandResult<bool> {
    let mut count = 0usize;
    for file in &ctx.files {
        let p = file.path.as_str();
        let basename = p.rsplit('/').next().unwrap_or("");
        if !p.starts_with("ToS/source-witnesses/")
            || p.contains("/.record-revisions/")
            || p.contains("/catalog/")
            || !basename.ends_with(".jsonl")
        {
            continue;
        }
        let key = if basename.contains("anchor") {
            "anchor_id"
        } else if basename.contains("provenance") {
            "event_id"
        } else {
            continue;
        };
        for line in file.raw.split(|b| *b == b'\n') {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let row = parse(line)?;
            if row.object_get(key).and_then(JsonValue::as_str) == Some(identity) {
                count += 1;
            }
        }
    }
    if count > 1 {
        return Err(SourceCommandError::Conflict(
            "duplicate selected evidence identity",
        ));
    }
    Ok(count == 1)
}

fn has_record_evidence(ctx: &CommandContext, identity: &str) -> SourceCommandResult<bool> {
    match find_record(ctx, identity) {
        Ok(_) => Ok(true),
        Err(SourceCommandError::Unsupported(_)) => Ok(false),
        Err(error) => Err(error),
    }
}
fn exact_ref(value: &JsonValue) -> SourceCommandResult<()> {
    exact_keys(value, &["id", "version", "digest"])?;
    if text(value, "id")?.is_empty()
        || integer(value, "version")? == 0
        || integer(value, "version")? > 9_007_199_254_740_991
        || Digest256::from_prefixed(text(value, "digest")?).is_err()
    {
        return Err(SourceCommandError::Invalid("exact source reference"));
    }
    Ok(())
}
fn validate_identity_ground(
    ctx: &CommandContext,
    config: &JsonValue,
    claim: &JsonValue,
    reader: &str,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let v2 = reader == "identity-transition-v2";
    let predicate = if v2 {
        "subject_identity_transition_proposal"
    } else {
        "identity_transition_proposal"
    };
    let owner = text(config, "schema_version")?;
    if !owner.starts_with("tos_local_identity_proposal_")
        || !owner.ends_with(if v2 { "_v2" } else { "_v1" })
        || text(claim, "predicate")? != predicate
    {
        return Err(SourceCommandError::Denied(
            "identity proposal requires its separate exact reader grant",
        ));
    }
    let value = field(claim, "object")?;
    grant(config, "allowed_object_values", value)?;
    let previous = field(value, "supersedes_proposal")?;
    let left = array(value, "predecessors")?;
    let right = array(value, "successors")?;
    let members = array(value, "members")?;
    let mappings = array(value, "mapping")?;
    if left.is_empty()
        || right.is_empty()
        || left.len() > 8
        || right.len() > 8
        || members.len() < 3
        || members.len() > 9
        || mappings.len() < 2
        || mappings.len() > 8
    {
        return Err(SourceCommandError::Invalid(
            "identity proposal participant capacity",
        ));
    }
    let mut identities = BTreeSet::new();
    let mut predecessors = BTreeSet::new();
    let mut successors = BTreeSet::new();
    for (refs, target) in [(left, &mut predecessors), (right, &mut successors)] {
        for reference in refs {
            exact_ref(reference)?;
            let id = text(reference, "id")?;
            if !identities.insert(id.to_owned()) || id == text(claim, "claim_id")? {
                return Err(SourceCommandError::Invalid(
                    "identity proposal participants must be distinct",
                ));
            }
            target.insert(id.to_owned());
            grant(config, "allowed_object_refs", &string(id))?;
        }
    }
    if !predecessors.contains(text(claim, "subject_ref")?)
        || members.len() != identities.len()
        || members
            .iter()
            .filter_map(JsonValue::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            != identities
    {
        return Err(SourceCommandError::Invalid(
            "proposal complete member set and predecessor subject",
        ));
    }
    match text(value, "operation")? {
        "merge" if left.len() >= 2 && right.len() == 1 => {}
        "split" if left.len() == 1 && right.len() >= 2 => {}
        _ => {
            return Err(SourceCommandError::Invalid(
                "proposal must be merge N-to-one or split one-to-N",
            ));
        }
    }
    let mut actual = BTreeSet::new();
    for edge in mappings {
        exact_keys(edge, &["predecessor", "successor"])?;
        let from = text(edge, "predecessor")?;
        let to = text(edge, "successor")?;
        if !predecessors.contains(from)
            || !successors.contains(to)
            || !actual.insert((from.to_owned(), to.to_owned()))
        {
            return Err(SourceCommandError::Invalid(
                "identity mapping outside participants or repeated",
            ));
        }
    }
    if actual.len() != left.len() * right.len() {
        return Err(SourceCommandError::Invalid(
            "identity mapping must cover every participant",
        ));
    }
    let entities = json_file(ctx, ENTITIES)?;
    let types = array(&entities, "types")?;
    for reference in left.iter().chain(right) {
        let (record, source) = find_record(ctx, text(reference, "id")?)?;
        if !same(&metadata_subject(&record)?, reference)? {
            return Err(SourceCommandError::Unsupported(
                "identity participant exact historical metadata reader",
            ));
        }
        let kind = text(&record, "record_type")?;
        let entry = types
            .iter()
            .find(|entry| {
                entry
                    .object_get("source_mappings")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|mappings| {
                        mappings.iter().any(|mapping| {
                            mapping
                                .object_get("source_graph")
                                .and_then(JsonValue::as_str)
                                == Some("source-claims")
                                && mapping
                                    .object_get("source_kind_id")
                                    .and_then(JsonValue::as_str)
                                    == Some(kind)
                        })
                    })
            })
            .ok_or(SourceCommandError::Invalid(
                "identity participant source type",
            ))?;
        if entry.object_get("abstract") != Some(&JsonValue::Bool(false)) {
            return Err(SourceCommandError::Invalid(
                "identity participant must have concrete source type",
            ));
        }
        let role = text(entry, "object_role")?;
        if role != "identity" && !(v2 && role == "semantic") {
            return Err(SourceCommandError::Denied(
                "identity reader participant role",
            ));
        }
        if let Some(profile) = entry.object_get("source_record_profile") {
            if role == "semantic"
                && (text(profile, "reader")? != "semantic-metadata-v1"
                    || text(profile, "identity_proposal_adapter")? != "exact-semantic-metadata-v1"
                    || text(profile, "record_type")? != kind
                    || !source.ends_with(&format!("/{}", text(profile, "source_basename")?))
                    || !source.starts_with("ToS/source-witnesses/")
                    || source.split('/').any(|s| {
                        s.starts_with('.')
                            || [
                                "catalog",
                                "payload",
                                "private",
                                "local-content",
                                "owner-local",
                            ]
                            .contains(&s)
                    })
                    || !["public", "public_metadata_only"].contains(&text(&record, "visibility")?))
            {
                return Err(SourceCommandError::Denied(
                    "semantic identity participant needs exact opted-in source representation",
                ));
            }
            let route = array(profile, "schemas")?
                .iter()
                .find(|route| {
                    route.object_get("schema_version") == record.object_get("schema_version")
                })
                .ok_or(SourceCommandError::Unsupported(
                    "identity participant source schema route",
                ))?;
            schema_check(
                executor,
                &source,
                &record,
                text(route, "schema_ref")?,
                deadline,
                cancelled,
            )?;
        } else {
            if role != "identity"
                || ![
                    "agent",
                    "place",
                    "organization",
                    "work",
                    "expression",
                    "edition",
                    "collection",
                    "item",
                ]
                .contains(&kind)
                || !source.ends_with(&format!("/{kind}.json"))
            {
                return Err(SourceCommandError::Unsupported(
                    "identity native participant source adapter",
                ));
            }
            schema_check(
                executor,
                &source,
                &record,
                "ToS/contracts/corpus-record.schema.json",
                deadline,
                cancelled,
            )?;
        }
    }
    let unresolved = array(value, "unresolved_links")?;
    if unresolved.len() > 32 {
        return Err(SourceCommandError::Invalid("unresolved Claim capacity"));
    }
    let mut related = Vec::new();
    for entry in unresolved {
        related.push(field(entry, "claim")?)
    }
    if !previous.is_null() {
        related.push(previous)
    }
    let expected_navigation = if previous.is_null() {
        JsonValue::Null
    } else {
        string(text(previous, "id")?)
    };
    if !same(
        claim
            .object_get("supersedes_claim_ref")
            .unwrap_or(&JsonValue::Null),
        &expected_navigation,
    )? {
        return Err(SourceCommandError::Invalid(
            "proposal predecessor navigation differs",
        ));
    }
    for reference in related {
        exact_ref(reference)?;
        grant(config, "allowed_related_claim_refs", reference)?;
        if text(reference, "id")? == text(claim, "claim_id")? {
            return Err(SourceCommandError::Invalid(
                "identity proposal cannot reference itself as retained assertion",
            ));
        }
        let retained = resolve_claim_reference(ctx, reference)?;
        if same(reference, previous)? {
            let p = text(&retained, "predicate")?;
            if p != "identity_transition_proposal"
                && !(v2 && p == "subject_identity_transition_proposal")
            {
                return Err(SourceCommandError::Invalid(
                    "identity predecessor reader is incompatible",
                ));
            }
            let (_, profile) = profile(ctx, p)?;
            let route = array(&profile, "schemas")?
                .iter()
                .find(|route| {
                    route.object_get("schema_version") == retained.object_get("schema_version")
                })
                .ok_or(SourceCommandError::Invalid(
                    "identity predecessor schema route",
                ))?;
            schema_check(
                executor,
                text(config, "source_path")?,
                &retained,
                text(route, "schema_ref")?,
                deadline,
                cancelled,
            )?;
        }
    }
    Ok(())
}
fn resolve_claim_reference(
    ctx: &CommandContext,
    reference: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    exact_ref(reference)?;
    let id = text(reference, "id")?;
    let mut found = None;
    for file in &ctx.files {
        if !file.path.as_str().starts_with("ToS/source-witnesses/")
            || !file.path.as_str().ends_with("/source-claims.jsonl")
            || file.path.as_str().contains("/.record-revisions/")
        {
            continue;
        }
        let records = rows(&file.raw)?;
        if let Some(record) = records.get(id) {
            if found.is_some() {
                return Err(SourceCommandError::Conflict(
                    "duplicate current Claim owner",
                ));
            }
            found = Some((record.clone(), file.path.clone()));
        }
    }
    let (record, p) = found.ok_or(SourceCommandError::Unsupported(
        "related Claim owner source not selected",
    ))?;
    let mut config = parse(&ctx.configuration_raw)?;
    set(&mut config, "source_path", string(p.as_str()))?;
    set(&mut config, "claim_id", string(id))?;
    let files = package(ctx, &p)?;
    let history = verify_claim_history(ctx, &config, &p, &files)?;
    if same(&metadata_subject(&record)?, reference)? {
        return Ok(record);
    }
    for receipt in array(&history, "receipts")? {
        if same(field(receipt, "previous_source")?, reference)? {
            let retained = archive(ctx, &config, receipt)?;
            return rows(&retained[CLAIM_STREAM])?
                .remove(id)
                .ok_or(SourceCommandError::Conflict(
                    "exact related Claim absent in archive",
                ));
        }
    }
    Err(SourceCommandError::Unsupported(
        "exact related Claim version not retained in selected owner history",
    ))
}

fn validate_member_order(claim: &JsonValue) -> SourceCommandResult<()> {
    let value = field(claim, "object")?;
    let members = array(value, "members")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(SourceCommandError::Invalid("member identity"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if members.contains(text(claim, "subject_ref")?) {
        return Err(SourceCommandError::Invalid(
            "scoped composition cannot contain its subject",
        ));
    }
    let order = field(value, "ordering")?;
    let edges = array(order, "precedes")?;
    let mode = text(order, "mode")?;
    if !["unordered", "partial", "total"].contains(&mode)
        || mode == "unordered" && !edges.is_empty()
    {
        return Err(SourceCommandError::Invalid("scoped member ordering mode"));
    }
    let mut outgoing: BTreeMap<&str, BTreeSet<&str>> =
        members.iter().map(|id| (*id, BTreeSet::new())).collect();
    let mut indegree: BTreeMap<&str, usize> = members.iter().map(|id| (*id, 0)).collect();
    for edge in edges {
        let edge = edge
            .as_array()
            .filter(|e| e.len() == 2)
            .ok_or(SourceCommandError::Invalid("member precedence pair"))?;
        let from = edge[0]
            .as_str()
            .ok_or(SourceCommandError::Invalid("member precedence identity"))?;
        let to = edge[1]
            .as_str()
            .ok_or(SourceCommandError::Invalid("member precedence identity"))?;
        if !members.contains(from) || !members.contains(to) {
            return Err(SourceCommandError::Invalid(
                "precedence endpoint outside scoped members",
            ));
        }
        if !outgoing
            .get_mut(from)
            .ok_or(SourceCommandError::Invalid("precedence source"))?
            .insert(to)
        {
            return Err(SourceCommandError::Invalid(
                "duplicate member precedence pair",
            ));
        }
        *indegree
            .get_mut(to)
            .ok_or(SourceCommandError::Invalid("precedence target"))? += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(*id))
        .collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        if mode == "total" && !ready.is_empty() {
            return Err(SourceCommandError::Invalid(
                "total scoped member order leaves members incomparable",
            ));
        }
        visited += 1;
        for following in &outgoing[id] {
            let count = indegree
                .get_mut(following)
                .ok_or(SourceCommandError::Invalid("precedence target"))?;
            *count -= 1;
            if *count == 0 {
                ready.push(following)
            }
        }
    }
    if visited != members.len() {
        return Err(SourceCommandError::Invalid("cyclic scoped member order"));
    }
    Ok(())
}

fn ground_collection_membership(
    ctx: &CommandContext,
    claim: &JsonValue,
) -> SourceCommandResult<()> {
    let value = field(claim, "object")?;
    let collection_ref = field(value, "collection_version")?;
    exact_ref(collection_ref)?;
    let subject = text(claim, "subject_ref")?;
    if text(collection_ref, "id")? != subject || !subject.starts_with("tos.collection.") {
        return Err(SourceCommandError::Invalid(
            "Collection order exact own Collection version",
        ));
    }
    let (collection, _) = find_record(ctx, subject)?;
    if text(&collection, "record_type")? != "collection"
        || !same(&metadata_subject(&collection)?, collection_ref)?
    {
        return Err(SourceCommandError::Unsupported(
            "Collection order exact historical metadata basis reader",
        ));
    }
    let members = array(value, "members")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(SourceCommandError::Invalid("Collection member identity"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let bindings = array(value, "membership_versions")?;
    if bindings.len() != members.len() {
        return Err(SourceCommandError::Invalid(
            "one exact membership binding per Collection member",
        ));
    }
    let mut binding_ids = BTreeSet::new();
    let mut resolved = BTreeSet::new();
    let declared = collection
        .object_get("membership_claim_refs")
        .and_then(JsonValue::as_array)
        .unwrap_or(&[]);
    for reference in bindings {
        exact_ref(reference)?;
        let id = text(reference, "id")?;
        if !binding_ids.insert(id) || !declared.iter().any(|v| v.as_str() == Some(id)) {
            return Err(SourceCommandError::Invalid(
                "membership basis is repeated or absent from exact Collection",
            ));
        }
        let member = resolve_claim_reference(ctx, reference)?;
        let legacy = text(&member, "schema_version")? == "tos_claim_packet_v1"
            && text(&member, "claim_type")? == "bibliographic";
        let native = text(&member, "schema_version")? == "tos_source_relation_claim_v1"
            && text(&member, "claim_type")? == "relation";
        let object = text(&member, "object")?;
        let polarity = member
            .object_get("polarity")
            .and_then(JsonValue::as_str)
            .or(if legacy { Some("positive") } else { None });
        if !(legacy || native)
            || !["bibliographic_assertion", "scholarly_report"]
                .contains(&text(&member, "assertion_layer")?)
            || text(&member, "subject_ref")? != subject
            || text(&member, "predicate")? != "contains_work"
            || polarity != Some("positive")
            || !members.contains(object)
            || !resolved.insert(object.to_owned())
        {
            return Err(SourceCommandError::Invalid(
                "basis must be distinct positive membership in exact Collection",
            ));
        }
    }
    if resolved != members.iter().map(|v| (*v).to_owned()).collect() {
        return Err(SourceCommandError::Invalid(
            "Collection membership basis does not close scoped member set",
        ));
    }
    Ok(())
}
