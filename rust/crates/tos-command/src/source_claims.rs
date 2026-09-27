//! Source-owned Claim command mechanics. Selected bytes are proposals, never
//! admission. Schemas are executed by the actual bounded source-cut worker.
//! Unsupported grounding adapters fail closed without flattening their values.
use crate::source_command::*;
use crate::source_forms::{
    apply_form_changes, materialize_source_forms, metadata_subject, prepare_form_change,
};
use crate::source_sign_native::{NativeReadKind, NativeReadScope, SignNativeRead};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath, python_strip_unicode16_v1};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
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
        if python_bytes_blank(line) {
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
        if !python_bytes_blank(line) && text(&parse(line)?, "claim_id")? == id {
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
        match key {
            "allowed_object_values" => {
                if value.as_object().is_none() {
                    return Err(SourceCommandError::Invalid("exact value scope object"));
                }
            }
            "allowed_related_claim_refs" => {
                exact_ref(value)?;
                if !text(value, "id")?.starts_with("tos.claim.") {
                    return Err(SourceCommandError::Invalid("related Claim scope identity"));
                }
            }
            "allowed_layer_transitions" => {
                exact_keys(value, &["from", "to"])?;
                let left = text(value, "from")?;
                let right = text(value, "to")?;
                if left == right
                    || ![left, right].iter().all(|name| {
                        name.len() <= 64
                            && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                            && name.bytes().all(|byte| {
                                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                            })
                    })
                {
                    return Err(SourceCommandError::Invalid("exact layer transition names"));
                }
            }
            _ => {
                let scope = value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("bounded Claim scope string"))?;
                if stripped(scope)?.is_empty() {
                    return Err(SourceCommandError::Invalid("bounded Claim scope string"));
                }
            }
        }
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
        || stripped(text(&c, "principal_id")?)?.is_empty()
        || stripped(text(&c, "authority_ref")?)?.is_empty()
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
    if create {
        if !identifier_tail(text(&c, "provenance_event_id")?, "tos.event.") {
            return Err(SourceCommandError::Invalid(
                "delegated provenance event identity",
            ));
        }
        if !identifier_tail(parts[3], "") {
            return Err(SourceCommandError::Invalid("named source relation package"));
        }
        for id in array(&c, "allowed_claim_ids")? {
            if !identifier_tail(
                id.as_str()
                    .ok_or(SourceCommandError::Invalid("delegated Claim identity"))?,
                "tos.claim.",
            ) {
                return Err(SourceCommandError::Invalid("delegated Claim identity"));
            }
        }
        if !["human", "software", "model"].contains(&text(&c, "maker_type")?) {
            return Err(SourceCommandError::Denied("delegated Claim maker type"));
        }
    } else {
        if !identifier_tail(text(&c, "claim_id")?, "tos.claim.") {
            return Err(SourceCommandError::Invalid("delegated Claim identity"));
        }
        for id in array(&c, "allowed_form_ids")? {
            let id = id
                .as_str()
                .and_then(|id| id.strip_prefix("tos.form."))
                .ok_or(SourceCommandError::Invalid("delegated form identity"))?;
            if id.is_empty()
                || !id.as_bytes()[0].is_ascii_lowercase() && !id.as_bytes()[0].is_ascii_digit()
                || !id.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || [b'.', b'_', b'-'].contains(&byte)
                })
            {
                return Err(SourceCommandError::Invalid("delegated form identity"));
            }
        }
    }
    exact_keys(&c, &keys)?;
    bounded_list(&c, "allowed_operations", 1)?;
    for operation in array(&c, "allowed_operations")? {
        if operation.as_str()
            != Some(if create {
                "claims.create"
            } else {
                "claim.revise"
            })
        {
            return Err(SourceCommandError::Invalid(
                "delegated Claim operation name",
            ));
        }
    }
    if let Some(fields) = c.object_get("allowed_form_field_ids") {
        let fields = fields
            .as_array()
            .ok_or(SourceCommandError::Invalid("Claim form field scope"))?;
        if fields.is_empty() || fields.len() > 4 {
            return Err(SourceCommandError::Invalid(
                "Claim form field scope capacity",
            ));
        }
        let mut seen = BTreeSet::new();
        for field in fields {
            let id = field
                .as_str()
                .ok_or(SourceCommandError::Invalid("Claim form field identity"))?;
            if ![
                "claim.statement",
                "claim.name",
                "claim.caption",
                "claim.hover",
            ]
            .contains(&id)
                || !seen.insert(id)
            {
                return Err(SourceCommandError::Invalid(
                    "exact known Claim form field scope",
                ));
            }
        }
    }

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
    run_claim_command_inner(ctx, None, false, executor, deadline, cancelled)
}

/// Authenticate the complete current member universe before inventory-based
/// absence, form allocation or maintained dependency fingerprint decisions.
/// Named Python sources are frozen rule inputs, never a runtime identity.
pub fn run_claim_command_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    if ctx
        .files
        .iter()
        .any(|input| !input.path.as_str().starts_with("ToS/"))
    {
        return Err(SourceCommandError::Unsupported(
            "Claim software rule inputs require separately selected software capture",
        ));
    }
    let files = complete_authored_inputs(ctx, cut, deadline, cancelled)?;
    let complete = CommandContext {
        files,
        ..ctx.clone()
    };
    complete.check()?;
    run_claim_command_inner(&complete, Some(ctx), false, executor, deadline, cancelled)
}

/// Software rule-contract bytes are custody-checked through their separate
/// selected component capture; they never become authored source members.
pub fn run_claim_command_from_captures(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    let complete =
        selected_claim_context_from_captures(ctx, cut, software, components, deadline, cancelled)?;
    run_claim_command_inner(&complete, Some(ctx), false, executor, deadline, cancelled)
}

fn selected_claim_context_from_captures(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommandContext> {
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let mut files = complete_authored_inputs(ctx, cut, deadline, cancelled)?;
    files.extend(
        ctx.files
            .iter()
            .filter(|input| !input.path.as_str().starts_with("ToS/"))
            .cloned(),
    );
    let complete = CommandContext {
        files,
        ..ctx.clone()
    };
    complete.check()?;
    Ok(complete)
}

/// A complete, native-observed Claim creation package. Its selected context is
/// retained for custody checks, not as a grant to the canonical source writer.
pub struct SerializedClaimCreation {
    context: CommandContext,
    command: PreparedCommand,
    home: RelativePath,
    files: BTreeMap<String, Vec<u8>>,
    components: SoftwareComponentSelectionV1,
}

impl SerializedClaimCreation {
    pub fn command(&self) -> &PreparedCommand {
        &self.command
    }
    pub fn home(&self) -> &RelativePath {
        &self.home
    }
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    pub(crate) fn context(&self) -> &CommandContext {
        &self.context
    }
    pub(crate) fn components(&self) -> &SoftwareComponentSelectionV1 {
        &self.components
    }
}

/// Complete Claim planning, native capture, provenance execution and the
/// maintained five-file receipt. Publication remains a separate held action.
fn serialize_claim_creation_from_captures(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SerializedClaimCreation> {
    claim_creation_from_captures(
        ctx, cut, software, components, worker, None, deadline, cancelled,
    )
}

/// Rebuild an original Claim creation from its retained package and the
/// independently selected original source/software captures. The receipt is
/// compared byte-for-byte; neither its timestamp nor its authority is minted.
fn restore_claim_creation_from_captures(
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    retained: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SerializedClaimCreation> {
    claim_creation_from_captures(
        ctx,
        original_cut,
        software,
        components,
        worker,
        Some(retained),
        deadline,
        cancelled,
    )
}

fn claim_creation_from_captures(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    retained: Option<&BTreeMap<String, Vec<u8>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SerializedClaimCreation> {
    let complete =
        selected_claim_context_from_captures(ctx, cut, software, components, deadline, cancelled)?;
    let request = parse(&complete.request_raw)?;
    if text(&request, "operation")? != "claims.create" {
        return Err(SourceCommandError::Unsupported(
            "native Claim serialization requires claims.create",
        ));
    }
    let preview = run_claim_command_inner(&complete, Some(ctx), true, worker, deadline, cancelled)?;
    if preview.changes.len() != 1 || preview.changes[0].before.is_some() {
        return Err(SourceCommandError::Conflict(
            "initial Claim serialization proposal closure",
        ));
    }
    let config = parse(&complete.configuration_raw)?;
    let home = path(
        preview.changes[0]
            .path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim creation home"))?
            .0,
    )?;
    let stream = preview.changes[0]
        .after
        .as_ref()
        .ok_or(SourceCommandError::Conflict("initial Claim stream absent"))?;
    let mut files = BTreeMap::from([(CLAIM_STREAM.to_owned(), stream.clone())]);
    if let Some(original) = retained {
        crate::source_serialization::restore_creation_capture(
            &request,
            text(&config, "provenance_event_id")?,
            home.as_str(),
            &mut files,
            original,
            software,
            components,
            deadline,
            cancelled,
        )?;
    } else {
        crate::source_serialization::capture_claim_creation(
            &request,
            text(&config, "provenance_event_id")?,
            home.as_str(),
            &mut files,
            software,
            components,
            deadline,
            cancelled,
        )?;
    }
    check_claim_capture(&complete, &home, &files, worker, deadline, cancelled)?;
    let recorded_at = if let Some(original) = retained {
        let raw =
            original
                .get("source-create-receipt.json")
                .ok_or(SourceCommandError::Conflict(
                    "retained Claim creation receipt absent",
                ))?;
        let receipt = parse(raw)?;
        let instant = text(&receipt, "recorded_at")?;
        validate_instant(instant)?;
        instant.to_owned()
    } else {
        crate::source_serialization::instant()?
    };
    let receipt =
        claim_creation_receipt(&config, &request, &preview.response, &files, &recorded_at)?;
    let mut receipt_raw = canonical(&receipt)?;
    receipt_raw.push(b'\n');
    files.insert("source-create-receipt.json".into(), receipt_raw);
    if files.len() != 5
        || files.values().any(|raw| raw.len() > 8_388_608)
        || files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|total| total > 33_554_432)
    {
        return Err(SourceCommandError::Invalid(
            "Claim creation package byte closure",
        ));
    }
    if retained.is_some_and(|original| original != &files) {
        return Err(SourceCommandError::Conflict(
            "retained Claim creation package differs from original request",
        ));
    }
    let mut result = preview.response.clone();
    for key in ["expected_dependencies", "source_bindings", "prepared_files"] {
        if let JsonValue::Object(members) = &mut result {
            members.retain(|(name, _)| name.as_str() != Some(key));
        }
    }
    set(&mut result, "target_exists", JsonValue::Bool(true))?;
    set(&mut result, "receipt", receipt)?;
    set(&mut result, "replayed", JsonValue::Bool(retained.is_some()))?;
    let changes = if retained.is_none() {
        files
            .iter()
            .map(|(name, raw)| {
                Ok(SourceChange {
                    path: path(&format!("{}/{name}", home.as_str()))?,
                    before: None,
                    after: Some(raw.clone()),
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?
    } else {
        vec![]
    };
    let command = complete.plan(&preview.handler_id, result, changes, retained.is_some())?;
    Ok(SerializedClaimCreation {
        context: complete,
        command,
        home,
        files,
        components: components.clone(),
    })
}

fn claim_creation_receipt(
    config: &JsonValue,
    request: &JsonValue,
    preview: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    recorded_at: &str,
) -> SourceCommandResult<JsonValue> {
    let claims = array(request, "claims")?
        .iter()
        .map(metadata_subject)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(object(vec![
        (
            "schema_version",
            string("tos_local_claim_create_receipt_v1"),
        ),
        ("command_id", field(request, "command_id")?.clone()),
        (
            "request_digest",
            string(&record_digest(request)?.to_prefixed()),
        ),
        ("principal_id", field(config, "principal_id")?.clone()),
        ("authority_ref", field(config, "authority_ref")?.clone()),
        (
            "owner_configuration",
            field(request, "expected_configuration")?.clone(),
        ),
        ("recorded_at", string(recorded_at)),
        ("source_path", field(config, "source_path")?.clone()),
        (
            "dependencies",
            field(preview, "expected_dependencies")?.clone(),
        ),
        (
            "source_bindings",
            field(preview, "source_bindings")?.clone(),
        ),
        ("claims", JsonValue::Array(claims)),
        ("files", refs(files)),
        ("grants_admission", JsonValue::Bool(false)),
    ]))
}

fn check_claim_capture(
    ctx: &CommandContext,
    home: &RelativePath,
    files: &BTreeMap<String, Vec<u8>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let raw = files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Conflict("native Claim event absent"))?;
    let event = parse(raw)?;
    if text(field(field(&event, "method")?, "procedure")?, "name")? != "native-claim-serialization"
    {
        return Err(SourceCommandError::Conflict(
            "retained Claim event procedure differs",
        ));
    }
    crate::source_revisions::schema(
        worker,
        deadline,
        cancelled,
        ctx,
        &["ToS/contracts/provenance-event-v2.schema.json".into()],
        "ToS/contracts/provenance-event-v2.schema.json",
        &event,
    )?;
    let decoded: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|_| SourceCommandError::Invalid("native Claim event JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("native Claim event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid("native Claim event semantics"));
    }
    let entities = field(&event, "entities")?;
    let mut observed = BTreeSet::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in array(entities, group)? {
            let name = text(entity, "entity_ref")?
                .strip_prefix(&format!("{}/", home.as_str()))
                .ok_or(SourceCommandError::Conflict("native Claim entity home"))?;
            let bytes = files
                .get(name)
                .ok_or(SourceCommandError::Conflict("native Claim entity absent"))?;
            if !observed.insert(name.to_owned())
                || integer(entity, "size_bytes")? != bytes.len() as u64
                || text(entity, "sha256")? != Digest256::of_bytes(bytes).to_hex()
                || field(entity, "fixity_verified")? != &JsonValue::Bool(false)
            {
                return Err(SourceCommandError::Conflict("native Claim entity bytes"));
            }
        }
    }
    if observed
        != files
            .keys()
            .filter(|name| name.as_str() != "source-create-provenance.jsonl")
            .cloned()
            .collect()
    {
        return Err(SourceCommandError::Conflict(
            "native Claim event output closure",
        ));
    }
    Ok(())
}

/// The actual isolated Claim create/retry entry. A retry reconstructs from
/// retained bytes and the original selected cut; it never reuses an in-memory
/// event as a write grant. The schema child is finalized before filesystem
/// locking or publication on both paths.
pub fn execute_isolated_claim_creation_from_captures(
    filesystem: &crate::source_creation_store::CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    mut current_worker: Option<&mut CutWorkerSchemaExecutor>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    SerializedClaimCreation,
    crate::source_creation_store::CreationPublication,
    JsonValue,
)> {
    let (_, source_path, _, create, _) = config(ctx)?;
    if !create {
        return Err(SourceCommandError::Unsupported(
            "isolated Claim creation requires a creation delegation",
        ));
    }
    let home = path(
        source_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim creation source parent"))?
            .0,
    )?;
    let retained = filesystem.read_claim_retained(ctx, &home, deadline, cancelled)?;
    let mut current = None;
    let serialized = if let Some(retained) = &retained {
        let current_context = CommandContext {
            base_revision: current_cut.current().revision(),
            files: vec![],
            ..ctx.clone()
        };
        ctx.check_from_selected_captures(original_cut, software, components, deadline, cancelled)?;
        let mut current_files =
            complete_authored_inputs(&current_context, current_cut, deadline, cancelled)?;
        current_files.extend(
            ctx.files
                .iter()
                .filter(|file| !file.path.as_str().starts_with("ToS/"))
                .cloned(),
        );
        let current_context = CommandContext {
            files: current_files,
            ..current_context
        };
        if reference_replay_required(&current_context, &parse(&ctx.request_raw)?)? {
            let current_worker =
                current_worker
                    .as_deref_mut()
                    .ok_or(SourceCommandError::Unsupported(
                        "reference Claim replay requires selected current schema worker",
                    ))?;
            if current_worker.source_revision() != current_context.base_revision {
                return Err(SourceCommandError::Conflict(
                    "reference Claim replay worker cut differs",
                ));
            }
            reference_replay_snapshot(
                &current_context,
                &parse(&ctx.configuration_raw)?,
                &parse(&ctx.request_raw)?,
                current_worker,
                deadline,
                cancelled,
            )?;
            crate::source_creation_store::finish_creation_worker(
                current_worker,
                deadline,
                cancelled,
            )?;
        }
        let original = retained_claim_creation_files(
            &current_context,
            &parse(&ctx.configuration_raw)?,
            &parse(&ctx.request_raw)?,
            &source_path,
            retained,
        )?;
        current = Some(current_context);
        restore_claim_creation_from_captures(
            ctx,
            original_cut,
            software,
            components,
            worker,
            &original,
            deadline,
            cancelled,
        )?
    } else {
        serialize_claim_creation_from_captures(
            ctx,
            original_cut,
            software,
            components,
            worker,
            deadline,
            cancelled,
        )?
    };
    crate::source_creation_store::finish_creation_worker(worker, deadline, cancelled)?;
    let publication = if retained.is_some() {
        filesystem.replay_claim_isolated(
            &serialized,
            original_cut,
            current.as_ref().ok_or(SourceCommandError::Conflict(
                "Claim replay current cut absent",
            ))?,
            current_cut,
            software,
            components,
            deadline,
            cancelled,
        )?
    } else {
        filesystem.publish_claim_isolated(
            &serialized,
            original_cut,
            software,
            components,
            deadline,
            cancelled,
        )?
    };
    let response = serialized.command().response.clone();
    Ok((serialized, publication, response))
}

fn reference_replay_required(
    current: &CommandContext,
    request: &JsonValue,
) -> SourceCommandResult<bool> {
    for claim in array(request, "claims")? {
        let (_, descriptor) = profile(current, text(claim, "predicate")?)?;
        if text(&descriptor, "reader")? == "structured-reference-value-v1" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reference_replay_snapshot(
    current: &CommandContext,
    config: &JsonValue,
    request: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    let mut dependencies = Vec::new();
    for claim in array(request, "claims")? {
        let (_, descriptor) = profile(current, text(claim, "predicate")?)?;
        if text(&descriptor, "reader")? == "structured-reference-value-v1" {
            let (_, create, version) = family(text(config, "schema_version")?)?;
            if !create {
                return Err(SourceCommandError::Denied("reference replay owner family"));
            }
            claim_scope(config, claim, true, version)?;
            validate_ground(current, config, claim, version, worker, deadline, cancelled)?;
            dependencies.push(
                maintained_grounding(
                    current,
                    config,
                    std::slice::from_ref(claim),
                    None,
                    true,
                    worker,
                    deadline,
                    cancelled,
                )?
                .dependencies,
            );
        }
    }
    Ok(record_digest(&JsonValue::Array(dependencies))?)
}

fn retained_claim_creation_files(
    current: &CommandContext,
    config: &JsonValue,
    request: &JsonValue,
    source_path: &RelativePath,
    direct: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let home = source_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Claim retained home"))?
        .0;
    let required = BTreeSet::from([
        CLAIM_STREAM.to_owned(),
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
        "source-create-receipt.json".to_owned(),
    ]);
    if !required.iter().all(|name| direct.contains_key(name)) {
        return Err(SourceCommandError::Conflict(
            "Claim original package incomplete",
        ));
    }
    let mut form_targets = BTreeMap::new();
    for claim in array(request, "claims")? {
        form_targets.insert(
            form_name(text(claim, "claim_id")?),
            text(claim, "claim_id")?,
        );
    }
    let mut allowed = required.clone();
    allowed.insert(CLAIM_HISTORY.into());
    for name in form_targets.keys() {
        allowed.insert(name.clone());
        allowed.insert(format!(".{name}.writer.lock"));
    }
    if direct.keys().any(|name| !allowed.contains(name)) {
        return Err(SourceCommandError::Conflict(
            "Claim package contains unrelated member",
        ));
    }
    for (name, raw) in direct {
        if name.ends_with(".writer.lock") {
            if !raw.is_empty() {
                return Err(SourceCommandError::Conflict(
                    "Claim form lock contains data",
                ));
            }
            continue;
        }
        let selected = current.file(&path(&format!("{home}/{name}"))?)?;
        if selected != Some(raw.as_slice()) {
            return Err(SourceCommandError::Conflict(
                "Claim current package differs from selected cut",
            ));
        }
    }
    let current_claims = rows(&direct[CLAIM_STREAM])?;
    for (name, id) in form_targets {
        if let Some(raw) = direct.get(&name) {
            let form = parse(raw)?;
            let subject = metadata_subject(
                current_claims
                    .get(id)
                    .ok_or(SourceCommandError::Conflict("Claim form subject missing"))?,
            )?;
            tos_validation::source_forms::source_copy_kernel::validate_history(&form, &subject)
                .map_err(crate::source_forms::form_error)?;
        }
    }
    let history = verify_claim_history(current, config, source_path, direct)?;
    let archived_or_current = if let Some(first) = array(&history, "receipts")?.first() {
        archive(current, config, first)?
    } else {
        direct.clone()
    };
    let original = required
        .iter()
        .map(|name| {
            Ok((
                name.clone(),
                archived_or_current
                    .get(name)
                    .ok_or(SourceCommandError::Conflict(
                        "archived creation member absent",
                    ))?
                    .clone(),
            ))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    if required
        .iter()
        .any(|name| name.as_str() != CLAIM_STREAM && direct[name] != original[name])
    {
        return Err(SourceCommandError::Conflict(
            "Claim correction rewrote immutable creation evidence",
        ));
    }
    Ok(original)
}

pub(crate) fn complete_authored_inputs(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceFile>> {
    ctx.check()?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "Claim inventory cut differs from command base",
        ));
    }
    const MAX_COMPLETE_BYTES: u64 = 33_554_432;
    let software_inputs = ctx
        .files
        .iter()
        .filter(|input| !input.path.as_str().starts_with("ToS/"))
        .collect::<Vec<_>>();
    let software_bytes = software_inputs
        .iter()
        .try_fold(0u64, |total, input| {
            total.checked_add(input.raw.len() as u64)
        })
        .filter(|total| *total <= MAX_COMPLETE_BYTES)
        .ok_or(SourceCommandError::Unsupported(
            "Claim selected software exceeds complete input byte budget",
        ))?;
    let mut descriptor_bytes = software_bytes;
    let mut actual_bytes = software_bytes;
    let mut files = Vec::new();
    for descriptor in cut.current().members() {
        if !descriptor.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Unsupported(
                "software or other namespace cannot masquerade as authored Claim cut member",
            ));
        }
        let components = descriptor.path.as_str().split('/').collect::<Vec<_>>();
        if descriptor
            .path
            .as_str()
            .starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace in complete Claim inventory",
            ));
        }
        // This is the complete authenticated authored input, not the public
        // metadata inventory. Keep eligible content bytes in the command read
        // closure; maintained_inventory_inner excludes them from its catalog.
        // Native content still requires its separate binding and rights checks.
        if components
            .iter()
            .any(|part| ["payload", "local-content"].contains(part))
        {
            let basename = descriptor.path.as_str().rsplit('/').next().unwrap_or("");
            if NATIVE_CATALOG_KINDS
                .iter()
                .any(|kind| basename == format!("{kind}.json"))
                || basename == CLAIM_STREAM
                || LEGACY_CLAIM_STREAMS.contains(&basename)
                || ["artifact-witness.json", "composite-witness.json"].contains(&basename)
                || basename.starts_with("semantic-annotation") && basename.ends_with(".json")
            {
                return Err(SourceCommandError::Unsupported(
                    "private metadata carrier cannot enter maintained public catalog fingerprint",
                ));
            }
        }
        if files
            .len()
            .checked_add(software_inputs.len())
            .is_none_or(|count| count >= 4096)
        {
            return Err(SourceCommandError::Unsupported(
                "Claim complete inventory exceeds command member budget",
            ));
        }
        let next_descriptor_bytes = descriptor_bytes
            .checked_add(descriptor.size_bytes)
            .filter(|total| *total <= MAX_COMPLETE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "Claim complete inventory exceeds aggregate descriptor byte budget",
            ))?;
        let member = cut
            .read_member(
                ctx.base_revision,
                &descriptor.path,
                8_388_608u64.min(MAX_COMPLETE_BYTES - actual_bytes),
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Claim inventory member read refused"))?;
        let raw_bytes = member.raw.len() as u64;
        let next_actual_bytes = actual_bytes
            .checked_add(raw_bytes)
            .filter(|total| *total <= MAX_COMPLETE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "Claim complete inventory exceeds aggregate raw byte budget",
            ))?;
        if raw_bytes != descriptor.size_bytes {
            return Err(SourceCommandError::Conflict(
                "Claim inventory raw size differs from descriptor",
            ));
        }
        descriptor_bytes = next_descriptor_bytes;
        actual_bytes = next_actual_bytes;
        files.push(SourceFile {
            path: member.path,
            raw: member.raw,
        });
    }
    for selected in ctx
        .files
        .iter()
        .filter(|input| input.path.as_str().starts_with("ToS/"))
    {
        if files
            .iter()
            .find(|file| file.path == selected.path)
            .map(|file| &file.raw)
            != Some(&selected.raw)
        {
            return Err(SourceCommandError::Conflict(
                "selected Claim input differs from complete cut",
            ));
        }
    }
    Ok(files)
}

fn run_claim_command_inner(
    ctx: &CommandContext,
    selected_context: Option<&CommandContext>,
    serialize_creation: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    // Planning must bind the complete read closure as well as caller inputs.
    // A partial caller context cannot be used to bind this proposal afterward.
    if selected_context.is_some_and(|selected| selected.files.len() != ctx.files.len()) {
        return Err(SourceCommandError::Unsupported(
            "Claim proposal requires complete cut members in command context",
        ));
    }
    let (config, p, handler, create, version) = config(ctx)?;
    if executor.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "Claim schema worker cut differs from command base",
        ));
    }

    let request = parse(&ctx.request_raw)?;
    grammar(&request, create, version == 7)?;
    let operation = text(&request, "operation")?;
    if !["describe", "inspect-version"].contains(&operation) && selected_context.is_none() {
        return Err(SourceCommandError::Unsupported(
            "maintained Claim grounding requires authenticated complete source cut inventory",
        ));
    }
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
            "supported_operations",
            JsonValue::Array(vec![string("claims.create")]),
        )?;
        set(
            &mut response,
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare-create", "claims.create"]
                    .into_iter()
                    .map(string)
                    .collect(),
            ),
        )?;
        set(&mut response, "expected_revision", JsonValue::Null)?;
        set(
            &mut response,
            "creation_provenance_event_id",
            field(&config, "provenance_event_id")?.clone(),
        )?;
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
                || claim
                    .object_get("supersedes_claim_ref")
                    .is_some_and(|value| value != &JsonValue::Null)
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
        let grounding = maintained_grounding(
            ctx, &config, claims, None, false, executor, deadline, cancelled,
        )?;
        set(
            &mut response,
            "expected_dependencies",
            grounding.dependencies.clone(),
        )?;
        set(&mut response, "source_bindings", grounding.bindings)?;
        set(
            &mut response,
            "prepared_files",
            refs(&BTreeMap::from([(CLAIM_STREAM.into(), raw.clone())])),
        )?;
        if operation == "claims.create" {
            if text(&request, "expected_configuration")? != digest
                || field(&request, "expected_revision")? != &JsonValue::Null
                || !same(
                    field(&request, "expected_dependencies")?,
                    field(&response, "expected_dependencies")?,
                )?
                || !same(
                    field(&request, "expected_inputs")?,
                    field(&response, "source_bindings")?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "prepared Claim creation delegation or source snapshot changed",
                ));
            }
            if !serialize_creation {
                return Err(SourceCommandError::Unsupported(
                    "Claim creation requires native serialization capture",
                ));
            }
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
                set(
                    &mut response,
                    "files",
                    archive_locations(&archived, text(receipt, "archive_path")?),
                )?;
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
    let reason = stripped(text(&request, "reason")?)?;
    if reason.is_empty() || reason.chars().count() > 4096 {
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
    let grounding = maintained_grounding(
        ctx,
        &config,
        std::slice::from_ref(&revised),
        Some(forms),
        true,
        executor,
        deadline,
        cancelled,
    )?;
    set(
        &mut response,
        "expected_dependencies",
        grounding.dependencies.clone(),
    )?;
    set(&mut response, "source_bindings", grounding.bindings.clone())?;
    if operation == "claim.revise" {
        if !same(field(&request, "expected_configuration")?, &string(&digest))?
            || !same(
                field(&request, "expected_source")?,
                &metadata_subject(record)?,
            )?
            || !same(field(&request, "expected_revision")?, &revision(&files)?)?
            || !same(
                field(&request, "expected_dependencies")?,
                &grounding.dependencies,
            )?
            || !same(field(&request, "expected_inputs")?, &grounding.bindings)?
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
            ("dependencies", grounding.dependencies),
            ("source_bindings", grounding.bindings),
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
            ("files", archive_refs(&files)),
        ]);
        let mut output = Vec::new();
        let mut archived_blobs = BTreeSet::new();
        for raw in files.values() {
            let name = blob_name(raw);
            if !archived_blobs.insert(name.clone()) {
                continue;
            }
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
/// Fixed maintained rule-contract inputs, not executed producers or issuers.
pub const CLAIM_GROUNDING_RULE_INPUTS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_claim_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_record_profiles.py",
    "scripts/source_identity_proposals.py",
    "scripts/source_document_catalogue.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_version_reader.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/build_source_witness_catalog.py",
    "scripts/source_witness_bibliographic_graph_common.py",
];
pub const CLAIM_REVISION_RULE_INPUTS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
    "scripts/source_witness_human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
pub(crate) const NATIVE_CATALOG_KINDS: &[&str] = &[
    "agent",
    "place",
    "organization",
    "work",
    "expression",
    "edition",
    "collection",
    "item",
    "link",
];
const CATALOG_LINK_FIELDS: &[&str] = &[
    "work_ref",
    "expression_claim_refs",
    "responsibility_claim_refs",
    "chronology_claim_refs",
    "embodiment_claim_refs",
    "derivation_claim_refs",
    "embodies_expression_refs",
    "publication_claim_refs",
    "provision_activity_claim_refs",
    "exemplar_claim_refs",
    "collection_ref",
    "membership_claim_refs",
    "item_manifest_ref",
    "association_claim_refs",
];
const LEGACY_CLAIM_STREAMS: &[&str] = &[
    "membership-claims.jsonl",
    "responsibility-claims.jsonl",
    "publication-claims.jsonl",
    "provision-activity-claims.jsonl",
    "work-chronology-claims.jsonl",
    "work-expression-claims.jsonl",
    "expression-edition-claims.jsonl",
    "edition-item-claims.jsonl",
    "expression-derivation-claims.jsonl",
    "object-link-claims.jsonl",
    "historical-claims.jsonl",
];
struct ClaimGrounding {
    dependencies: JsonValue,
    bindings: JsonValue,
}
pub(crate) fn raw_digests(
    ctx: &CommandContext,
    refs: &[&str],
    prefixed: bool,
) -> SourceCommandResult<JsonValue> {
    let mut result = object(vec![]);
    for name in refs {
        let raw = ctx
            .file(&path(name)?)?
            .ok_or(SourceCommandError::Unsupported(
                "fixed maintained Claim rule input absent from selected authored/software carrier",
            ))?;
        let digest = Digest256::of_bytes(raw);
        set(
            &mut result,
            name,
            string(&if prefixed {
                digest.to_prefixed()
            } else {
                digest.to_hex()
            }),
        )?;
    }
    Ok(result)
}
fn include_schema(
    ctx: &CommandContext,
    inputs: &mut JsonValue,
    name: &str,
) -> SourceCommandResult<()> {
    set(
        inputs,
        name,
        string(&Digest256::of_bytes(selected(ctx, name)?).to_hex()),
    )
}
fn include_route(
    ctx: &CommandContext,
    inputs: &mut JsonValue,
    route: &JsonValue,
    extras: &[&str],
) -> SourceCommandResult<()> {
    for name in extras {
        include_schema(ctx, inputs, name)?;
    }
    for name in array(route, "schema_dependencies")? {
        include_schema(
            ctx,
            inputs,
            name.as_str()
                .ok_or(SourceCommandError::Invalid("schema dependency"))?,
        )?;
    }
    include_schema(ctx, inputs, text(route, "schema_ref")?)
}
fn claim_profile_inputs(
    ctx: &CommandContext,
    claim: &JsonValue,
    inputs: &mut JsonValue,
) -> SourceCommandResult<&'static str> {
    let (_, descriptor) = profile(ctx, text(claim, "predicate")?)?;
    let reader = text(&descriptor, "reader")?;
    if reader.starts_with("identity-transition-")
        || descriptor
            .object_get("object_reference_set")
            .and_then(|v| v.object_get("basis_adapter"))
            .is_some()
    {
        return Err(SourceCommandError::Unsupported(
            "maintained exact retained identity/order provenance bindings adapter",
        ));
    }
    let route = array(&descriptor, "schemas")?
        .iter()
        .find(|v| v.object_get("schema_version") == claim.object_get("schema_version"))
        .ok_or(SourceCommandError::Unsupported(
            "Claim schema fingerprint route",
        ))?;
    include_route(
        ctx,
        inputs,
        route,
        &[
            "ToS/contracts/claim-packet.schema.json",
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/source-claim-record.schema.json",
        ],
    )?;
    let temporal = ["historical-temporal-v1", "document-catalogue-temporal-v1"].contains(&reader);
    let structured = ["structured-value-v1", "structured-reference-value-v1"].contains(&reader);
    if temporal {
        include_schema(ctx, inputs, "ToS/contracts/historical-claim.schema.json")?;
    }
    if structured {
        for name in [
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/source-structured-value.schema.json",
        ] {
            include_schema(ctx, inputs, name)?;
        }
    }
    if descriptor
        .object_get("object_reference_set")
        .and_then(|v| v.object_get("structure_adapter"))
        .and_then(JsonValue::as_str)
        == Some("scoped-members-v1")
    {
        include_schema(
            ctx,
            inputs,
            "ToS/contracts/scoped-member-structure.schema.json",
        )?;
    }
    if claim
        .object_get("qualifiers")
        .and_then(|v| v.object_get("display_fields"))
        .and_then(|v| v.object_get("schema_version"))
        .and_then(JsonValue::as_str)
        == Some("tos_claim_display_fields_v1")
    {
        for name in [
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/claim-display-fields.schema.json",
        ] {
            include_schema(ctx, inputs, name)?;
        }
    }
    Ok(if temporal {
        "temporal"
    } else if structured {
        "structured"
    } else {
        "identity"
    })
}
fn catalogue_record(
    record: &JsonValue,
    location: &str,
    schema: Option<&str>,
) -> SourceCommandResult<JsonValue> {
    let mut result = object(vec![
        (
            "schema_version",
            string("tos_source_witness_catalog_entry_v1"),
        ),
        ("record_id", field(record, "record_id")?.clone()),
        ("record_type", field(record, "record_type")?.clone()),
        (
            "preferred_label",
            record
                .object_get("preferred_label")
                .cloned()
                .unwrap_or_else(|| string("")),
        ),
        (
            "identity_status",
            record
                .object_get("identity_status")
                .cloned()
                .unwrap_or_else(|| string("")),
        ),
        ("source_record_ref", string(location)),
        ("record_sha256", string(&record_digest(record)?.to_hex())),
    ]);
    if let Some(schema) = schema {
        set(&mut result, "source_schema_ref", string(schema))?;
    }
    let mut links = object(vec![]);
    for key in CATALOG_LINK_FIELDS {
        if let Some(value) = record.object_get(key) {
            set(&mut links, key, value.clone())?;
        }
    }
    set(&mut result, "links", links)?;
    Ok(result)
}
fn catalogue_claim(
    ctx: &CommandContext,
    claim: &JsonValue,
    location: &str,
    line: usize,
    profiled: bool,
) -> SourceCommandResult<JsonValue> {
    let mut result = object(vec![
        (
            "schema_version",
            string("tos_source_witness_claim_catalog_entry_v1"),
        ),
        ("source_claim_file_ref", string(location)),
        ("source_claim_line", number(line as u64)),
        ("claim_sha256", string(&record_digest(claim)?.to_hex())),
    ]);
    for key in [
        "claim_id",
        "claim_type",
        "assertion_layer",
        "subject_ref",
        "predicate",
        "object",
        "evidence_refs",
        "maker",
        "provenance_event_ref",
        "epistemic_status",
        "review_status",
        "visibility",
        "claim_version",
    ] {
        set(
            &mut result,
            key,
            claim.object_get(key).cloned().unwrap_or(JsonValue::Null),
        )?;
    }
    let reviews = claim
        .object_get("reviews")
        .map(|value| {
            value
                .as_array()
                .ok_or(SourceCommandError::Invalid("catalog Claim reviews array"))
        })
        .transpose()?
        .unwrap_or(&[])
        .iter()
        .filter_map(|v| v.object_get("review_id"))
        .filter(|v| v.as_str().is_some())
        .cloned()
        .collect();
    set(&mut result, "review_refs", JsonValue::Array(reviews))?;
    for key in ["supersedes_claim_ref", "qualifiers"] {
        if let Some(v) = claim.object_get(key) {
            set(&mut result, key, v.clone())?;
        }
    }
    if text(claim, "schema_version")? == "tos_historical_claim_v1" {
        set(
            &mut result,
            "source_schema_ref",
            string("ToS/contracts/historical-claim.schema.json"),
        )?;
    }
    if profiled {
        let (_, descriptor) = profile(ctx, text(claim, "predicate")?)?;
        let route = array(&descriptor, "schemas")?
            .iter()
            .find(|v| v.object_get("schema_version") == claim.object_get("schema_version"))
            .ok_or(SourceCommandError::Unsupported(
                "Claim catalogue schema route",
            ))?;
        set(
            &mut result,
            "source_schema_ref",
            field(route, "schema_ref")?.clone(),
        )?;
    }
    Ok(result)
}

/// Exact metadata transport for the shared native resolver. The cut supplies
/// membership and secure current reads; selected context bytes must match it.
struct ProfileMetadataReader<'a> {
    ctx: &'a CommandContext,
    cut: &'a CorpusCutReader,
    observed: BTreeMap<String, (Digest256, usize)>,
    observed_bytes: usize,
}
impl ProfileMetadataReader<'_> {
    fn public_path(&self, name: &str) -> SourceCommandResult<RelativePath> {
        let parsed = path(name)?;
        if !name.starts_with("ToS/")
            || name.split('/').any(|part| {
                matches!(
                    part,
                    "owner-local" | "catalog" | "payload" | "local-content"
                )
            })
        {
            return Err(SourceCommandError::Denied(
                "native profile public metadata namespace",
            ));
        }
        Ok(parsed)
    }
    fn current_raw(
        &self,
        name: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if self.cut.current().revision() != self.ctx.base_revision {
            return Err(SourceCommandError::Conflict(
                "native profile source cut changed",
            ));
        }
        let parsed = self.public_path(name)?;
        let expected = selected(self.ctx, name)?;
        let descriptor = self
            .cut
            .current()
            .member(&parsed)
            .ok_or(SourceCommandError::Conflict(
                "native profile member missing",
            ))?;
        if descriptor.size_bytes > cap as u64
            || expected.len() > cap
            || descriptor.size_bytes != expected.len() as u64
            || descriptor.sha256 != Digest256::of_bytes(expected)
        {
            return Err(SourceCommandError::Conflict(
                "native profile selected metadata descriptor mismatch",
            ));
        }
        let member = self
            .cut
            .read_member(
                self.ctx.base_revision,
                &parsed,
                cap as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Conflict("native profile secure current read refused")
            })?;
        if member.raw != expected {
            return Err(SourceCommandError::Conflict(
                "native profile selected metadata bytes changed",
            ));
        }
        Ok(member.raw)
    }
}
impl SignNativeRead for ProfileMetadataReader<'_> {
    fn read(
        &mut self,
        name: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let prefix = match kind {
            NativeReadKind::Schema => "ToS/contracts/",
            NativeReadKind::Support => "ToS/",
            NativeReadKind::Metadata => "ToS/source-witnesses/",
            NativeReadKind::Content => {
                return Err(SourceCommandError::Denied(
                    "profile resolver cannot read content",
                ));
            }
        };
        if !name.starts_with(prefix) {
            return Err(SourceCommandError::Denied(
                "native profile input kind namespace",
            ));
        }
        let prior = self.observed.get(name);
        if prior.is_none() && self.observed.len() >= 128 {
            return Err(SourceCommandError::Invalid(
                "native profile shared input count",
            ));
        }
        let cap = max_bytes.min(1_048_576).min(if prior.is_some() {
            8_388_608
        } else {
            8_388_608 - self.observed_bytes
        });
        let raw = self.current_raw(name, cap, deadline, cancelled)?;
        let digest = Digest256::of_bytes(&raw);
        if let Some((expected, length)) = prior {
            if *expected != digest || *length != raw.len() {
                return Err(SourceCommandError::Conflict(
                    "native profile shared read changed",
                ));
            }
        } else {
            self.observed_bytes = self
                .observed_bytes
                .checked_add(raw.len())
                .filter(|total| *total <= 8_388_608)
                .ok_or(SourceCommandError::Invalid(
                    "native profile shared metadata byte budget",
                ))?;
            self.observed.insert(name.into(), (digest, raw.len()));
        }
        Ok(raw)
    }
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(SourceCommandError::Invalid(
                "native profile read deadline or cancellation",
            ));
        }
        for (name, (digest, length)) in &self.observed {
            let raw = self.current_raw(name, *length, deadline, cancelled)?;
            if Digest256::of_bytes(&raw) != *digest {
                return Err(SourceCommandError::Conflict(
                    "native profile observed input changed",
                ));
            }
        }
        Ok(())
    }
    fn owner_local(&self, name: &str) -> SourceCommandResult<bool> {
        // Content locators are declarations in MetadataOnly scope, including
        // payload/local-content locators absent from the metadata cut. Native
        // owns their content route; only actual metadata reads use public_path.
        path(name)?;
        if !name.starts_with("ToS/") {
            return Err(SourceCommandError::Denied("native profile owner namespace"));
        }
        Ok(name == "ToS/source-witnesses/owner-local"
            || name.starts_with("ToS/source-witnesses/owner-local/"))
    }
}

/// Existing maintained selected-cut catalog mechanics, shared by Claim and
/// source-create handlers. These observations carry no permission or admission.
pub(crate) struct MaintainedInventory {
    pub records: JsonValue,
    pub record_inputs: JsonValue,
    pub record_member_inputs: BTreeMap<String, JsonValue>,
    pub objects: BTreeMap<String, JsonValue>,
    pub source_records: BTreeMap<String, JsonValue>,
    pub claims: BTreeMap<String, JsonValue>,
    pub claim_profile_inputs: JsonValue,
    pub events: JsonValue,
    pub anchors: JsonValue,
    pub native_identity_snapshot: Option<String>,
    pub native_text_snapshot: Option<String>,
}
pub(crate) fn maintained_inventory(
    ctx: &CommandContext,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<MaintainedInventory> {
    maintained_inventory_inner(ctx, None, false, executor, deadline, cancelled)
}

/// Complete authored membership comes from the actual cut, independently of
/// selected software inputs. Native observations remain metadata-only.
pub(crate) fn maintained_inventory_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<MaintainedInventory> {
    let mut files = complete_authored_inputs(ctx, cut, deadline, cancelled)?;
    files.extend(
        ctx.files
            .iter()
            .filter(|f| !f.path.as_str().starts_with("ToS/"))
            .cloned(),
    );
    let complete = CommandContext {
        files,
        ..ctx.clone()
    };
    complete.check()?;
    maintained_inventory_inner(&complete, Some(cut), false, executor, deadline, cancelled)
}

/// Only the privately verified complete managed Agent input may use this route.
/// Native packet/profile/Claim scopes remain unported rather than inheriting
/// the schema cut's authored completeness.
pub(crate) fn maintained_agent_inventory_from_managed(
    ctx: &CommandContext,
    capture_member_inputs: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<MaintainedInventory> {
    for file in &ctx.files {
        let path = file.path.as_str();
        let name = path.rsplit('/').next().unwrap_or(path);
        if path.starts_with("ToS/source-witnesses/")
            && !path
                .split('/')
                .any(|p| ["catalog", "payload", "local-content", ".record-revisions"].contains(&p))
            && (name.starts_with("semantic-annotation") && name.ends_with(".json")
                || name == CLAIM_STREAM
                || LEGACY_CLAIM_STREAMS.contains(&name)
                || name == "historical-claims.jsonl")
        {
            return Err(SourceCommandError::Unsupported(
                "managed Agent inventory native/Claim scope requires owner reader",
            ));
        }
    }
    let inventory = maintained_inventory_inner(
        ctx,
        None,
        capture_member_inputs,
        executor,
        deadline,
        cancelled,
    )?;
    complete_agent_inventory(inventory)
}

fn complete_agent_inventory(
    mut inventory: MaintainedInventory,
) -> SourceCommandResult<MaintainedInventory> {
    if !inventory.claims.is_empty()
        || inventory
            .objects
            .values()
            .any(|entry| text(entry, "record_type").ok() != Some("agent"))
    {
        return Err(SourceCommandError::Unsupported(
            "managed Agent inventory has unported record scope",
        ));
    }
    // Same maintained empty native-packet map encoding, after complete
    // controlled membership has proved absence, never a caller sentinel.
    inventory.native_identity_snapshot = Some(crate::source_revisions::python_ascii_digest(
        &object(vec![]),
    )?);
    Ok(inventory)
}

#[derive(Default)]
struct AgentInventoryMember<'a> {
    records: BTreeMap<&'a str, Vec<&'a JsonValue>>,
    events: Vec<(&'a JsonString, &'a JsonValue)>,
    anchors: Vec<(&'a JsonString, &'a JsonValue)>,
}

/// Operation-local borrowed grouping of the already extracted inventory.
/// No body is parsed again or copied to a second inventory.
pub(crate) struct AgentInventoryMembers<'a> {
    files: BTreeMap<&'a str, &'a SourceFile>,
    members: BTreeMap<&'a str, AgentInventoryMember<'a>>,
    base_profiles: JsonValue,
}
pub(crate) fn agent_inventory_members<'a>(
    ctx: &'a CommandContext,
    inventory: &'a MaintainedInventory,
) -> SourceCommandResult<AgentInventoryMembers<'a>> {
    ctx.check()?;
    let mut grouped = AgentInventoryMembers {
        files: ctx
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect(),
        members: BTreeMap::new(),
        base_profiles: raw_digests(
            ctx,
            &[
                ENTITIES,
                "ToS/contracts/semantic-entity-type-registry.schema.json",
            ],
            false,
        )?,
    };
    for (kind, entries) in inventory
        .records
        .as_object()
        .ok_or(SourceCommandError::Invalid("Agent inventory record map"))?
    {
        let kind = kind
            .as_str()
            .ok_or(SourceCommandError::Invalid("Agent inventory kind"))?;
        for entry in entries
            .as_array()
            .ok_or(SourceCommandError::Invalid("Agent catalogue array"))?
        {
            grouped
                .members
                .entry(text(entry, "source_record_ref")?)
                .or_default()
                .records
                .entry(kind)
                .or_default()
                .push(entry);
        }
    }
    for (entries, anchors) in [(&inventory.events, false), (&inventory.anchors, true)] {
        for (key, entry) in entries
            .as_object()
            .ok_or(SourceCommandError::Invalid("Agent evidence map"))?
        {
            let member = grouped
                .members
                .entry(text(entry, "source_ref")?)
                .or_default();
            if anchors {
                member.anchors.push((key, entry));
            } else {
                member.events.push((key, entry));
            }
        }
    }
    Ok(grouped)
}

/// One actual member's maintained inventory contribution. The same extractor
/// serves full inventory and this controlled Agent projection; no source body
/// or permission is retained in the projection.
pub(crate) fn agent_inventory_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
    inventory: &MaintainedInventory,
    grouped: &AgentInventoryMembers<'_>,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    if !file.path.as_str().starts_with("ToS/") {
        return Err(SourceCommandError::Invalid(
            "Agent projection authored namespace",
        ));
    }
    let location = file.path.as_str();
    if grouped
        .files
        .get(location)
        .copied()
        .is_none_or(|selected| !std::ptr::eq(selected, file))
    {
        return Err(SourceCommandError::Conflict(
            "Agent contribution differs from selected bytes",
        ));
    }
    if location
        .split('/')
        .any(|part| ["owner-local", "payload", "local-content"].contains(&part))
    {
        return Err(SourceCommandError::Unsupported(
            "Agent projection requires private/native reader",
        ));
    }
    let basename = location.rsplit('/').next().unwrap_or(location);
    // These scopes require their actual owner readers. Never infer their
    // absence from an empty Agent catalogue contribution.
    if location.starts_with("ToS/source-witnesses/")
        && !location.split('/').any(|part| {
            ["catalog", "payload", "local-content", ".record-revisions"].contains(&part)
        })
        && (basename.starts_with("semantic-annotation") && basename.ends_with(".json")
            || basename == CLAIM_STREAM
            || LEGACY_CLAIM_STREAMS.contains(&basename)
            || basename == "historical-claims.jsonl")
    {
        return Err(SourceCommandError::Unsupported(
            "Agent projection requires Claim/native owner",
        ));
    }
    let member = grouped.members.get(location);
    let mut records = object(vec![]);
    for kind in NATIVE_CATALOG_KINDS {
        set(
            &mut records,
            kind,
            JsonValue::Array(
                member
                    .and_then(|member| member.records.get(kind))
                    .into_iter()
                    .flatten()
                    .map(|entry| (**entry).clone())
                    .collect(),
            ),
        )?;
    }
    let evidence = |anchors: bool| -> JsonValue {
        JsonValue::Object(
            member
                .into_iter()
                .flat_map(|member| {
                    if anchors {
                        member.anchors.iter()
                    } else {
                        member.events.iter()
                    }
                })
                .map(|(key, entry)| ((**key).clone(), (**entry).clone()))
                .collect(),
        )
    };
    let form = if location.starts_with("ToS/source-witnesses/")
        && basename.ends_with(".human-forms.json")
    {
        let prior = parse(&file.raw)?;
        crate::source_forms::apply_form_changes(Some(&prior), field(&prior, "subject")?, &[])?;
        let parent = format!(
            "{}.json",
            location.strip_suffix(".human-forms.json").unwrap()
        );
        if parent.ends_with("/agent.json") && grouped.files.contains_key(parent.as_str()) {
            crate::source_revisions::schema(
                executor,
                deadline,
                cancelled,
                ctx,
                &["ToS/contracts/human-form-set.schema.json".into()],
                "ToS/contracts/human-form-set.schema.json",
                &prior,
            )?;
        }
        object(vec![
            (
                "raw_sha256",
                string(&Digest256::of_bytes(&file.raw).to_prefixed()),
            ),
            (
                "form_ids",
                JsonValue::Array(
                    ["forms", "prior_forms"]
                        .into_iter()
                        .map(|section| array(&prior, section))
                        .collect::<SourceCommandResult<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .map(|form| field(form, "form_id").cloned())
                        .collect::<SourceCommandResult<Vec<_>>>()?,
                ),
            ),
        ])
    } else {
        JsonValue::Null
    };
    Ok(object(vec![
        (
            "schema_version",
            string("tos_managed_agent_inventory_member_v1"),
        ),
        ("path", string(location)),
        (
            "raw_sha256",
            string(&Digest256::of_bytes(&file.raw).to_hex()),
        ),
        ("records", records),
        (
            "source_profiles",
            inventory
                .record_member_inputs
                .get(location)
                .cloned()
                .unwrap_or_else(|| grouped.base_profiles.clone()),
        ),
        ("events", evidence(false)),
        ("anchors", evidence(true)),
        ("form", form),
    ]))
}

fn maintained_inventory_inner(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    capture_member_inputs: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<MaintainedInventory> {
    let entities = crate::source_revisions::validate_source_profile_registry(
        executor, deadline, cancelled, ctx,
    )?;
    let types = array(&entities, "types")?;
    // collect_records seeds its collision universe before reading metadata.
    // Even an empty snapshot is observed only by the cut-backed owner reader.
    let (native_identities, native_identity_snapshot, native_schema_used) = if let Some(cut) = cut {
        let (identities, snapshot, schema_used) =
            crate::source_revisions::native_identity_inventory_from_cut(
                ctx, cut, executor, deadline, cancelled,
            )?;
        (identities, Some(snapshot), schema_used)
    } else {
        (BTreeMap::new(), None, false)
    };
    let mut native_bindings = Vec::new();
    let mut kinds: BTreeMap<String, String> = NATIVE_CATALOG_KINDS
        .iter()
        .map(|kind| ((*kind).into(), format!("{kind}.json")))
        .collect();
    for entity in types {
        if let Some(profile) = entity.object_get("source_record_profile") {
            kinds.insert(
                text(profile, "record_type")?.into(),
                text(profile, "source_basename")?.into(),
            );
        }
    }
    let registry_refs = [
        ENTITIES,
        "ToS/contracts/semantic-entity-type-registry.schema.json",
    ];
    let claim_registry_refs = [
        ENTITIES,
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        RELATIONS,
        "ToS/contracts/semantic-relation-type-registry.schema.json",
    ];
    let mut record_inputs = raw_digests(ctx, &registry_refs, false)?;
    let mut record_member_inputs = BTreeMap::new();
    if native_schema_used {
        let name = "ToS/contracts/semantic-annotation-packet-v2.schema.json";
        set(
            &mut record_inputs,
            name,
            string(&Digest256::of_bytes(selected(ctx, name)?).to_hex()),
        )?;
    }
    let mut prior_profile_inputs = raw_digests(ctx, &claim_registry_refs, false)?;
    let mut records: BTreeMap<String, Vec<JsonValue>> = NATIVE_CATALOG_KINDS
        .iter()
        .map(|kind| ((*kind).into(), Vec::new()))
        .collect();
    let mut objects = BTreeMap::new();
    let mut source_records = BTreeMap::new();
    let mut prior = BTreeMap::new();
    let mut events = object(vec![]);
    let mut anchors = object(vec![]);
    let mut inventory = ctx.files.iter().collect::<Vec<_>>();
    inventory.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    let mut has_profiled_claim = false;
    for file in &inventory {
        let location = file.path.as_str();
        if !location.starts_with("ToS/source-witnesses/")
            || location.split('/').any(|part| {
                ["catalog", "payload", "local-content", ".record-revisions"].contains(&part)
            })
        {
            continue;
        }
        let basename = location.rsplit('/').next().unwrap_or("");
        if cut.is_none()
            && basename.starts_with("semantic-annotation")
            && basename.ends_with(".json")
        {
            return Err(SourceCommandError::Unsupported(
                "maintained native semantic identity snapshot adapter",
            ));
        }
        if ["artifact-witness.json", "composite-witness.json"].contains(&basename) {
            return Err(SourceCommandError::Unsupported(
                "maintained native artifact/composite catalog fingerprint adapter",
            ));
        }
        if let Some((kind, _)) = kinds.iter().find(|(_, name)| name.as_str() == basename) {
            let record = parse(&file.raw)?;
            if text(&record, "record_type")? != kind {
                return Err(SourceCommandError::Conflict(
                    "catalog record kind differs from basename",
                ));
            }
            let id = text(&record, "record_id")?.to_owned();
            if id.is_empty() {
                return Err(SourceCommandError::Invalid("catalog metadata identity"));
            }
            if native_identities.contains_key(&id) {
                return Err(SourceCommandError::Conflict(
                    "catalog metadata identity is occupied by native semantic packet",
                ));
            }
            let descriptor = types.iter().find_map(|entity| {
                entity
                    .object_get("source_record_profile")
                    .filter(|profile| {
                        profile
                            .object_get("record_type")
                            .and_then(JsonValue::as_str)
                            == Some(kind.as_str())
                    })
            });
            let mut member_inputs = if capture_member_inputs {
                Some(raw_digests(ctx, &registry_refs, false)?)
            } else {
                None
            };
            let schema = if let Some(descriptor) = descriptor {
                if cut.is_none()
                    && (descriptor.object_get("native_binding_adapter").is_some()
                        || record.object_get("native_text_binding").is_some())
                {
                    return Err(SourceCommandError::Unsupported(
                        "maintained native text binding snapshot adapter",
                    ));
                }
                if descriptor.object_get("native_binding_adapter").is_some() {
                    native_bindings.push(field(&record, "native_text_binding")?.clone());
                }
                let route = array(descriptor, "schemas")?
                    .iter()
                    .find(|v| v.object_get("schema_version") == record.object_get("schema_version"))
                    .ok_or(SourceCommandError::Unsupported(
                        "catalog metadata schema route",
                    ))?;
                include_route(
                    ctx,
                    &mut record_inputs,
                    route,
                    &["ToS/contracts/corpus-record.schema.json"],
                )?;
                if let Some(inputs) = &mut member_inputs {
                    include_route(
                        ctx,
                        inputs,
                        route,
                        &["ToS/contracts/corpus-record.schema.json"],
                    )?;
                }
                let exact = metadata_subject(&record)?;
                let (verified, locator) = if let Some(cut) = cut {
                    crate::source_revisions::resolve_record_version_from_cut(
                        ctx, cut, &exact, executor, deadline, cancelled,
                    )?
                } else {
                    crate::source_revisions::resolve_record_version(
                        ctx, &exact, executor, deadline, cancelled,
                    )?
                };
                if !same(&verified, &record)? || locator != location {
                    return Err(SourceCommandError::Conflict(
                        "catalog profile source owner drift",
                    ));
                }
                Some(text(route, "schema_ref")?)
            } else {
                None
            };
            let entry = catalogue_record(&record, location, schema)?;
            if objects.insert(id.clone(), entry.clone()).is_some() {
                return Err(SourceCommandError::Conflict(
                    "complete catalog has duplicate metadata identity",
                ));
            }
            if let Some(inputs) = member_inputs {
                record_member_inputs.insert(location.to_owned(), inputs);
            }
            source_records.insert(id, record);
            records.entry(kind.clone()).or_default().push(entry);
        }
        if basename == CLAIM_STREAM || LEGACY_CLAIM_STREAMS.contains(&basename) {
            let profiled = basename == CLAIM_STREAM;
            has_profiled_claim |= profiled;
            if !profiled
                && std::str::from_utf8(&file.raw)
                    .map_err(|_| SourceCommandError::Invalid("legacy Claim carrier UTF-8"))?
                    .chars()
                    .any(|character| {
                        matches!(
                            character,
                            '\u{b}' | '\u{c}' | '\u{1c}'
                                ..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                        )
                    })
            {
                return Err(SourceCommandError::Unsupported(
                    "legacy Claim Unicode splitlines carrier adapter",
                ));
            }
            // The source JSONL carrier preserves physical line numbers even
            // when a line contains only source-authorized whitespace.
            for (index, raw) in file.raw.split(|byte| *byte == b'\n').enumerate() {
                let raw_text = std::str::from_utf8(raw)
                    .map_err(|_| SourceCommandError::Invalid("catalog Claim UTF-8"))?;
                if if profiled {
                    python_bytes_blank(raw)
                } else {
                    stripped(raw_text)?.is_empty()
                } {
                    continue;
                }
                let claim = parse(raw)?;
                if !["public", "public_metadata_only"].contains(&text(&claim, "visibility")?) {
                    return Err(SourceCommandError::Denied("catalog Claim visibility"));
                }
                if profiled {
                    claim_profile_inputs(ctx, &claim, &mut prior_profile_inputs)?;
                    let (_, descriptor) = profile(ctx, text(&claim, "predicate")?)?;
                    let route = array(&descriptor, "schemas")?
                        .iter()
                        .find(|v| {
                            v.object_get("schema_version") == claim.object_get("schema_version")
                        })
                        .ok_or(SourceCommandError::Unsupported(
                            "existing Claim schema route",
                        ))?;
                    schema_check(
                        executor,
                        location,
                        &claim,
                        text(route, "schema_ref")?,
                        deadline,
                        cancelled,
                    )?;
                    schema_check(
                        executor,
                        location,
                        &claim,
                        "ToS/contracts/source-claim-record.schema.json",
                        deadline,
                        cancelled,
                    )?;
                }
                let id = text(&claim, "claim_id")?.to_owned();
                if id.is_empty() {
                    return Err(SourceCommandError::Invalid("catalog Claim identity"));
                }
                if prior
                    .insert(
                        id,
                        catalogue_claim(ctx, &claim, location, index + 1, profiled)?,
                    )
                    .is_some()
                {
                    return Err(SourceCommandError::Conflict(
                        "complete catalog has duplicate Claim identity",
                    ));
                }
            }
        }
        if basename.ends_with(".jsonl")
            && (basename.contains("provenance") || basename.contains("anchor"))
        {
            let key = if basename.contains("provenance") {
                "event_id"
            } else {
                "anchor_id"
            };
            let indexed = if key == "event_id" {
                &mut events
            } else {
                &mut anchors
            };
            for (index, raw) in file.raw.split(|byte| *byte == b'\n').enumerate() {
                if stripped(
                    std::str::from_utf8(raw)
                        .map_err(|_| SourceCommandError::Invalid("evidence index UTF-8"))?,
                )?
                .is_empty()
                {
                    continue;
                }
                let payload = parse(raw)?;
                let Some(id) = payload.object_get(key) else {
                    continue;
                };
                if id == &JsonValue::Null {
                    continue;
                }
                let id = id
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or(SourceCommandError::Invalid("indexed evidence identity"))?;
                if indexed.object_get(id).is_some() {
                    return Err(SourceCommandError::Conflict(
                        "duplicate complete evidence index identity",
                    ));
                }
                set(
                    indexed,
                    id,
                    object(vec![
                        ("payload", payload.clone()),
                        ("source_ref", string(location)),
                        ("source_line", number((index + 1) as u64)),
                        ("source_sha256", string(&record_digest(&payload)?.to_hex())),
                    ]),
                )?;
            }
        }
    }
    let native_text_snapshot = if native_bindings.is_empty() {
        None
    } else {
        let cut = cut.ok_or(SourceCommandError::Unsupported(
            "maintained native text binding snapshot adapter",
        ))?;
        let mut reader = ProfileMetadataReader {
            ctx,
            cut,
            observed: BTreeMap::new(),
            observed_bytes: 0,
        };
        let resolved = crate::source_sign_native::resolve_bindings(
            &mut reader,
            executor,
            &native_bindings,
            NativeReadScope::MetadataOnly,
            deadline,
            cancelled,
        )?;
        if resolved.summaries.len() != native_bindings.len() {
            return Err(SourceCommandError::Conflict(
                "native profile resolver summary count",
            ));
        }
        for summary in &resolved.summaries {
            if summary.object_get("public_content_declared") != Some(&JsonValue::Bool(true)) {
                return Err(SourceCommandError::Denied(
                    "profile native text binding has no declared public content",
                ));
            }
        }
        // The exported closure must name only metadata inputs authenticated
        // by this same reader. Category-specific duplicates retain their roles.
        for input in &resolved.inputs {
            if input.kind == NativeReadKind::Content
                || reader
                    .observed
                    .get(&input.reference)
                    .map(|(digest, _)| *digest)
                    != Some(input.raw_sha256)
            {
                return Err(SourceCommandError::Conflict(
                    "native profile exported read closure differs",
                ));
            }
        }
        for (name, digest) in resolved.schema_digests {
            if Digest256::of_bytes(selected(ctx, &name)?) != digest {
                return Err(SourceCommandError::Conflict(
                    "native profile schema input differs from selected source",
                ));
            }
            let value = string(&digest.to_hex());
            if record_inputs
                .object_get(&name)
                .is_some_and(|prior| prior != &value)
            {
                return Err(SourceCommandError::Conflict(
                    "native profile schema digest conflict",
                ));
            }
            set(&mut record_inputs, &name, value)?;
        }
        // The owner's batch snapshot contains exactly its successful shared
        // cache inputs; it is not synthesized from caller-selected files.
        Some(resolved.input_snapshot)
    };
    if !has_profiled_claim {
        prior_profile_inputs = object(vec![]);
    }
    for entries in records.values_mut() {
        entries.sort_by(|left, right| {
            text(left, "record_id")
                .unwrap_or("")
                .cmp(text(right, "record_id").unwrap_or(""))
        });
    }
    let mut record_catalog = object(vec![]);
    for (kind, entries) in &records {
        set(&mut record_catalog, kind, JsonValue::Array(entries.clone()))?;
    }
    Ok(MaintainedInventory {
        records: record_catalog,
        record_inputs,
        record_member_inputs,
        objects,
        source_records,
        claims: prior,
        claim_profile_inputs: prior_profile_inputs,
        events,
        anchors,
        native_identity_snapshot,
        native_text_snapshot,
    })
}

fn maintained_grounding(
    ctx: &CommandContext,
    config: &JsonValue,
    claims: &[JsonValue],
    forms: Option<&[JsonValue]>,
    allow_existing: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ClaimGrounding> {
    let MaintainedInventory {
        records: record_catalog,
        record_inputs,
        objects,
        source_records,
        claims: prior,
        claim_profile_inputs: prior_profile_inputs,
        events,
        anchors,
        ..
    } = maintained_inventory(ctx, executor, deadline, cancelled)?;
    let mut new_profile_inputs = raw_digests(
        ctx,
        &[
            ENTITIES,
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            RELATIONS,
            "ToS/contracts/semantic-relation-type-registry.schema.json",
        ],
        false,
    )?;
    let mut bindings = object(vec![
        ("objects", object(vec![])),
        ("evidence", object(vec![])),
    ]);
    let mut bound_objects = object(vec![]);
    let mut bound_evidence = object(vec![]);
    let mut values = object(vec![]);
    let mut evidence = Vec::new();
    for claim in claims {
        if forms.is_none()
            && !allow_existing
            && (objects.contains_key(text(claim, "claim_id")?)
                || prior.contains_key(text(claim, "claim_id")?))
        {
            return Err(SourceCommandError::Conflict(
                "initial Claim identity exists in complete metadata universe",
            ));
        }
        if forms.is_none()
            && !allow_existing
            && events
                .object_get(text(config, "provenance_event_id")?)
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "initial serialization event identity exists in complete evidence universe",
            ));
        }
        let reader_kind = claim_profile_inputs(ctx, claim, &mut new_profile_inputs)?;
        let (relation, descriptor) = profile(ctx, text(claim, "predicate")?)?;
        let mut identities = BTreeSet::from([text(claim, "subject_ref")?.to_owned()]);
        if reader_kind == "identity" {
            identities.insert(text(claim, "object")?.to_owned());
        }
        if reader_kind == "temporal" && text(field(claim, "object")?, "kind")? == "relative-order" {
            identities.insert(
                text(field(field(claim, "object")?, "relative")?, "anchor_ref")?.to_owned(),
            );
        }
        if text(&descriptor, "reader")? == "structured-reference-value-v1" {
            for id in array(field(claim, "object")?, "members")? {
                identities.insert(
                    id.as_str()
                        .ok_or(SourceCommandError::Invalid("value member identity"))?
                        .into(),
                );
            }
        }
        for id in identities {
            let entry = objects.get(&id).ok_or(SourceCommandError::Conflict(
                "declared object missing from complete catalog",
            ))?;
            let raw = selected(ctx, text(entry, "source_record_ref")?)?;
            let record = &source_records[&id];
            set(
                &mut bound_objects,
                &id,
                object(vec![
                    ("source_ref", field(entry, "source_record_ref")?.clone()),
                    (
                        "source_sha256",
                        string(&Digest256::of_bytes(raw).to_prefixed()),
                    ),
                    (
                        "canonical_record_sha256",
                        string(&format!("sha256:{}", text(entry, "record_sha256")?)),
                    ),
                    (
                        "schema_version",
                        record
                            .object_get("schema_version")
                            .cloned()
                            .unwrap_or(JsonValue::Null),
                    ),
                    (
                        "record_version",
                        record
                            .object_get("record_version")
                            .cloned()
                            .unwrap_or(JsonValue::Null),
                    ),
                ]),
            )?;
        }
        if reader_kind != "identity" {
            let value = field(claim, "object")?;
            set(
                &mut values,
                text(claim, "claim_id")?,
                object(vec![
                    ("value", value.clone()),
                    ("sha256", string(&record_digest(value)?.to_prefixed())),
                    ("type_ids", field(&relation, "range_type_ids")?.clone()),
                ]),
            )?;
        }
        for key in ["evidence_refs", "counterevidence_refs"] {
            for reference in claim
                .object_get(key)
                .and_then(JsonValue::as_array)
                .unwrap_or(&[])
            {
                let reference = reference
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("evidence reference"))?;
                let node = maintained_evidence(ctx, reference, &objects, &anchors, &events)?;
                set(
                    &mut bound_evidence,
                    reference,
                    object(vec![
                        ("source_ref", field(&node, "source_ref")?.clone()),
                        (
                            "source_sha256",
                            string(&format!("sha256:{}", text(&node, "source_sha256")?)),
                        ),
                        (
                            "source_line",
                            node.object_get("source_line")
                                .cloned()
                                .unwrap_or(JsonValue::Null),
                        ),
                        (
                            "evidence_kind",
                            field(field(&node, "properties")?, "evidence_kind")?.clone(),
                        ),
                    ]),
                )?;
                evidence.push(node);
            }
        }
    }
    set(&mut bindings, "objects", bound_objects)?;
    set(&mut bindings, "evidence", bound_evidence)?;
    if !values.as_object().unwrap().is_empty() {
        set(&mut bindings, "values", values)?;
    }
    let mut form_inputs = object(vec![]);
    if let Some(selected_forms) = forms {
        let selected_ids = selected_forms
            .iter()
            .map(|selection| text(selection, "form_id"))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let mut paths = BTreeSet::new();
        for entry in objects.values() {
            let location = text(entry, "source_record_ref")?;
            paths.insert(format!(
                "{}.human-forms.json",
                location
                    .strip_suffix(".json")
                    .ok_or(SourceCommandError::Invalid("metadata form source basename"))?
            ));
        }
        for entry in prior.values() {
            let location = text(entry, "source_claim_file_ref")?;
            if location.ends_with(&format!("/{CLAIM_STREAM}"))
                || location.ends_with("/historical-claims.jsonl")
            {
                paths.insert(format!(
                    "{}/{}.{}.human-forms.json",
                    location.rsplit_once('/').unwrap().0,
                    location
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .strip_suffix(".jsonl")
                        .unwrap(),
                    Digest256::of_bytes(text(entry, "claim_id")?.as_bytes()).to_hex()
                ));
            }
        }
        let own = format!(
            "{}/{}",
            text(config, "source_path")?.rsplit_once('/').unwrap().0,
            form_name(text(config, "claim_id")?)
        );
        for locator in paths {
            if locator == own {
                continue;
            }
            let Some(raw) = ctx.file(&path(&locator)?)? else {
                continue;
            };
            let set_value = parse(raw)?;
            schema_check(
                executor,
                &locator,
                &set_value,
                "ToS/contracts/human-form-set.schema.json",
                deadline,
                cancelled,
            )?;
            apply_form_changes(Some(&set_value), field(&set_value, "subject")?, &[])?;
            for section in ["forms", "prior_forms"] {
                for form in array(&set_value, section)? {
                    if selected_ids.contains(text(form, "form_id")?) {
                        return Err(SourceCommandError::Conflict(
                            "complete current or prior form identity already allocated",
                        ));
                    }
                }
            }
            set(
                &mut form_inputs,
                &locator,
                string(&Digest256::of_bytes(raw).to_prefixed()),
            )?;
        }
    }
    let source_contracts = JsonValue::Object(
        record_inputs
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    string(&format!("sha256:{}", value.as_str().unwrap())),
                )
            })
            .collect(),
    );
    let grounding = object(vec![
        ("records", record_catalog),
        ("claims", JsonValue::Array(prior.into_values().collect())),
        (
            "source_profiles",
            object(vec![
                ("source_contracts", source_contracts),
                // collect_records always inspects the native semantic ID
                // inventory. Even its verified empty map has a fingerprint.
                (
                    "native_semantic_identity_snapshot",
                    string(&Digest256::of_bytes(b"{}").to_prefixed()),
                ),
            ]),
        ),
        ("existing_claim_profiles", prior_profile_inputs),
        ("new_claim_profiles", new_profile_inputs),
        ("events", events),
        ("anchors", anchors),
        ("evidence", JsonValue::Array(evidence)),
        ("selected_source_bindings", bindings.clone()),
        ("forms", form_inputs),
        (
            "provenance_contract",
            string(
                &Digest256::of_bytes(selected(
                    ctx,
                    "ToS/contracts/provenance-event-v2.schema.json",
                )?)
                .to_prefixed(),
            ),
        ),
        (
            "implementation",
            raw_digests(ctx, CLAIM_GROUNDING_RULE_INPUTS, true)?,
        ),
    ]);
    let mut dependencies = string(&record_digest(&grounding)?.to_prefixed());
    if forms.is_some() {
        dependencies = string(
            &record_digest(&object(vec![
                ("grounding", dependencies),
                (
                    "implementation",
                    raw_digests(ctx, CLAIM_REVISION_RULE_INPUTS, true)?,
                ),
            ]))?
            .to_prefixed(),
        );
    }
    Ok(ClaimGrounding {
        dependencies,
        bindings,
    })
}

fn bounded_navigation(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.into()
    } else {
        format!("{}…", value.chars().take(limit - 1).collect::<String>())
    }
}
fn python_bytes_blank(raw: &[u8]) -> bool {
    raw.iter()
        .all(|byte| matches!(*byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}
pub(crate) fn maintained_evidence(
    ctx: &CommandContext,
    reference: &str,
    objects: &BTreeMap<String, JsonValue>,
    anchors: &JsonValue,
    events: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    let (kind, location, digest, line, label, mut properties) = if reference.starts_with("ToS/") {
        (
            "repo_path",
            reference.to_owned(),
            Digest256::of_bytes(selected(ctx, reference)?).to_hex(),
            None,
            None,
            object(vec![]),
        )
    } else if let Some(indexed) = anchors.object_get(reference) {
        let anchor = field(indexed, "payload")?;
        let mut properties = anchor.clone();
        for (key, value) in [
            (
                "anchor_status",
                anchor
                    .object_get("status")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "item_ref",
                anchor
                    .object_get("item_id")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "file_ref",
                anchor
                    .object_get("file_id")
                    .cloned()
                    .unwrap_or(JsonValue::Null),
            ),
            ("source_anchor", anchor.clone()),
        ] {
            set(&mut properties, key, value)?;
        }
        (
            "anchor",
            text(indexed, "source_ref")?.into(),
            text(indexed, "source_sha256")?.into(),
            Some(integer(indexed, "source_line")?),
            None,
            properties,
        )
    } else if let Some(entry) = objects.get(reference) {
        (
            "identity",
            text(entry, "source_record_ref")?.into(),
            text(entry, "record_sha256")?.into(),
            None,
            entry
                .object_get("preferred_label")
                .and_then(JsonValue::as_str),
            object(vec![(
                "identity_node_id",
                string(&format!("identity:{reference}")),
            )]),
        )
    } else if let Some(indexed) = events.object_get(reference) {
        (
            "provenance_event",
            text(indexed, "source_ref")?.into(),
            text(indexed, "source_sha256")?.into(),
            Some(integer(indexed, "source_line")?),
            None,
            object(vec![(
                "provenance_event_node_id",
                string(&format!("provenance_event:{reference}")),
            )]),
        )
    } else {
        return Err(SourceCommandError::Unsupported(
            "maintained external citation evidence binding adapter",
        ));
    };
    set(&mut properties, "evidence_ref", string(reference))?;
    set(&mut properties, "evidence_kind", string(kind))?;
    set(&mut properties, "resolved", JsonValue::Bool(true))?;
    let supplied = label.is_some_and(|label| {
        !stripped(label).unwrap_or("").is_empty() && label.chars().count() <= 240
    });
    let (title, origin) = if supplied {
        (label.unwrap().to_owned(), "source-metadata-label")
    } else if kind == "repo_path" {
        (
            bounded_navigation(reference.rsplit('/').next().unwrap(), 240),
            "repository-filename-fallback",
        )
    } else {
        (
            bounded_navigation(
                &format!(
                    "{} · {}{}",
                    kind.replace('_', " "),
                    location.rsplit('/').next().unwrap(),
                    line.map(|v| format!(":{v}")).unwrap_or_default()
                ),
                240,
            ),
            "source-slot-fallback",
        )
    };
    let name = kind.replace('_', " ");
    let capitalized = format!("{}{}", name[..1].to_ascii_uppercase(), &name[1..]);
    let summary = bounded_navigation(
        &format!(
            "{capitalized} evidence reference: {reference}. Return to {location}{}.",
            line.map(|v| format!(":{v}")).unwrap_or_default()
        ),
        1024,
    );
    let display = object(vec![
        ("title", object(vec![("default", string(&title))])),
        ("summary", object(vec![("default", string(&summary))])),
        ("summary_state", string("metadata-synthesis")),
        (
            "provenance",
            object(vec![
                ("title", string(origin)),
                ("summary", string("evidence-reference-navigation")),
                ("source_title_available", JsonValue::Bool(supplied)),
                ("source_summary_available", JsonValue::Bool(false)),
                ("human_form_authority", string("none")),
                ("source_ref", string(&location)),
            ]),
        ),
    ]);
    let mut result = object(vec![
        (
            "node_id",
            string(&format!(
                "evidence:sha256:{}",
                Digest256::of_bytes(reference.as_bytes()).to_hex()
            )),
        ),
        ("node_kind", string("evidence")),
        ("source_ref", string(&location)),
        ("source_sha256", string(&digest)),
        ("display", display),
        ("properties", properties),
    ]);
    if let Some(line) = line {
        set(&mut result, "source_line", number(line))?;
    }
    Ok(result)
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
                    ground_collection_membership(ctx, claim, executor, deadline, cancelled)?;
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
        let record_type = native_record_type(&record)?;
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
            let reference = metadata_subject(&record)?;
            let (verified, source) = crate::source_revisions::resolve_record_version(
                ctx, &reference, executor, deadline, cancelled,
            )?;
            if !same(&record, &verified)? || source != _loc {
                return Err(SourceCommandError::Conflict(
                    "native endpoint current owner changed",
                ));
            }
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
            let id_field = match record
                .object_get("schema_version")
                .and_then(JsonValue::as_str)
            {
                Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
                    "artifact_id"
                }
                Some("tos_scholarly_composite_witness_v1") => "composite_id",
                _ => "record_id",
            };
            if record.object_get(id_field).and_then(JsonValue::as_str) == Some(id) {
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
pub(crate) fn ancestry(types: &[JsonValue], id: &str) -> SourceCommandResult<BTreeSet<String>> {
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
        || !same(
            field(&manifest, "revision")?,
            field(receipt, "previous_revision")?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "retained Claim archive bytes differ",
        ));
    }
    let mut restored = BTreeMap::new();
    let mut bound_blobs = BTreeSet::new();
    for (name, binding) in field(&manifest, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("archive file bindings"))?
    {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("archive basename"))?;
        if name.is_empty() || name.contains('/') || [".", ".."].contains(&name) {
            return Err(SourceCommandError::Invalid("archive basename"));
        }
        exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let blob = text(binding, "blob")?;
        let raw = files
            .get(blob)
            .ok_or(SourceCommandError::Conflict("retained archive blob absent"))?;
        if blob != blob_name(raw)
            || text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "retained archive blob binding differs",
            ));
        }
        bound_blobs.insert(blob.to_owned());
        restored.insert(name.to_owned(), raw.clone());
    }
    if files.keys().cloned().collect::<BTreeSet<_>>() != bound_blobs
        || !same(&revision(&restored)?, field(receipt, "previous_revision")?)?
    {
        return Err(SourceCommandError::Conflict(
            "retained archive contains unbound bytes or wrong package revision",
        ));
    }
    Ok(restored)
}

fn blob_name(raw: &[u8]) -> String {
    format!("{}.blob", Digest256::of_bytes(raw).to_hex())
}
fn archive_refs(files: &BTreeMap<String, Vec<u8>>) -> JsonValue {
    let mut result = refs(files);
    if let JsonValue::Object(entries) = &mut result {
        for (name, binding) in entries {
            // Keys originate from the same verified finite package map.
            let raw = &files[name.as_str().unwrap()];
            if let JsonValue::Object(fields) = binding {
                fields.push((
                    tos_foundation::JsonString::from_utf8("blob"),
                    string(&blob_name(raw)),
                ));
            }
        }
    }
    result
}
fn archive_locations(files: &BTreeMap<String, Vec<u8>>, directory: &str) -> JsonValue {
    let mut result = refs(files);
    if let JsonValue::Object(entries) = &mut result {
        for (name, binding) in entries {
            let raw = &files[name.as_str().unwrap()];
            if let JsonValue::Object(fields) = binding {
                fields.push((
                    tos_foundation::JsonString::from_utf8("archive_path"),
                    string(&format!("{directory}/{}", blob_name(raw))),
                ));
            }
        }
    }
    result
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
        let recorded_at = text(receipt, "recorded_at")?;
        tos_validation::retirement_rules::observed_instant_order(recorded_at, recorded_at)
            .map_err(|_| {
                SourceCommandError::Invalid(
                    "retained Claim receipt instant requires explicit valid timezone",
                )
            })?;
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
        let basename = file.path.as_str().rsplit('/').next().unwrap_or("");
        if file.path.as_str().starts_with("ToS/source-witnesses/")
            && (basename == CLAIM_STREAM || LEGACY_CLAIM_STREAMS.contains(&basename))
            && !file.path.as_str().contains("/.record-revisions/")
            && !file
                .path
                .as_str()
                .split('/')
                .any(|part| ["catalog", "payload", "local-content"].contains(&part))
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
        let (record, source) = crate::source_revisions::resolve_record_version(
            ctx, reference, executor, deadline, cancelled,
        )?;
        let kind = native_record_type(&record)?;
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
            if role != "identity" {
                return Err(SourceCommandError::Denied(
                    "native identity participant role",
                ));
            }
            // The exact metadata owner resolver above validates each actual
            // native carrier schema and retained history; no copied descriptor.
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
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
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
    let (collection, _) = crate::source_revisions::resolve_record_version(
        ctx,
        collection_ref,
        executor,
        deadline,
        cancelled,
    )?;
    if text(&collection, "record_type")? != "collection" {
        return Err(SourceCommandError::Invalid(
            "Collection order basis record type",
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

fn native_record_type(record: &JsonValue) -> SourceCommandResult<&str> {
    if let Some(kind) = record.object_get("record_type").and_then(JsonValue::as_str) {
        return Ok(kind);
    }
    match text(record, "schema_version")? {
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => Ok("artifact"),
        "tos_scholarly_composite_witness_v1" => Ok("composite"),
        _ => Err(SourceCommandError::Unsupported(
            "native metadata record type adapter",
        )),
    }
}

fn stripped(value: &str) -> SourceCommandResult<&str> {
    python_strip_unicode16_v1(value, 1_048_576)
        .map_err(|_| SourceCommandError::Invalid("Claim Python Unicode16 strip budget"))
}

fn identifier_tail(value: &str, prefix: &str) -> bool {
    let Some(tail) = value.strip_prefix(prefix) else {
        return false;
    };
    !tail.is_empty()
        && tail.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}
