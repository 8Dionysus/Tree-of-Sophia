//! Source-owned Claim command mechanics. Selected bytes are proposals, never
//! admission. Schemas are executed by the actual bounded source-cut worker.
//! Unsupported grounding adapters fail closed without flattening their values.
use crate::source_command::*;
use crate::source_creation_store::{ClaimCatalogCapture, CreationFilesystem};
use crate::source_forms::{
    apply_form_changes, materialize_source_forms, metadata_subject, prepare_form_change,
};
use crate::source_sign_native::{NativeReadKind, NativeReadScope, SignNativeRead};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tos_foundation::{
    canonical_count_v1, python_strip_unicode16_v1, CanonicalProfile, Digest256, JsonLimits,
    JsonString, JsonValue, RelativePath,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use tos_validation::PredicateRead;

pub const CLAIM_STREAM: &str = "source-claims.jsonl";
pub const CLAIM_HISTORY: &str = "claim-revision-history.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CLAIM_COMPLETE_BYTES: u64 = 33_554_432;

/// One isolated Claim invocation's source/catalog/history work. Software
/// capture and the schema executable retain their separate owner budgets.
pub(crate) struct ClaimCallBudget {
    read_bytes: u64,
    live_state_bytes: usize,
}
impl ClaimCallBudget {
    fn new(read_bytes: u64, live_state_bytes: usize) -> SourceCommandResult<Self> {
        if read_bytes > CLAIM_COMPLETE_BYTES || live_state_bytes > CLAIM_COMPLETE_BYTES as usize {
            return Err(SourceCommandError::Unsupported(
                "Claim whole-call source budget",
            ));
        }
        Ok(Self {
            read_bytes,
            live_state_bytes,
        })
    }
    pub(crate) fn read(&mut self, bytes: u64) -> SourceCommandResult<()> {
        self.read_bytes = self
            .read_bytes
            .checked_add(bytes)
            .filter(|n| *n <= CLAIM_COMPLETE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "Claim whole-call read budget",
            ))?;
        Ok(())
    }
    pub(crate) fn check_live(&self, additional: usize) -> SourceCommandResult<()> {
        if self
            .live_state_bytes
            .checked_add(additional)
            .is_none_or(|n| n > CLAIM_COMPLETE_BYTES as usize)
        {
            return Err(SourceCommandError::Unsupported(
                "Claim whole-call live state budget",
            ));
        }
        Ok(())
    }
    pub(crate) fn retain(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.check_live(bytes)?;
        self.live_state_bytes += bytes;
        Ok(())
    }
    pub(crate) fn remaining_read(&self) -> SourceCommandResult<u64> {
        CLAIM_COMPLETE_BYTES
            .checked_sub(self.read_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "Claim whole-call read budget",
            ))
    }
    pub(crate) fn remaining_live(&self) -> SourceCommandResult<usize> {
        (CLAIM_COMPLETE_BYTES as usize)
            .checked_sub(self.live_state_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "Claim whole-call live state budget",
            ))
    }
}
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
pub(crate) fn family(schema: &str) -> SourceCommandResult<(String, bool, u8)> {
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
    let identity_predicate = match text(&c, "schema_version")? {
        "tos_local_identity_proposal_create_owner_v1" => Some("identity_transition_proposal"),
        "tos_local_identity_proposal_create_owner_v2" => {
            Some("subject_identity_transition_proposal")
        }
        _ => None,
    };
    if let Some(expected) = identity_predicate {
        let predicates = array(&c, "allowed_predicates")?;
        if predicates.len() != 1 || predicates[0].as_str() != Some(expected) {
            return Err(SourceCommandError::Denied(
                "identity proposal delegation selects its exact predicate",
            ));
        }
    }
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
    package_bounded(ctx, p, None, None)
}

fn package_bounded(
    ctx: &CommandContext,
    p: &RelativePath,
    remaining_read: Option<u64>,
    remaining_state: Option<usize>,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent = p
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Claim parent"))?
        .0;
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for f in &ctx.files {
        if let Some(name) = f.path.as_str().strip_prefix(&format!("{parent}/")) {
            if !name.contains('/') {
                total = total
                    .checked_add(f.raw.len())
                    .filter(|sum| {
                        *sum <= 8_388_608
                            && remaining_read.is_none_or(|cap| *sum as u64 <= cap)
                            && remaining_state.is_none_or(|cap| *sum <= cap)
                    })
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim package read or state budget",
                    ))?;
                if files.len() >= 64 || f.raw.len() > 8_388_608 {
                    return Err(SourceCommandError::Invalid("Claim package budget"));
                }
                files.insert(name.into(), f.raw.clone());
            }
        }
    }
    if files.len() > 64 || total > 8_388_608 {
        return Err(SourceCommandError::Invalid("Claim package budget"));
    }
    Ok(files)
}
fn refs_view<'a>(files: impl Iterator<Item = (&'a str, &'a [u8])>) -> JsonValue {
    JsonValue::Object(
        files
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
fn refs(files: &BTreeMap<String, Vec<u8>>) -> JsonValue {
    refs_view(
        files
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
}
pub(crate) fn revision(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    Ok(string(&record_digest(&refs(files))?.to_prefixed()))
}
fn revised_package_revision(
    files: &BTreeMap<&str, &[u8]>,
    overlapping_state: usize,
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
) -> SourceCommandResult<JsonValue> {
    // The same refs_view recipe used by revision() is built from borrowed
    // successor bytes. Reserve both its bounded value and the canonical hash
    // buffer before either is allocated; no second package copy/read occurs.
    let template = object(vec![
        ("sha256", string(&Digest256::of_bytes(b"").to_prefixed())),
        ("bytes", number(u64::MAX)),
    ]);
    let template_state = retained_value_bytes(&template)?;
    let refs_state = files.iter().try_fold(2usize, |sum, (name, _)| {
        sum.checked_add(catalogue_string_bound(name)?)
            .and_then(|n| n.checked_add(template_state))
            .and_then(|n| n.checked_add(2 + std::mem::size_of::<(JsonString, JsonValue)>()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim revised package digest state overflow",
            ))
    })?;
    if let Some(budget) = whole_call {
        let peak = refs_state
            .checked_mul(2)
            .and_then(|state| state.checked_add(overlapping_state))
            .ok_or(SourceCommandError::Unsupported(
                "Claim revised package digest state overflow",
            ))?;
        budget.borrow().check_live(peak)?;
    }
    let digest = record_digest(&refs_view(files.iter().map(|(name, raw)| (*name, *raw))))?;
    if let Some(budget) = whole_call {
        budget.borrow_mut().retain(refs_state)?;
    }
    Ok(string(&digest.to_prefixed()))
}
fn form_name(id: &str) -> String {
    format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(id.as_bytes())
            .to_prefixed()
            .trim_start_matches("sha256:")
    )
}

fn current_claim_materializations(
    record: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    id: &str,
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
) -> SourceCommandResult<JsonValue> {
    let Some(raw) = files.get(&form_name(id)) else {
        return Ok(JsonValue::Array(vec![]));
    };
    if let Some(budget) = whole_call {
        // The selected raw bytes already belong to the package. Its parsed
        // value and the existing source-copy kernel's 262,144-byte output
        // limit overlap here with the current record and retained context.
        let temporary = raw
            .len()
            .checked_add(retained_value_bytes(record)?)
            .and_then(|n| n.checked_add(262_144))
            .ok_or(SourceCommandError::Unsupported(
                "Claim current materialization state overflow",
            ))?;
        budget.borrow().check_live(temporary)?;
    }
    let payload = parse(raw)?;
    let materializations = JsonValue::Array(materialize_source_forms(record, &payload)?);
    if let Some(budget) = whole_call {
        budget
            .borrow_mut()
            .retain(retained_value_bytes(&materializations)?)?;
    }
    Ok(materializations)
}

/// Execute against the exact current selected cut and protected configuration.
/// The successful output has no production writer or authority lease.
pub fn run_claim_command(
    ctx: &CommandContext,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    run_claim_command_inner(
        ctx, None, None, None, None, None, false, executor, deadline, cancelled,
    )
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
    let complete = claim_context_with_files(ctx, cut.current().revision(), files);
    complete.check()?;
    run_claim_command_inner(
        &complete,
        Some(ctx),
        Some(cut),
        None,
        None,
        None,
        false,
        executor,
        deadline,
        cancelled,
    )
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
    run_claim_command_inner(
        &complete,
        Some(ctx),
        Some(cut),
        None,
        None,
        None,
        false,
        executor,
        deadline,
        cancelled,
    )
}

/// Read-only isolated creation preview for Claims whose exact grounding needs
/// the separately selected generated catalogue. The physical owner and
/// selected cut are rechecked; the returned plan has no publication route.
pub fn prepare_isolated_claim_creation_from_captures(
    filesystem: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    let (_, _, _, create, _) = config(ctx)?;
    if !create || text(&parse(&ctx.request_raw)?, "operation")? != "prepare-create" {
        return Err(SourceCommandError::Unsupported(
            "isolated Claim creation preview operation",
        ));
    }
    filesystem.current_context(ctx, deadline, cancelled)?;
    let whole_call = claim_call_for_selected_context(ctx)?;
    charge_selected_claim_context(ctx, cut, &whole_call)?;
    let complete =
        selected_claim_context_from_captures(ctx, cut, software, components, deadline, cancelled)?;
    let request = parse(&ctx.request_raw)?;
    let needs_catalog = array(&request, "claims")?
        .iter()
        .map(|claim| retained_profile_required(&complete, claim))
        .collect::<SourceCommandResult<Vec<_>>>()?
        .into_iter()
        .any(|needed| needed);
    let catalog = if needs_catalog {
        let budget = whole_call.borrow();
        budget.check_live(8192)?;
        if budget.remaining_read()? < 8192 {
            return Err(SourceCommandError::Unsupported(
                "Claim catalog control read budget",
            ));
        }
        drop(budget);
        let selected = filesystem.select_claim_catalog(deadline, cancelled)?;
        selected.bind_cut(cut, deadline, cancelled)?;
        let bytes = selected.control().map_or(0, <[u8]>::len);
        let mut budget = whole_call.borrow_mut();
        budget.read(bytes as u64)?;
        budget.retain(bytes)?;
        Some(Rc::new(RefCell::new(selected)))
    } else {
        None
    };
    let plan = run_claim_command_inner(
        &complete,
        Some(&complete),
        Some(cut),
        catalog.clone(),
        None,
        Some(whole_call.clone()),
        false,
        worker,
        deadline,
        cancelled,
    )?;
    filesystem.current_context(ctx, deadline, cancelled)?;
    if let Some(catalog) = catalog {
        let bytes = catalog.borrow().selected_bytes()?;
        let mut budget = whole_call.borrow_mut();
        budget.check_live(bytes)?;
        budget.read(bytes as u64)?;
        drop(budget);
        catalog
            .borrow()
            .verify_current(filesystem, deadline, cancelled)?;
    }
    Ok(plan)
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
    let complete = claim_context_with_files(ctx, cut.current().revision(), files);
    complete.check()?;
    Ok(complete)
}

fn claim_context_with_files(
    ctx: &CommandContext,
    base_revision: tos_foundation::SourceRevision,
    files: Vec<SourceFile>,
) -> CommandContext {
    CommandContext {
        base_revision,
        configuration_raw: ctx.configuration_raw.clone(),
        request_raw: ctx.request_raw.clone(),
        recorded_at: ctx.recorded_at.clone(),
        effective_uid: ctx.effective_uid,
        files,
    }
}

fn claim_call_for_selected_context(
    ctx: &CommandContext,
) -> SourceCommandResult<Rc<RefCell<ClaimCallBudget>>> {
    let files = ctx
        .files
        .iter()
        .try_fold(0usize, |sum, file| {
            sum.checked_add(file.raw.len())
                .and_then(|sum| sum.checked_add(file.path.as_str().len()))
                .and_then(|sum| sum.checked_add(std::mem::size_of::<SourceFile>()))
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim caller state overflow",
        ))?;
    let live = files
        .checked_add(ctx.configuration_raw.len())
        .and_then(|sum| sum.checked_add(ctx.request_raw.len()))
        .and_then(|sum| sum.checked_add(ctx.recorded_at.len()))
        .and_then(|sum| sum.checked_add(std::mem::size_of::<CommandContext>()))
        .ok_or(SourceCommandError::Unsupported(
            "Claim caller state overflow",
        ))?;
    Ok(Rc::new(RefCell::new(ClaimCallBudget::new(0, live)?)))
}

fn charge_selected_claim_context(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    whole_call: &Rc<RefCell<ClaimCallBudget>>,
) -> SourceCommandResult<()> {
    let (authored, selected_paths, selected_count) = cut
        .current()
        .members()
        .try_fold((0usize, 0usize, 0usize), |(bytes, paths, count), member| {
            Some((
                bytes.checked_add(usize::try_from(member.size_bytes).ok()?)?,
                paths.checked_add(member.path.as_str().len())?,
                count.checked_add(1)?,
            ))
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim selected cut size overflow",
        ))?;
    let (software_copy, software_paths, software_count) = ctx
        .files
        .iter()
        .filter(|file| !file.path.as_str().starts_with("ToS/"))
        .try_fold((0usize, 0usize, 0usize), |(bytes, paths, count), file| {
            Some((
                bytes.checked_add(file.raw.len())?,
                paths.checked_add(file.path.as_str().len())?,
                count.checked_add(1)?,
            ))
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim selected software state overflow",
        ))?;
    let mut budget = whole_call.borrow_mut();
    budget.read(authored as u64)?;
    budget.retain(
        authored
            .checked_add(software_copy)
            .and_then(|sum| sum.checked_add(software_paths))
            .and_then(|sum| {
                sum.checked_add(software_count.checked_mul(std::mem::size_of::<SourceFile>())?)
            })
            .and_then(|sum| sum.checked_add(selected_paths))
            .and_then(|sum| {
                sum.checked_add(selected_count.checked_mul(std::mem::size_of::<SourceFile>())?)
            })
            .and_then(|sum| sum.checked_add(ctx.configuration_raw.len()))
            .and_then(|sum| sum.checked_add(ctx.request_raw.len()))
            .and_then(|sum| sum.checked_add(ctx.recorded_at.len()))
            .and_then(|sum| sum.checked_add(std::mem::size_of::<CommandContext>()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim selected context state overflow",
            ))?,
    )
}

/// A complete, native-observed Claim creation package. Its selected context is
/// retained for custody checks, not as a grant to the canonical source writer.
pub struct SerializedClaimCreation {
    context: CommandContext,
    command: PreparedCommand,
    home: RelativePath,
    files: BTreeMap<String, Vec<u8>>,
    operational_sidecars: BTreeSet<String>,
    components: SoftwareComponentSelectionV1,
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
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
    pub(crate) fn operational_sidecars(&self) -> &BTreeSet<String> {
        &self.operational_sidecars
    }
    pub(crate) fn catalog_capture(&self) -> Option<&Rc<RefCell<ClaimCatalogCapture>>> {
        self.catalog_capture.as_ref()
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
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SerializedClaimCreation> {
    claim_creation_from_captures(
        ctx,
        cut,
        software,
        components,
        worker,
        None,
        catalog_capture,
        whole_call,
        deadline,
        cancelled,
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
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
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
        catalog_capture,
        whole_call,
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
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SerializedClaimCreation> {
    if let Some(budget) = &whole_call {
        charge_selected_claim_context(ctx, cut, budget)?;
    }
    let complete =
        selected_claim_context_from_captures(ctx, cut, software, components, deadline, cancelled)?;
    let request = parse(&complete.request_raw)?;
    if text(&request, "operation")? != "claims.create" {
        return Err(SourceCommandError::Unsupported(
            "native Claim serialization requires claims.create",
        ));
    }
    let preview = run_claim_command_inner(
        &complete,
        Some(ctx),
        Some(cut),
        catalog_capture.clone(),
        None,
        whole_call.clone(),
        true,
        worker,
        deadline,
        cancelled,
    )?;
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
    let operational_sidecars = claim_creation_allowed_names(&request)?
        .into_iter()
        .filter(|name| name.ends_with(".writer.lock"))
        .map(|name| format!("{}/{name}", home.as_str()))
        .collect();
    let stream = preview.changes[0]
        .after
        .as_ref()
        .ok_or(SourceCommandError::Conflict("initial Claim stream absent"))?;
    if let Some(budget) = &whole_call {
        budget.borrow_mut().retain(stream.len())?;
    }
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
    if let Some(budget) = &whole_call {
        let package_bytes = files.values().map(Vec::len).sum::<usize>();
        budget
            .borrow_mut()
            .retain(package_bytes.saturating_sub(stream.len()))?;
        if retained.is_none() {
            budget.borrow_mut().retain(package_bytes)?;
        }
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
        operational_sidecars,
        components: components.clone(),
        catalog_capture,
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
    let request = parse(&ctx.request_raw)?;
    let allowed_names = claim_creation_allowed_names(&request)?;
    let needs_catalog = array(&request, "claims")?
        .iter()
        .map(|claim| retained_profile_required(ctx, claim))
        .collect::<SourceCommandResult<Vec<_>>>()?
        .into_iter()
        .any(|needed| needed);
    let whole_call = needs_catalog
        .then(|| claim_call_for_selected_context(ctx))
        .transpose()?;
    if let Some(budget) = &whole_call {
        let budget = budget.borrow();
        budget.check_live(8192)?;
        if budget.remaining_read()? < 8192 {
            return Err(SourceCommandError::Unsupported(
                "Claim catalog control read budget",
            ));
        }
    }
    let current_catalog = needs_catalog
        .then(|| filesystem.select_claim_catalog(deadline, cancelled))
        .transpose()?
        .map(|selected| Rc::new(RefCell::new(selected)));
    if let (Some(budget), Some(catalog)) = (&whole_call, &current_catalog) {
        let control_bytes = catalog.borrow().control().map_or(0, <[u8]>::len);
        let mut budget = budget.borrow_mut();
        budget.read(control_bytes as u64)?;
        budget.retain(control_bytes)?;
    }
    let retained = filesystem.read_claim_retained(
        ctx,
        &home,
        &allowed_names,
        whole_call.as_ref(),
        deadline,
        cancelled,
    )?;
    let mut current = None;
    let serialized = if let Some(retained) = &retained {
        if let Some(catalog) = &current_catalog {
            catalog
                .borrow()
                .bind_cut(current_cut, deadline, cancelled)?;
        }
        if let Some(budget) = &whole_call {
            charge_selected_claim_context(ctx, current_cut, budget)?;
        }
        let current_context =
            claim_context_with_files(ctx, current_cut.current().revision(), vec![]);
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
                current_cut,
                &parse(&ctx.configuration_raw)?,
                &parse(&ctx.request_raw)?,
                current_worker,
                current_catalog.clone(),
                whole_call.clone(),
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
            whole_call.as_ref(),
        )?;
        let original_catalog = if needs_catalog {
            Some(Rc::new(RefCell::new(
                filesystem.read_claim_catalog_capture(
                    original.get("source-create-receipt.json").ok_or(
                        SourceCommandError::Conflict("Claim original receipt absent"),
                    )?,
                    original_cut,
                    whole_call.as_ref(),
                    true,
                    deadline,
                    cancelled,
                )?,
            )))
        } else {
            None
        };
        current = Some(current_context);
        restore_claim_creation_from_captures(
            ctx,
            original_cut,
            software,
            components,
            worker,
            &original,
            original_catalog,
            whole_call.clone(),
            deadline,
            cancelled,
        )?
    } else {
        if let Some(catalog) = &current_catalog {
            catalog
                .borrow()
                .bind_cut(original_cut, deadline, cancelled)?;
        }
        serialize_claim_creation_from_captures(
            ctx,
            original_cut,
            software,
            components,
            worker,
            current_catalog.clone(),
            whole_call.clone(),
            deadline,
            cancelled,
        )?
    };
    // Every supplied schema child must be finalized before publication locks,
    // including a selected current worker not needed by this Claim profile.
    if let Some(current_worker) = current_worker.as_deref_mut() {
        crate::source_creation_store::finish_creation_worker(current_worker, deadline, cancelled)?;
    }
    crate::source_creation_store::finish_creation_worker(worker, deadline, cancelled)?;
    if let Some(catalog) = &current_catalog {
        if let Some(budget) = &whole_call {
            let bytes = catalog.borrow().selected_bytes()?;
            budget.borrow_mut().check_live(bytes)?;
            budget.borrow_mut().read(bytes as u64)?;
        }
        catalog
            .borrow()
            .verify_current(filesystem, deadline, cancelled)?;
    }
    let publication = if retained.is_some() {
        let current_catalog_borrow = current_catalog.as_ref().map(|catalog| catalog.borrow());
        filesystem.replay_claim_isolated(
            &serialized,
            original_cut,
            current.as_ref().ok_or(SourceCommandError::Conflict(
                "Claim replay current cut absent",
            ))?,
            current_cut,
            current_catalog_borrow.as_deref(),
            whole_call.as_ref(),
            software,
            components,
            deadline,
            cancelled,
        )?
    } else {
        filesystem.publish_claim_isolated(
            &serialized,
            original_cut,
            whole_call.as_ref(),
            software,
            components,
            deadline,
            cancelled,
        )?
    };
    let response = serialized.command().response.clone();
    Ok((serialized, publication, response))
}

struct CheckedClaimRevision {
    context: CommandContext,
    home: RelativePath,
    before: BTreeMap<String, Vec<u8>>,
    operational_sidecars: BTreeSet<String>,
    plan: PreparedCommand,
    current_catalog: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    whole_call: Rc<RefCell<ClaimCallBudget>>,
}

fn checked_isolated_claim_revision(
    filesystem: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CheckedClaimRevision> {
    let whole_call = claim_call_for_selected_context(ctx)?;
    charge_selected_claim_context(ctx, current_cut, &whole_call)?;
    let complete = selected_claim_context_from_captures(
        ctx,
        current_cut,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let (config, source_path, _, create, _) = config(&complete)?;
    if create {
        return Err(SourceCommandError::Denied(
            "isolated Claim revision owner family",
        ));
    }
    let request = parse(&complete.request_raw)?;
    if !["prepare-revise", "claim.revise"].contains(&text(&request, "operation")?) {
        return Err(SourceCommandError::Unsupported(
            "isolated Claim revision operation",
        ));
    }
    let home = path(
        source_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim revision parent"))?
            .0,
    )?;
    let current_raw = selected(&complete, source_path.as_str())?;
    whole_call.borrow().check_live(current_raw.len())?;
    let current = rows(current_raw)?;
    let record = current
        .get(text(&config, "claim_id")?)
        .ok_or(SourceCommandError::Conflict(
            "delegated Claim absent from current cut",
        ))?;
    let mut names = BTreeSet::new();
    let prefix = format!("{}/", home.as_str());
    for input in complete
        .files
        .iter()
        .filter(|input| input.path.as_str().starts_with(&prefix))
    {
        let leaf = &input.path.as_str()[prefix.len()..];
        if !leaf.contains('/') {
            names.insert(leaf.to_owned());
        }
    }
    let mut operational_sidecars = BTreeSet::new();
    for id in current.keys() {
        let lock = format!(".{}.writer.lock", form_name(id));
        names.insert(lock.clone());
        operational_sidecars.insert(format!("{}/{}", home.as_str(), lock));
    }
    let before = filesystem
        .read_claim_retained(
            &complete,
            &home,
            &names,
            Some(&whole_call),
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "Claim revision current package absent",
        ))?;
    let needs_catalog = retained_profile_required(&complete, record)?;
    if needs_catalog {
        let budget = whole_call.borrow();
        budget.check_live(8192)?;
        if budget.remaining_read()? < 8192 {
            return Err(SourceCommandError::Unsupported(
                "Claim catalog control read budget",
            ));
        }
    }
    let current_catalog = needs_catalog
        .then(|| filesystem.select_claim_catalog(deadline, cancelled))
        .transpose()?
        .map(|capture| Rc::new(RefCell::new(capture)));
    if let Some(catalog) = &current_catalog {
        let control_bytes = catalog.borrow().control().map_or(0, <[u8]>::len);
        whole_call.borrow_mut().read(control_bytes as u64)?;
        whole_call.borrow_mut().retain(control_bytes)?;
        catalog
            .borrow()
            .bind_cut(current_cut, deadline, cancelled)?;
        let original_receipt =
            before
                .get("source-create-receipt.json")
                .ok_or(SourceCommandError::Conflict(
                    "Claim original receipt absent",
                ))?;
        filesystem.read_claim_catalog_capture(
            original_receipt,
            original_cut,
            Some(&whole_call),
            false,
            deadline,
            cancelled,
        )?;
    }
    let plan = run_claim_command_inner(
        &complete,
        Some(&complete),
        Some(current_cut),
        current_catalog.clone(),
        Some(&before),
        Some(whole_call.clone()),
        false,
        worker,
        deadline,
        cancelled,
    )?;
    Ok(CheckedClaimRevision {
        context: complete,
        home,
        before,
        operational_sidecars,
        plan,
        current_catalog,
        whole_call,
    })
}

/// Read-only isolated preview uses the actual flat predecessor, including
/// exact empty operational locks, while returning only an unauthorised plan.
pub fn prepare_isolated_claim_revision_from_captures(
    filesystem: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    let prepared = checked_isolated_claim_revision(
        filesystem,
        ctx,
        original_cut,
        current_cut,
        software,
        components,
        worker,
        deadline,
        cancelled,
    )?;
    if text(&parse(&ctx.request_raw)?, "operation")? != "prepare-revise" {
        return Err(SourceCommandError::Unsupported(
            "Claim revision preview operation",
        ));
    }
    Ok(prepared.plan)
}

/// The only isolated native revision entry rebuilds the package internally;
/// neither a public proposal nor a retained receipt is a publication grant.
pub fn execute_isolated_claim_revision_from_captures(
    filesystem: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    PreparedCommand,
    crate::source_creation_store::CreationPublication,
)> {
    let prepared = checked_isolated_claim_revision(
        filesystem,
        ctx,
        original_cut,
        current_cut,
        software,
        components,
        worker,
        deadline,
        cancelled,
    )?;
    if text(&parse(&ctx.request_raw)?, "operation")? != "claim.revise" {
        return Err(SourceCommandError::Unsupported(
            "Claim revision commit operation",
        ));
    }
    crate::source_creation_store::finish_creation_worker(worker, deadline, cancelled)?;
    let current_catalog = prepared
        .current_catalog
        .as_ref()
        .map(|capture| capture.borrow());
    let publication = filesystem.publish_claim_revision_isolated(
        &prepared.context,
        &prepared.home,
        &prepared.before,
        &prepared.operational_sidecars,
        &prepared.plan,
        original_cut,
        current_cut,
        current_catalog.as_deref(),
        &prepared.whole_call,
        software,
        components,
        deadline,
        cancelled,
    )?;
    Ok((prepared.plan, publication))
}

fn reference_replay_required(
    current: &CommandContext,
    request: &JsonValue,
) -> SourceCommandResult<bool> {
    for claim in array(request, "claims")? {
        let (_, descriptor) = profile(current, text(claim, "predicate")?)?;
        if text(&descriptor, "reader")? == "structured-reference-value-v1"
            || retained_profile_required(current, claim)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reference_replay_snapshot(
    current: &CommandContext,
    cut: &CorpusCutReader,
    config: &JsonValue,
    request: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    let mut special_claims = Vec::new();
    let mut bindings = BTreeMap::new();
    let mut retained = None;
    for claim in array(request, "claims")? {
        let (_, descriptor) = profile(current, text(claim, "predicate")?)?;
        if text(&descriptor, "reader")? == "structured-reference-value-v1"
            || retained_profile_required(current, claim)?
        {
            let (_, create, version) = family(text(config, "schema_version")?)?;
            if !create {
                return Err(SourceCommandError::Denied("reference replay owner family"));
            }
            claim_scope(config, claim, true, version)?;
            if retained.is_none() && retained_profile_required(current, claim)? {
                retained = Some(RetainedProfileRead::new(
                    current,
                    worker,
                    deadline,
                    cancelled,
                    catalog_capture.clone(),
                    whole_call.clone(),
                )?);
            }
            let binding = validate_ground(
                current,
                Some(cut),
                retained.as_mut(),
                config,
                claim,
                version,
                worker,
                deadline,
                cancelled,
            )?;
            if let Some(binding) = binding {
                if let Some(read) = retained.as_mut() {
                    read.debit(0, retained_value_bytes(&binding.1)?)?;
                }
                bindings.insert(text(claim, "claim_id")?.to_owned(), binding);
            }
            special_claims.push(claim.clone());
        }
    }
    let grounding = maintained_grounding(
        current,
        config,
        &special_claims,
        None,
        true,
        retained
            .take()
            .map(|read| read.into_inventory(current, Some(cut)))
            .transpose()?,
        &bindings,
        worker,
        deadline,
        cancelled,
    )?;
    Ok(record_digest(&grounding.dependencies)?)
}

fn retained_claim_creation_files(
    current: &CommandContext,
    config: &JsonValue,
    request: &JsonValue,
    source_path: &RelativePath,
    direct: &BTreeMap<String, Vec<u8>>,
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
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
    let allowed = claim_creation_allowed_names(request)?;
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
    let mut first_archive = None;
    let history = verify_claim_history_inner(
        current,
        config,
        source_path,
        direct,
        None,
        whole_call,
        0,
        Some(&mut first_archive),
    )?;
    let archived_or_current = if !array(&history, "receipts")?.is_empty() {
        first_archive.ok_or(SourceCommandError::Conflict(
            "Claim first verified archive absent",
        ))?
    } else {
        if let Some(budget) = whole_call {
            let bytes = direct.values().map(Vec::len).sum::<usize>();
            budget.borrow_mut().retain(bytes)?;
        }
        direct.clone()
    };
    if let Some(budget) = whole_call {
        let bytes = required
            .iter()
            .try_fold(0usize, |sum, name| {
                archived_or_current
                    .get(name)
                    .and_then(|raw| sum.checked_add(raw.len()))
            })
            .ok_or(SourceCommandError::Conflict(
                "Claim archived creation member absent",
            ))?;
        budget.borrow_mut().retain(bytes)?;
    }
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

fn claim_creation_allowed_names(request: &JsonValue) -> SourceCommandResult<BTreeSet<String>> {
    let claims = array(request, "claims")?;
    if claims.is_empty() || claims.len() > 32 {
        return Err(SourceCommandError::Invalid("initial Claim batch capacity"));
    }
    let mut names = BTreeSet::from([
        CLAIM_STREAM.to_owned(),
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
        "source-create-receipt.json".to_owned(),
        CLAIM_HISTORY.to_owned(),
    ]);
    let mut ids = BTreeSet::new();
    for claim in claims {
        let id = text(claim, "claim_id")?;
        if !ids.insert(id) {
            return Err(SourceCommandError::Conflict("duplicate initial Claim"));
        }
        let form = form_name(id);
        names.insert(format!(".{form}.writer.lock"));
        names.insert(form);
    }
    Ok(names)
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
    const MAX_COMPLETE_BYTES: u64 = CLAIM_COMPLETE_BYTES;
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
    cut: Option<&CorpusCutReader>,
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    selected_package: Option<&BTreeMap<String, Vec<u8>>>,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
    serialize_creation: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    if let Some(budget) = &whole_call {
        let temporary = ctx
            .configuration_raw
            .len()
            .checked_add(ctx.request_raw.len())
            .ok_or(SourceCommandError::Unsupported(
                "Claim request/configuration state overflow",
            ))?;
        budget.borrow().check_live(temporary)?;
    }
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
        if let Some(budget) = &whole_call {
            budget.borrow().check_live(selected(ctx, source)?.len())?;
        }
        schema_check(
            executor,
            source,
            &json_file(ctx, source)?,
            contract,
            deadline,
            cancelled,
        )?;
    }
    let mut retained = None;
    if !create && !["describe", "inspect-version"].contains(&operation) {
        let id = text(&config, "claim_id")?;
        let current = rows(selected(ctx, p.as_str())?)?;
        let record = current
            .get(id)
            .ok_or(SourceCommandError::Conflict("delegated Claim absent"))?;
        if retained_profile_required(ctx, record)? {
            retained = Some(RetainedProfileRead::new(
                ctx,
                executor,
                deadline,
                cancelled,
                catalog_capture.clone(),
                whole_call.clone(),
            )?);
        }
    }
    let files = if let Some(selected_package) = selected_package {
        if create || selected_package.len() > 64 {
            return Err(SourceCommandError::Invalid("Claim selected package scope"));
        }
        let package_size = selected_package
            .values()
            .try_fold(0usize, |total, raw| {
                total.checked_add(raw.len()).filter(|sum| *sum <= 8_388_608)
            })
            .ok_or(SourceCommandError::Invalid(
                "Claim selected package byte budget",
            ))?;
        if let Some(read) = retained.as_ref() {
            if (whole_call.is_none() && package_size as u64 > read.remaining_read_bytes()?)
                || package_size > read.remaining_state_bytes()?
            {
                return Err(SourceCommandError::Unsupported(
                    "Claim package read or state budget",
                ));
            }
        } else if let Some(budget) = &whole_call {
            budget.borrow_mut().retain(package_size)?;
        }
        let parent = p
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim selected package parent"))?
            .0;
        let selected = ctx
            .files
            .iter()
            .filter_map(|file| {
                file.path
                    .as_str()
                    .strip_prefix(&format!("{parent}/"))
                    .filter(|name| !name.contains('/'))
                    .map(|name| (name, file.raw.as_slice()))
            })
            .collect::<BTreeMap<_, _>>();
        for (name, raw) in selected_package {
            if name.ends_with(".writer.lock") {
                if !raw.is_empty() {
                    return Err(SourceCommandError::Conflict(
                        "Claim operational lock changed",
                    ));
                }
            } else if selected.get(name.as_str()).copied() != Some(raw.as_slice()) {
                return Err(SourceCommandError::Conflict(
                    "Claim package differs from selected cut",
                ));
            }
        }
        if selected
            .keys()
            .any(|name| !selected_package.contains_key(*name))
        {
            return Err(SourceCommandError::Conflict(
                "Claim selected package member absent",
            ));
        }
        selected_package.clone()
    } else if let Some(read) = retained.as_ref() {
        package_bounded(
            ctx,
            &p,
            Some(read.remaining_read_bytes()?),
            Some(read.remaining_state_bytes()?),
        )?
    } else {
        package(ctx, &p)?
    };
    let package_bytes = files.values().try_fold(0usize, |sum, raw| {
        sum.checked_add(raw.len())
            .ok_or(SourceCommandError::Unsupported(
                "Claim package byte overflow",
            ))
    })?;
    if let Some(read) = retained.as_mut() {
        read.debit(
            if selected_package.is_some() && whole_call.is_some() {
                0
            } else {
                package_bytes as u64
            },
            package_bytes,
        )?;
    }
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
        let mut retained_profile_bindings = BTreeMap::new();
        let mut raw = Vec::new();
        for claim in claims {
            claim_scope(&config, claim, true, version)?;
            if retained.is_none() && retained_profile_required(ctx, claim)? {
                retained = Some(RetainedProfileRead::new(
                    ctx,
                    executor,
                    deadline,
                    cancelled,
                    catalog_capture.clone(),
                    whole_call.clone(),
                )?);
            }
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
            if let Some(binding) = validate_ground(
                ctx,
                cut,
                retained.as_mut(),
                &config,
                claim,
                version,
                executor,
                deadline,
                cancelled,
            )? {
                if let Some(read) = retained.as_mut() {
                    read.debit(0, retained_value_bytes(&binding.1)?)?;
                }
                retained_profile_bindings.insert(text(claim, "claim_id")?.to_owned(), binding);
            }
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
        let mut grounding = maintained_grounding(
            ctx,
            &config,
            claims,
            None,
            false,
            retained
                .take()
                .map(|read| read.into_inventory(ctx, cut))
                .transpose()?,
            &retained_profile_bindings,
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
        let changes = vec![SourceChange {
            path: p,
            before: None,
            after: Some(raw),
        }];
        account_retained_plan(&mut grounding, &response, &changes)?;
        return ctx.plan(&handler, response, changes, false);
    }
    let stream = files
        .get(CLAIM_STREAM)
        .ok_or(SourceCommandError::Conflict("Claim source package absent"))?;
    let records = rows(stream)?;
    let id = text(&config, "claim_id")?;
    let record = records
        .get(id)
        .ok_or(SourceCommandError::Conflict("delegated Claim absent"))?;
    let history = verify_claim_history_inner(
        ctx,
        &config,
        &p,
        &files,
        retained.as_mut(),
        None,
        0, // The live current package was already debited above.
        None,
    )?;
    if let Some(read) = retained.as_mut() {
        read.debit(0, retained_value_bytes(&history)?)?;
        let owner = VerifiedClaimOwner {
            history: history.clone(),
            revision: revision(&files)?,
            history_sha256: files
                .get(CLAIM_HISTORY)
                .map(|raw| Digest256::of_bytes(raw).to_prefixed()),
        };
        let cached = retained_value_bytes(&owner.history)?
            .checked_add(retained_value_bytes(&owner.revision)?)
            .and_then(|sum| sum.checked_add(p.as_str().len()))
            .and_then(|sum| sum.checked_add(std::mem::size_of::<VerifiedClaimOwner>()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim verified owner state overflow",
            ))?;
        read.debit(0, cached)?;
        read.claim_owners.insert(p.as_str().to_owned(), owner);
    }
    set(&mut response, "source", metadata_subject(record)?)?;
    set(&mut response, "revision", revision(&files)?)?;
    if operation != "claim.revise" {
        set(
            &mut response,
            "materializations",
            current_claim_materializations(record, &files, id, whole_call.as_ref())?,
        )?;
    }
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
            if retained.is_none() && retained_profile_required(ctx, record)? {
                retained = Some(RetainedProfileRead::new(
                    ctx,
                    executor,
                    deadline,
                    cancelled,
                    catalog_capture.clone(),
                    whole_call.clone(),
                )?);
            }
            validate_ground(
                ctx,
                cut,
                retained.as_mut(),
                &config,
                record,
                version,
                executor,
                deadline,
                cancelled,
            )?;
            let archived = if let Some(read) = retained.as_mut() {
                archive_accounted(ctx, &config, receipt, read, 0)?
            } else {
                archive(ctx, &config, receipt)?
            };
            let predecessors = rows(&archived[CLAIM_STREAM])?;
            let predecessor = predecessors.get(id).ok_or(SourceCommandError::Conflict(
                "Claim retry predecessor absent",
            ))?;
            let successor = advance_claim(predecessor, fields, layer)?;
            claim_scope(&config, &successor, false, version)?;
            validate_ground(
                ctx,
                cut,
                retained.as_mut(),
                &config,
                &successor,
                version,
                executor,
                deadline,
                cancelled,
            )?;
            if let Some(read) = retained.take() {
                // A retained revision replay has no fresh grounding call to
                // consume the read set; bind its historical observations to
                // this same complete selected cut before returning.
                read.into_inventory(ctx, cut)?;
            }
            set(
                &mut response,
                "materializations",
                current_claim_materializations(record, &files, id, whole_call.as_ref())?,
            )?;
            set(&mut response, "receipt", receipt.clone())?;
            set(&mut response, "replayed", JsonValue::Bool(true))?;
            return ctx.plan(&handler, response, vec![], true);
        }
    }
    let revised = advance_claim(record, fields, layer)?;
    claim_scope(&config, &revised, false, version)?;
    if retained.is_none() && retained_profile_required(ctx, &revised)? {
        retained = Some(RetainedProfileRead::new(
            ctx,
            executor,
            deadline,
            cancelled,
            catalog_capture.clone(),
            whole_call.clone(),
        )?);
    }
    let retained_profile_binding = validate_ground(
        ctx,
        cut,
        retained.as_mut(),
        &config,
        &revised,
        version,
        executor,
        deadline,
        cancelled,
    )?;
    if let (Some(read), Some((_, binding))) = (retained.as_mut(), retained_profile_binding.as_ref())
    {
        read.debit(0, retained_value_bytes(binding)?)?;
    }
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
    if operation == "prepare-revise" {
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
    } else {
        set(&mut response, "materializations", JsonValue::Array(views))?;
    }
    let mut retained_profile_bindings = BTreeMap::new();
    if let Some(binding) = retained_profile_binding {
        retained_profile_bindings.insert(id.to_owned(), binding);
    }
    let mut grounding = maintained_grounding(
        ctx,
        &config,
        std::slice::from_ref(&revised),
        Some(forms),
        true,
        retained
            .take()
            .map(|read| read.into_inventory(ctx, cut))
            .transpose()?,
        &retained_profile_bindings,
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
            (
                "dependencies",
                std::mem::replace(&mut grounding.dependencies, JsonValue::Null),
            ),
            (
                "source_bindings",
                std::mem::replace(&mut grounding.bindings, JsonValue::Null),
            ),
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
            if let Some(prior) = ctx.file(&path)? {
                if prior != raw {
                    return Err(SourceCommandError::Conflict(
                        "Claim predecessor archive path occupied",
                    ));
                }
                continue;
            }
            output.push(SourceChange {
                path,
                before: None,
                after: Some(raw.clone()),
            });
        }
        let manifest_path = path(&format!("{archive_path}/manifest.json"))?;
        let manifest_raw = published(&manifest)?;
        if let Some(prior) = ctx.file(&manifest_path)? {
            if prior != manifest_raw {
                return Err(SourceCommandError::Conflict(
                    "Claim predecessor manifest path occupied",
                ));
            }
        } else {
            output.push(SourceChange {
                path: manifest_path,
                before: None,
                after: Some(manifest_raw),
            });
        }
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
        let mut successor = files
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice()))
            .collect::<BTreeMap<_, _>>();
        let mut replaced = BTreeSet::new();
        let home_prefix = format!("{parent}/");
        for change in &output {
            if let Some(name) = change.path.as_str().strip_prefix(&home_prefix) {
                if name.contains('/') || !replaced.insert(name) {
                    return Err(SourceCommandError::Conflict(
                        "Claim successor package path differs",
                    ));
                }
                successor.insert(
                    name,
                    change.after.as_deref().ok_or(SourceCommandError::Conflict(
                        "Claim successor package member absent",
                    ))?,
                );
            }
        }
        if replaced != BTreeSet::from([CLAIM_STREAM, formname.as_str(), CLAIM_HISTORY]) {
            return Err(SourceCommandError::Conflict(
                "Claim successor package incomplete",
            ));
        }
        set(&mut response, "source", metadata_subject(&revised)?)?;
        let overlapping_state = output
            .iter()
            .try_fold(retained_value_bytes(&response)?, |state, change| {
                state
                    .checked_add(change.path.as_str().len())
                    .and_then(|state| state.checked_add(change.after.as_ref().map_or(0, Vec::len)))
                    .and_then(|state| state.checked_add(std::mem::size_of::<SourceChange>()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim revised result overlap overflow",
                    ))
            })?
            .checked_add(
                successor
                    .len()
                    .checked_mul(std::mem::size_of::<(&str, &[u8])>())
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim revised result overlap overflow",
                    ))?,
            )
            .and_then(|state| {
                state.checked_add(replaced.len().checked_mul(std::mem::size_of::<&str>())?)
            })
            .ok_or(SourceCommandError::Unsupported(
                "Claim revised result overlap overflow",
            ))?;
        set(
            &mut response,
            "revision",
            revised_package_revision(&successor, overlapping_state, whole_call.as_ref())?,
        )?;
        set(&mut response, "receipt", receipt)?;
        account_retained_plan(&mut grounding, &response, &output)?;
        return ctx.plan(&handler, response, output, false);
    }
    let parent = p.as_str().rsplit_once('/').unwrap().0;
    let changes = vec![
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
    ];
    account_retained_plan(&mut grounding, &response, &changes)?;
    ctx.plan(&handler, response, changes, false)
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
    retained_budget: Option<RetainedProfileRead>,
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
    claim_profile_inputs_selected(ctx, claim, &descriptor, inputs)
}
fn claim_profile_inputs_selected(
    ctx: &CommandContext,
    claim: &JsonValue,
    descriptor: &JsonValue,
    inputs: &mut JsonValue,
) -> SourceCommandResult<&'static str> {
    let reader = text(&descriptor, "reader")?;
    if reader.starts_with("identity-transition-")
        && !["identity-transition-v1", "identity-transition-v2"].contains(&reader)
        || descriptor
            .object_get("object_reference_set")
            .and_then(|v| v.object_get("basis_adapter"))
            .and_then(JsonValue::as_str)
            .is_some_and(|adapter| adapter != "collection-membership-versions-v1")
    {
        return Err(SourceCommandError::Unsupported(
            "declared retained Claim provenance bindings adapter",
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
    // SourceClaimProfiles treats both proposal readers as structured values:
    // their object bytes and the shared structured-value contract contribute
    // to the maintained grounding fingerprint independently of the endpoint
    // provenance returned by their exact historical reader.
    let structured = [
        "structured-value-v1",
        "structured-reference-value-v1",
        "identity-transition-v1",
        "identity-transition-v2",
    ]
    .contains(&reader);
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

fn retained_profile_required(ctx: &CommandContext, claim: &JsonValue) -> SourceCommandResult<bool> {
    let (_, descriptor) = profile(ctx, text(claim, "predicate")?)?;
    Ok(
        text(&descriptor, "reader")?.starts_with("identity-transition-")
            || descriptor
                .object_get("object_reference_set")
                .and_then(|value| value.object_get("basis_adapter"))
                .and_then(JsonValue::as_str)
                == Some("collection-membership-versions-v1"),
    )
}

// A borrowed description is both the construction recipe and its pre-copy
// bound. Adding a catalogue field necessarily adds it to the counted recipe.
enum CataloguePart<'a> {
    Value(&'a JsonValue),
    Text(Cow<'a, str>),
    Number(u64),
    Null,
    Links(&'a JsonValue),
    ReviewRefs(&'a JsonValue),
}
struct CatalogueEntry<'a>(Vec<(&'static str, CataloguePart<'a>)>);

fn catalogue_string_bound(value: &str) -> SourceCommandResult<usize> {
    // A UTF-8 byte contributes at most one six-byte JSON escape. This covers
    // quotes in valid RelativePaths without allocating a copied JsonString.
    value
        .len()
        .checked_mul(6)
        .and_then(|n| n.checked_add(2))
        .ok_or(SourceCommandError::Unsupported(
            "Claim catalogue string state overflow",
        ))
}

impl CatalogueEntry<'_> {
    fn bound(&self) -> SourceCommandResult<usize> {
        let mut bytes = 2usize;
        for (key, part) in &self.0 {
            bytes = bytes
                .checked_add(catalogue_string_bound(key)?)
                .and_then(|n| n.checked_add(2))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim catalogue state overflow",
                ))?;
            let value_bytes = match part {
                CataloguePart::Value(value) => retained_value_bytes(value)?,
                CataloguePart::Text(value) => catalogue_string_bound(value)?,
                CataloguePart::Number(_) => 20, // longest canonical u64
                CataloguePart::Null => 4,
                CataloguePart::Links(record) => {
                    let mut size = 2usize;
                    for key in CATALOG_LINK_FIELDS {
                        if let Some(value) = record.object_get(key) {
                            let value_bytes = retained_value_bytes(value)?;
                            size = size
                                .checked_add(catalogue_string_bound(key)?)
                                .and_then(|n| n.checked_add(2))
                                .and_then(|n| n.checked_add(value_bytes))
                                .ok_or(SourceCommandError::Unsupported(
                                    "Claim link state overflow",
                                ))?;
                        }
                    }
                    size
                }
                CataloguePart::ReviewRefs(claim) => {
                    let mut size = 2usize;
                    let reviews = claim
                        .object_get("reviews")
                        .map(|v| {
                            v.as_array()
                                .ok_or(SourceCommandError::Invalid("catalog Claim reviews array"))
                        })
                        .transpose()?
                        .unwrap_or(&[]);
                    for review in reviews {
                        if let Some(value) = review
                            .object_get("review_id")
                            .filter(|v| v.as_str().is_some())
                        {
                            size = size
                                .checked_add(retained_value_bytes(value)?)
                                .and_then(|n| n.checked_add(1))
                                .ok_or(SourceCommandError::Unsupported(
                                    "Claim review state overflow",
                                ))?;
                        }
                    }
                    size
                }
            };
            bytes = bytes
                .checked_add(value_bytes)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim catalogue state overflow",
                ))?;
        }
        bytes
            .checked_add(self.0.len().saturating_sub(1))
            .and_then(|n| {
                n.checked_add(
                    self.0
                        .len()
                        .checked_mul(std::mem::size_of::<(&str, CataloguePart<'_>)>())?,
                )
            })
            .ok_or(SourceCommandError::Unsupported(
                "Claim catalogue state overflow",
            ))
    }

    fn construct(self) -> SourceCommandResult<JsonValue> {
        let mut result = object(vec![]);
        for (key, part) in self.0 {
            let value = match part {
                CataloguePart::Value(value) => value.clone(),
                CataloguePart::Text(value) => string(value.as_ref()),
                CataloguePart::Number(value) => number(value),
                CataloguePart::Null => JsonValue::Null,
                CataloguePart::Links(record) => {
                    let mut links = object(vec![]);
                    for key in CATALOG_LINK_FIELDS {
                        if let Some(value) = record.object_get(key) {
                            set(&mut links, key, value.clone())?;
                        }
                    }
                    links
                }
                CataloguePart::ReviewRefs(claim) => {
                    let reviews = claim
                        .object_get("reviews")
                        .map(|v| {
                            v.as_array()
                                .ok_or(SourceCommandError::Invalid("catalog Claim reviews array"))
                        })
                        .transpose()?
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|v| v.object_get("review_id"))
                        .filter(|v| v.as_str().is_some())
                        .cloned()
                        .collect();
                    JsonValue::Array(reviews)
                }
            };
            set(&mut result, key, value)?;
        }
        Ok(result)
    }
}

fn catalogue_record<'a>(
    record: &'a JsonValue,
    location: &'a str,
    schema: Option<&'a str>,
) -> SourceCommandResult<CatalogueEntry<'a>> {
    let mut fields = vec![
        (
            "schema_version",
            CataloguePart::Text(Cow::Borrowed("tos_source_witness_catalog_entry_v1")),
        ),
        (
            "record_id",
            CataloguePart::Value(field(record, "record_id")?),
        ),
        (
            "record_type",
            CataloguePart::Value(field(record, "record_type")?),
        ),
        (
            "preferred_label",
            record
                .object_get("preferred_label")
                .map(CataloguePart::Value)
                .unwrap_or(CataloguePart::Text(Cow::Borrowed(""))),
        ),
        (
            "identity_status",
            record
                .object_get("identity_status")
                .map(CataloguePart::Value)
                .unwrap_or(CataloguePart::Text(Cow::Borrowed(""))),
        ),
        (
            "source_record_ref",
            CataloguePart::Text(Cow::Borrowed(location)),
        ),
        (
            "record_sha256",
            CataloguePart::Text(Cow::Owned(record_digest(record)?.to_hex())),
        ),
    ];
    if let Some(schema) = schema {
        fields.push((
            "source_schema_ref",
            CataloguePart::Text(Cow::Borrowed(schema)),
        ));
    }
    fields.push(("links", CataloguePart::Links(record)));
    Ok(CatalogueEntry(fields))
}

fn catalogue_claim<'a>(
    ctx: &CommandContext,
    claim: &'a JsonValue,
    location: &'a str,
    line: usize,
    profiled: bool,
    selected_schema: Option<&'a str>,
) -> SourceCommandResult<CatalogueEntry<'a>> {
    let mut fields = vec![
        (
            "schema_version",
            CataloguePart::Text(Cow::Borrowed("tos_source_witness_claim_catalog_entry_v1")),
        ),
        (
            "source_claim_file_ref",
            CataloguePart::Text(Cow::Borrowed(location)),
        ),
        ("source_claim_line", CataloguePart::Number(line as u64)),
        (
            "claim_sha256",
            CataloguePart::Text(Cow::Owned(record_digest(claim)?.to_hex())),
        ),
    ];
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
        fields.push((
            key,
            claim
                .object_get(key)
                .map(CataloguePart::Value)
                .unwrap_or(CataloguePart::Null),
        ));
    }
    fields.push(("review_refs", CataloguePart::ReviewRefs(claim)));
    for key in ["supersedes_claim_ref", "qualifiers"] {
        if let Some(value) = claim.object_get(key) {
            fields.push((key, CataloguePart::Value(value)));
        }
    }
    if text(claim, "schema_version")? == "tos_historical_claim_v1" {
        fields.push((
            "source_schema_ref",
            CataloguePart::Text(Cow::Borrowed("ToS/contracts/historical-claim.schema.json")),
        ));
    }
    if profiled {
        let schema = if let Some(schema) = selected_schema {
            Cow::Borrowed(schema)
        } else {
            let (_, descriptor) = profile(ctx, text(claim, "predicate")?)?;
            let route = array(&descriptor, "schemas")?
                .iter()
                .find(|v| v.object_get("schema_version") == claim.object_get("schema_version"))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim catalogue schema route",
                ))?;
            Cow::Owned(text(route, "schema_ref")?.to_owned())
        };
        fields.retain(|(key, _)| *key != "source_schema_ref");
        fields.push(("source_schema_ref", CataloguePart::Text(schema)));
    }
    Ok(CatalogueEntry(fields))
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
    profile_entities: Option<JsonValue>,
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

type MetadataBinding = (JsonValue, String, JsonValue, JsonValue, &'static str);

struct RetainedCatalogRows {
    by_id: BTreeMap<String, (usize, usize, u64)>,
    sha256: String,
}

#[derive(Clone)]
struct VerifiedClaimOwner {
    history: JsonValue,
    revision: JsonValue,
    history_sha256: Option<String>,
}

/// One selected Claim invocation owns its complete identity index and the
/// retained reads derived from it. None of these observations grants use.
struct RetainedProfileRead {
    inventory: Option<MaintainedInventory>,
    entities: JsonValue,
    catalogs: BTreeMap<String, RetainedCatalogRows>,
    metadata: BTreeMap<String, MetadataBinding>,
    claims: BTreeMap<String, ResolvedClaimReference>,
    claim_owners: BTreeMap<String, VerifiedClaimOwner>,
    catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
    observed_reads: Vec<PredicateRead>,
    read_bytes: u64,
    state_bytes: usize,
    whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
}

struct RetainedGroundingInput {
    inventory: MaintainedInventory,
    budget: RetainedProfileRead,
}

fn retained_value_bytes(value: &JsonValue) -> SourceCommandResult<usize> {
    canonical_count_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: CLAIM_COMPLETE_BYTES as usize,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Unsupported("Claim retained logical state budget"))
}

impl RetainedProfileRead {
    fn new(
        ctx: &CommandContext,
        executor: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
        catalog_capture: Option<Rc<RefCell<ClaimCatalogCapture>>>,
        whole_call: Option<Rc<RefCell<ClaimCallBudget>>>,
    ) -> SourceCommandResult<Self> {
        ctx.check()?;
        let mut inventory = maintained_inventory_inner(
            ctx,
            None,
            false,
            executor,
            deadline,
            cancelled,
            whole_call.as_ref(),
        )?;
        let entities = if whole_call.is_some() {
            inventory
                .profile_entities
                .take()
                .ok_or(SourceCommandError::Conflict(
                    "Claim retained profile registry absent",
                ))?
        } else {
            json_file(ctx, ENTITIES)?
        };
        let raw_bytes = ctx
            .files
            .iter()
            .try_fold(0usize, |sum, file| sum.checked_add(file.raw.len()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim complete input state overflow",
            ))?;
        let captured_control_bytes = catalog_capture
            .as_ref()
            .and_then(|capture| capture.borrow().control().map(<[u8]>::len))
            .unwrap_or(0);
        let control_bytes_to_add = if whole_call.is_some() {
            0
        } else {
            captured_control_bytes
        };
        let mut state_bytes = raw_bytes
            .checked_add(control_bytes_to_add)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained control state overflow",
            ))?
            .checked_add(retained_value_bytes(&entities)?)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained inventory state overflow",
            ))?;
        for value in [
            &inventory.records,
            &inventory.record_inputs,
            &inventory.claim_profile_inputs,
            &inventory.events,
            &inventory.anchors,
        ] {
            state_bytes = state_bytes
                .checked_add(retained_value_bytes(value)?)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained inventory state overflow",
                ))?;
        }
        for map in [
            &inventory.objects,
            &inventory.source_records,
            &inventory.claims,
            &inventory.record_member_inputs,
        ] {
            for (name, value) in map {
                let value_bytes = retained_value_bytes(value)?;
                state_bytes = state_bytes
                    .checked_add(name.len() + std::mem::size_of::<(String, JsonValue)>())
                    .and_then(|n| n.checked_add(value_bytes))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim retained inventory state overflow",
                    ))?;
            }
        }
        for snapshot in [
            inventory.native_identity_snapshot.as_deref(),
            inventory.native_text_snapshot.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            state_bytes = state_bytes
                .checked_add(snapshot.len() + std::mem::size_of::<String>())
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained inventory state overflow",
                ))?;
        }
        if let Some(budget) = &whole_call {
            let budget = budget.borrow();
            state_bytes = state_bytes
                .checked_add(budget.live_state_bytes.checked_sub(raw_bytes).ok_or(
                    SourceCommandError::Unsupported("Claim whole-call state basis"),
                )?)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim whole-call state overflow",
                ))?;
        }
        if state_bytes > CLAIM_COMPLETE_BYTES as usize {
            return Err(SourceCommandError::Unsupported(
                "Claim retained inventory state budget",
            ));
        }
        let read_bytes = if let Some(budget) = &whole_call {
            budget.borrow().read_bytes
        } else {
            (raw_bytes as u64)
                .checked_add(captured_control_bytes as u64)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained control read overflow",
                ))?
        };
        if let Some(budget) = &whole_call {
            let mut budget = budget.borrow_mut();
            let additional = state_bytes.checked_sub(budget.live_state_bytes).ok_or(
                SourceCommandError::Unsupported("Claim retained state basis changed"),
            )?;
            budget.retain(additional)?;
        }
        Ok(Self {
            inventory: Some(inventory),
            entities,
            catalogs: BTreeMap::new(),
            metadata: BTreeMap::new(),
            claims: BTreeMap::new(),
            claim_owners: BTreeMap::new(),
            catalog_capture,
            observed_reads: Vec::new(),
            read_bytes,
            state_bytes,
            whole_call,
        })
    }

    fn debit(&mut self, bytes_read: u64, state_bytes: usize) -> SourceCommandResult<()> {
        if let Some(budget) = &self.whole_call {
            let mut budget = budget.borrow_mut();
            budget.read(bytes_read)?;
            budget.retain(state_bytes)?;
            self.read_bytes = budget.read_bytes;
            self.state_bytes = budget.live_state_bytes;
            return Ok(());
        }
        self.read_bytes = self
            .read_bytes
            .checked_add(bytes_read)
            .filter(|n| *n <= CLAIM_COMPLETE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained read budget",
            ))?;
        self.state_bytes = self
            .state_bytes
            .checked_add(state_bytes)
            .filter(|n| *n <= CLAIM_COMPLETE_BYTES as usize)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained state budget",
            ))?;
        Ok(())
    }

    fn remaining_read_bytes(&self) -> SourceCommandResult<u64> {
        if let Some(budget) = &self.whole_call {
            return budget.borrow().remaining_read();
        }
        CLAIM_COMPLETE_BYTES
            .checked_sub(self.read_bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained read budget",
            ))
    }

    fn remaining_state_bytes(&self) -> SourceCommandResult<usize> {
        if let Some(budget) = &self.whole_call {
            return budget.borrow().remaining_live();
        }
        (CLAIM_COMPLETE_BYTES as usize)
            .checked_sub(self.state_bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained state budget",
            ))
    }

    fn check_temporary_state(&self, bytes: usize) -> SourceCommandResult<()> {
        if bytes > self.remaining_state_bytes()? {
            return Err(SourceCommandError::Unsupported(
                "Claim retained temporary state budget",
            ));
        }
        Ok(())
    }

    fn claim_owner(
        &mut self,
        ctx: &CommandContext,
        config: &JsonValue,
        p: &RelativePath,
    ) -> SourceCommandResult<VerifiedClaimOwner> {
        if let Some(owner) = self.claim_owners.get(p.as_str()) {
            self.check_temporary_state(retained_value_bytes(&owner.history)?)?;
            return Ok(owner.clone());
        }
        let files = package_bounded(
            ctx,
            p,
            Some(self.remaining_read_bytes()?),
            Some(self.remaining_state_bytes()?),
        )?;
        let package_bytes = files.values().try_fold(0usize, |sum, raw| {
            sum.checked_add(raw.len())
                .ok_or(SourceCommandError::Unsupported(
                    "Claim package byte overflow",
                ))
        })?;
        self.debit(package_bytes as u64, 0)?;
        let history = verify_claim_history_inner(
            ctx,
            config,
            p,
            &files,
            Some(self),
            None,
            package_bytes,
            None,
        )?;
        let owner = VerifiedClaimOwner {
            revision: revision(&files)?,
            history_sha256: files
                .get(CLAIM_HISTORY)
                .map(|raw| Digest256::of_bytes(raw).to_prefixed()),
            history,
        };
        let state = retained_value_bytes(&owner.history)?
            .checked_add(retained_value_bytes(&owner.revision)?)
            .and_then(|sum| sum.checked_add(p.as_str().len()))
            .and_then(|sum| sum.checked_add(std::mem::size_of::<VerifiedClaimOwner>()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim verified owner state overflow",
            ))?;
        self.check_temporary_state(package_bytes.checked_add(state).ok_or(
            SourceCommandError::Unsupported("Claim verified owner state overflow"),
        )?)?;
        self.debit(0, state)?;
        self.claim_owners
            .insert(p.as_str().to_owned(), owner.clone());
        Ok(owner)
    }

    fn inventory(&self) -> SourceCommandResult<&MaintainedInventory> {
        self.inventory.as_ref().ok_or(SourceCommandError::Conflict(
            "Claim retained inventory released",
        ))
    }

    fn into_inventory(
        mut self,
        ctx: &CommandContext,
        cut: Option<&CorpusCutReader>,
    ) -> SourceCommandResult<RetainedGroundingInput> {
        let cut = cut.ok_or(SourceCommandError::Unsupported(
            "retained Claim needs complete selected cut",
        ))?;
        if cut.current().revision() != ctx.base_revision {
            return Err(SourceCommandError::Conflict(
                "retained Claim selected cut changed",
            ));
        }
        let mut recheck_bytes = 0u64;
        for read in &self.observed_reads {
            match read {
                PredicateRead::ExactPath {
                    path: location,
                    digest,
                } => {
                    let raw = selected(ctx, location)?;
                    if raw.len() as u64 > self.remaining_read_bytes()?.saturating_sub(recheck_bytes)
                    {
                        return Err(SourceCommandError::Unsupported(
                            "retained Claim read recheck budget",
                        ));
                    }
                    if Digest256::of_bytes(raw).to_prefixed() != *digest {
                        return Err(SourceCommandError::Conflict(
                            "retained Claim exact path read differs from complete cut",
                        ));
                    }
                    recheck_bytes = recheck_bytes.checked_add(raw.len() as u64).ok_or(
                        SourceCommandError::Unsupported("retained Claim read recheck overflow"),
                    )?;
                }
                _ => {
                    return Err(SourceCommandError::Unsupported(
                        "retained Claim predicate read outside selected path closure",
                    ));
                }
            }
        }
        self.debit(recheck_bytes, 0)?;
        let inventory = self.inventory.take().ok_or(SourceCommandError::Conflict(
            "Claim retained inventory released",
        ))?;
        Ok(RetainedGroundingInput {
            inventory,
            budget: self,
        })
    }
}
pub(crate) fn maintained_inventory(
    ctx: &CommandContext,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<MaintainedInventory> {
    maintained_inventory_inner(ctx, None, false, executor, deadline, cancelled, None)
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
    let complete = claim_context_with_files(ctx, cut.current().revision(), files);
    complete.check()?;
    maintained_inventory_inner(
        &complete,
        Some(cut),
        false,
        executor,
        deadline,
        cancelled,
        None,
    )
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
        None,
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

fn inventory_live_preflight(
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
    retained: usize,
    additional: usize,
) -> SourceCommandResult<()> {
    if let Some(budget) = whole_call {
        budget
            .borrow()
            .check_live(retained.checked_add(additional).ok_or(
                SourceCommandError::Unsupported("Claim inventory state overflow"),
            )?)?;
    }
    Ok(())
}

fn maintained_inventory_inner(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    capture_member_inputs: bool,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
) -> SourceCommandResult<MaintainedInventory> {
    let registry_raw = selected(ctx, ENTITIES)?;
    inventory_live_preflight(
        whole_call,
        0,
        registry_raw
            .len()
            .checked_mul(3)
            .ok_or(SourceCommandError::Unsupported(
                "Claim selected registry state overflow",
            ))?,
    )?;
    let entities = crate::source_revisions::validate_source_profile_registry(
        executor, deadline, cancelled, ctx,
    )?;
    let mut retained_inventory_state = retained_value_bytes(&entities)?;
    inventory_live_preflight(whole_call, 0, retained_inventory_state)?;
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
    let native_identity_state = native_identities
        .iter()
        .try_fold(0usize, |sum, (id, refs)| {
            let refs_state = refs.iter().try_fold(0usize, |sum, reference| {
                sum.checked_add(reference.len())
                    .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
            })?;
            sum.checked_add(id.len())
                .and_then(|n| n.checked_add(refs_state))
                .and_then(|n| n.checked_add(std::mem::size_of::<(String, Vec<String>)>()))
        })
        .and_then(|sum| sum.checked_add(native_identity_snapshot.as_ref().map_or(0, String::len)))
        .ok_or(SourceCommandError::Unsupported(
            "Claim native identity state overflow",
        ))?;
    retained_inventory_state = retained_inventory_state
        .checked_add(native_identity_state)
        .ok_or(SourceCommandError::Unsupported(
            "Claim native identity state overflow",
        ))?;
    inventory_live_preflight(whole_call, retained_inventory_state, 0)?;
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
    let kinds_state = kinds
        .iter()
        .try_fold(0usize, |sum, (kind, name)| {
            sum.checked_add(kind.len())
                .and_then(|n| n.checked_add(name.len()))
                .and_then(|n| n.checked_add(std::mem::size_of::<(String, String)>()))
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim catalogue kind state overflow",
        ))?;
    retained_inventory_state = retained_inventory_state.checked_add(kinds_state).ok_or(
        SourceCommandError::Unsupported("Claim catalogue kind state overflow"),
    )?;
    inventory_live_preflight(whole_call, retained_inventory_state, 0)?;
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
    let initial_digest_state = registry_refs
        .iter()
        .chain(claim_registry_refs.iter())
        .try_fold(0usize, |sum, name| {
            sum.checked_add(name.len())?
                .checked_add(64 + 9 + std::mem::size_of::<(JsonString, JsonValue)>())
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim registry digest state overflow",
        ))?;
    inventory_live_preflight(whole_call, retained_inventory_state, initial_digest_state)?;
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
    let inventory_index_state = ctx
        .files
        .len()
        .checked_mul(std::mem::size_of::<&SourceFile>())
        .ok_or(SourceCommandError::Unsupported(
            "Claim inventory path index state overflow",
        ))?;
    retained_inventory_state = retained_inventory_state
        .checked_add(initial_digest_state)
        .and_then(|sum| sum.checked_add(inventory_index_state))
        .ok_or(SourceCommandError::Unsupported(
            "Claim inventory path index state overflow",
        ))?;
    inventory_live_preflight(whole_call, retained_inventory_state, 0)?;
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
            // Native witnesses retain their own identity/schema; the compiler's
            // pure renderer supplies the exact Python catalog shape.
            if file.raw.len() > 1_048_576 {
                return Err(SourceCommandError::Invalid("native witness metadata byte budget"));
            }
            let upper = file.raw.len().checked_mul(32).and_then(|n| n.checked_add(65_536))
                .ok_or(SourceCommandError::Unsupported("native witness inventory state overflow"))?;
            inventory_live_preflight(whole_call, retained_inventory_state, upper)?;
            let record = parse(&file.raw)?;
            let (kind, identity, schema_ref) = match (basename, text(&record, "schema_version")?) {
                ("artifact-witness.json", "tos_artifact_source_witness_v1") => ("artifact", "artifact_id", "ToS/contracts/artifact-source-witness.schema.json"),
                ("artifact-witness.json", "tos_artifact_source_witness_v2") => ("artifact", "artifact_id", "ToS/contracts/artifact-source-witness-v2.schema.json"),
                ("composite-witness.json", "tos_scholarly_composite_witness_v1") => ("composite", "composite_id", "ToS/contracts/scholarly-composite-witness.schema.json"),
                _ => return Err(SourceCommandError::Denied("native witness exact schema route")),
            };
            let subtree = if kind == "artifact" { "artifacts" } else { "scholarly-composites" };
            if !location.starts_with(&format!("ToS/source-witnesses/{subtree}/")) {
                return Err(SourceCommandError::Denied("native witness owner subtree"));
            }
            crate::source_revisions::schema(executor, deadline, cancelled, ctx, &[schema_ref.into()], schema_ref, &record)?;
            let id = text(&record, identity)?.to_owned();
            if id.is_empty() || native_identities.contains_key(&id) {
                return Err(SourceCommandError::Conflict("native witness identity reserved or empty"));
            }
            let rendered_source: serde_json::Value = serde_json::from_slice(&canonical(&record)?)
                .map_err(|_| SourceCommandError::Invalid("native witness renderer input"))?;
            let rendered = tos_compiler::source_witness_catalog::render_catalog_record(
                &rendered_source, location, Some(schema_ref), 8_388_608)
                .map_err(|_| SourceCommandError::Invalid("native witness catalog rendering"))?;
            let entry = parse(&serde_json::to_vec(&rendered).map_err(|_| SourceCommandError::Invalid("native witness renderer output"))?)?;
            let added = retained_value_bytes(&record)?.checked_add(retained_value_bytes(&entry)?.checked_mul(3)
                .ok_or(SourceCommandError::Unsupported("native witness inventory state overflow"))?)
                .and_then(|n| n.checked_add(id.len().saturating_mul(2)))
                .and_then(|n| n.checked_add(location.len().saturating_mul(2)))
                .and_then(|n| n.checked_add(3 * std::mem::size_of::<(String, JsonValue)>()))
                .ok_or(SourceCommandError::Unsupported("native witness inventory state overflow"))?;
            inventory_live_preflight(whole_call, retained_inventory_state, added)?;
            retained_inventory_state = retained_inventory_state.checked_add(added)
                .ok_or(SourceCommandError::Unsupported("native witness inventory state overflow"))?;
            if objects.insert(id.clone(), entry.clone()).is_some() {
                return Err(SourceCommandError::Conflict("complete catalog has duplicate metadata identity"));
            }
            if capture_member_inputs {
                record_member_inputs.insert(location.to_owned(), raw_digests(ctx, &[schema_ref], false)?);
            }
            source_records.insert(id, record);
            records.entry(kind.into()).or_default().push(entry);
            continue;
        }
        if let Some((kind, _)) = kinds.iter().find(|(_, name)| name.as_str() == basename) {
            inventory_live_preflight(whole_call, retained_inventory_state, file.raw.len())?;
            let record = parse(&file.raw)?;
            let record_state = retained_value_bytes(&record)?;
            inventory_live_preflight(whole_call, retained_inventory_state, record_state)?;
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
                // The selected resolver returns the same current record and
                // its exact subject while this parsed record is still live.
                inventory_live_preflight(
                    whole_call,
                    retained_inventory_state,
                    record_state
                        .checked_mul(3)
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim profile resolution state overflow",
                        ))?,
                )?;
                if cut.is_none()
                    && (descriptor.object_get("native_binding_adapter").is_some()
                        || record.object_get("native_text_binding").is_some())
                {
                    return Err(SourceCommandError::Unsupported(
                        "maintained native text binding snapshot adapter",
                    ));
                }
                if descriptor.object_get("native_binding_adapter").is_some() {
                    let binding = field(&record, "native_text_binding")?;
                    let binding_state = retained_value_bytes(binding)?
                        .checked_add(std::mem::size_of::<JsonValue>())
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim native binding state overflow",
                        ))?;
                    inventory_live_preflight(whole_call, retained_inventory_state, binding_state)?;
                    native_bindings.push(binding.clone());
                    retained_inventory_state =
                        retained_inventory_state.checked_add(binding_state).ok_or(
                            SourceCommandError::Unsupported("Claim native binding state overflow"),
                        )?;
                }
                let route = array(descriptor, "schemas")?
                    .iter()
                    .find(|v| v.object_get("schema_version") == record.object_get("schema_version"))
                    .ok_or(SourceCommandError::Unsupported(
                        "catalog metadata schema route",
                    ))?;
                let route_input_state = array(route, "schema_dependencies")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or(SourceCommandError::Invalid("schema dependency"))
                    })
                    .chain([
                        Ok("ToS/contracts/corpus-record.schema.json"),
                        text(route, "schema_ref"),
                    ])
                    .try_fold(0usize, |sum, name| {
                        sum.checked_add(name?.len())
                            .and_then(|sum| sum.checked_add(64 + 9))
                            .ok_or(SourceCommandError::Unsupported(
                                "Claim schema route state overflow",
                            ))
                    })?;
                inventory_live_preflight(
                    whole_call,
                    retained_inventory_state,
                    record_state
                        .checked_mul(3)
                        .and_then(|sum| sum.checked_add(initial_digest_state))
                        .and_then(|sum| sum.checked_add(route_input_state))
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim schema route state overflow",
                        ))?,
                )?;
                let inputs_before = retained_value_bytes(&record_inputs)?;
                include_route(
                    ctx,
                    &mut record_inputs,
                    route,
                    &["ToS/contracts/corpus-record.schema.json"],
                )?;
                let inputs_after = retained_value_bytes(&record_inputs)?;
                retained_inventory_state = retained_inventory_state
                    .checked_add(inputs_after.saturating_sub(inputs_before))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim schema route state overflow",
                    ))?;
                if let Some(inputs) = &mut member_inputs {
                    include_route(
                        ctx,
                        inputs,
                        route,
                        &["ToS/contracts/corpus-record.schema.json"],
                    )?;
                }
                let exact = metadata_subject(&record)?;
                let (verified, locator) = if let Some(budget) = whole_call {
                    let remaining_read = budget.borrow().remaining_read()?;
                    // The inventory and this parsed owner remain live while the
                    // nested history/archive reader allocates its own package.
                    // They are not yet retained in ClaimCallBudget: that debit
                    // happens when maintained_inventory_inner returns.
                    let exact_state = retained_value_bytes(&exact)?;
                    let member_inputs_state = member_inputs
                        .as_ref()
                        .map(retained_value_bytes)
                        .transpose()?
                        .unwrap_or(0);
                    let local_state = retained_inventory_state
                        .checked_add(record_state)
                        .and_then(|sum| sum.checked_add(exact_state))
                        .and_then(|sum| sum.checked_add(id.len()))
                        .and_then(|sum| sum.checked_add(route_input_state))
                        .and_then(|sum| sum.checked_add(member_inputs_state))
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim profile resolution state overflow",
                        ))?;
                    let remaining_state = budget
                        .borrow()
                        .remaining_live()?
                        .checked_sub(local_state)
                        .filter(|bytes| *bytes > 0)
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim profile resolution state budget",
                        ))?;
                    let resolved =
                        crate::source_revisions::resolve_record_version_evidence_at_selected(
                            ctx,
                            location,
                            tos_validation::item_rules::ItemLimits {
                                max_member_bytes: 8_388_608usize.min(remaining_read as usize),
                                max_total_bytes: remaining_read,
                                max_state_bytes: remaining_state,
                                max_issues: 256,
                                deadline,
                            },
                            &exact,
                            executor,
                            deadline,
                            cancelled,
                        )?;
                    let mut budget = budget.borrow_mut();
                    budget.read(resolved.bytes_read)?;
                    // Older non-Collection resolvers report zero retained
                    // state. Their record, route and historical evidence are
                    // nevertheless all live at this boundary; conservatively
                    // retain the entire returned value graph for this call.
                    let returned_values = [
                        Some(&resolved.record),
                        resolved.current_record.as_ref(),
                        Some(&resolved.current_ref),
                        Some(&resolved.source),
                        Some(&resolved.history),
                        Some(&resolved.transition),
                        Some(&resolved.route_profile),
                    ]
                    .into_iter()
                    .flatten()
                    .try_fold(0usize, |sum, value| {
                        sum.checked_add(retained_value_bytes(value)?).ok_or(
                            SourceCommandError::Unsupported(
                                "Claim profile returned state overflow",
                            ),
                        )
                    })?;
                    let returned_state = returned_values
                        .checked_add(resolved.source_path.len())
                        .and_then(|sum| {
                            sum.checked_add(
                                resolved
                                    .reads
                                    .len()
                                    .checked_mul(std::mem::size_of::<PredicateRead>())?,
                            )
                        })
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim profile returned state overflow",
                        ))?
                        .max(resolved.returned_state_bytes);
                    budget.retain(returned_state)?;
                    (resolved.record, resolved.source_path)
                } else if let Some(cut) = cut {
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
            let entry_plan = catalogue_record(&record, location, schema)?;
            let entry_bound = entry_plan.bound()?;
            let upper_added =
                record_state
                    .checked_add(entry_bound.checked_mul(3).ok_or(
                        SourceCommandError::Unsupported("Claim catalogue entry state overflow"),
                    )?)
                    .and_then(|sum| sum.checked_add(id.len().saturating_mul(2)))
                    .and_then(|sum| sum.checked_add(location.len().saturating_mul(2)))
                    .and_then(|sum| sum.checked_add(3 * std::mem::size_of::<(String, JsonValue)>()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim catalog record state overflow",
                    ))?;
            inventory_live_preflight(whole_call, retained_inventory_state, upper_added)?;
            let entry = entry_plan.construct()?;
            let entry_state = retained_value_bytes(&entry)?;
            let added =
                record_state
                    .checked_add(entry_state.checked_mul(3).ok_or(
                        SourceCommandError::Unsupported("Claim catalog entry state overflow"),
                    )?)
                    .and_then(|sum| sum.checked_add(id.len().saturating_mul(2)))
                    .and_then(|sum| sum.checked_add(location.len().saturating_mul(2)))
                    .and_then(|sum| sum.checked_add(3 * std::mem::size_of::<(String, JsonValue)>()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim catalog record state overflow",
                    ))?;
            inventory_live_preflight(whole_call, retained_inventory_state, added)?;
            retained_inventory_state = retained_inventory_state.checked_add(added).ok_or(
                SourceCommandError::Unsupported("Claim catalog record state overflow"),
            )?;
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
                inventory_live_preflight(whole_call, retained_inventory_state, raw.len())?;
                let claim = parse(raw)?;
                let claim_state = retained_value_bytes(&claim)?;
                inventory_live_preflight(whole_call, retained_inventory_state, claim_state)?;
                if !["public", "public_metadata_only"].contains(&text(&claim, "visibility")?) {
                    return Err(SourceCommandError::Denied("catalog Claim visibility"));
                }
                let mut selected_schema = None;
                let mut schema_bytes =
                    if text(&claim, "schema_version")? == "tos_historical_claim_v1" {
                        "ToS/contracts/historical-claim.schema.json".len()
                    } else {
                        0
                    };
                if profiled {
                    let registry_bytes = selected(ctx, RELATIONS)?.len();
                    inventory_live_preflight(
                        whole_call,
                        retained_inventory_state,
                        claim_state
                            .checked_add(registry_bytes.checked_mul(3).ok_or(
                                SourceCommandError::Unsupported(
                                    "Claim profile registry state overflow",
                                ),
                            )?)
                            .ok_or(SourceCommandError::Unsupported(
                                "Claim profile registry state overflow",
                            ))?,
                    )?;
                    let (_, descriptor) = profile(ctx, text(&claim, "predicate")?)?;
                    let route = array(&descriptor, "schemas")?
                        .iter()
                        .find(|v| {
                            v.object_get("schema_version") == claim.object_get("schema_version")
                        })
                        .ok_or(SourceCommandError::Unsupported(
                            "existing Claim schema route",
                        ))?;
                    let mut route_refs = array(route, "schema_dependencies")?
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .ok_or(SourceCommandError::Invalid("schema dependency"))
                        })
                        .chain([text(route, "schema_ref")]);
                    let route_digest_state = route_refs.try_fold(0usize, |sum, name| {
                        sum.checked_add(name?.len())
                            .and_then(|sum| sum.checked_add(64 + 9))
                            .ok_or(SourceCommandError::Unsupported(
                                "Claim profile route state overflow",
                            ))
                    })?;
                    let fixed_digest_state = [
                        "ToS/contracts/claim-packet.schema.json",
                        "ToS/contracts/knowledge-assessment.schema.json",
                        "ToS/contracts/source-claim-record.schema.json",
                        "ToS/contracts/historical-claim.schema.json",
                        "ToS/contracts/corpus-record.schema.json",
                        "ToS/contracts/source-structured-value.schema.json",
                        "ToS/contracts/scoped-member-structure.schema.json",
                        "ToS/contracts/claim-display-fields.schema.json",
                    ]
                    .iter()
                    .try_fold(0usize, |sum, name| {
                        sum.checked_add(name.len())?.checked_add(64 + 9)
                    })
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim profile route state overflow",
                    ))?;
                    inventory_live_preflight(
                        whole_call,
                        retained_inventory_state,
                        claim_state
                            .checked_add(registry_bytes.checked_mul(3).ok_or(
                                SourceCommandError::Unsupported(
                                    "Claim profile registry state overflow",
                                ),
                            )?)
                            .and_then(|sum| sum.checked_add(route_digest_state))
                            .and_then(|sum| sum.checked_add(fixed_digest_state))
                            .ok_or(SourceCommandError::Unsupported(
                                "Claim profile route state overflow",
                            ))?,
                    )?;
                    let inputs_before = retained_value_bytes(&prior_profile_inputs)?;
                    claim_profile_inputs_selected(
                        ctx,
                        &claim,
                        &descriptor,
                        &mut prior_profile_inputs,
                    )?;
                    let inputs_after = retained_value_bytes(&prior_profile_inputs)?;
                    retained_inventory_state = retained_inventory_state
                        .checked_add(inputs_after.saturating_sub(inputs_before))
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim profile route state overflow",
                        ))?;
                    schema_bytes = text(route, "schema_ref")?.len();
                    inventory_live_preflight(
                        whole_call,
                        retained_inventory_state,
                        claim_state.checked_add(schema_bytes).ok_or(
                            SourceCommandError::Unsupported("Claim schema path state overflow"),
                        )?,
                    )?;
                    selected_schema = Some(text(route, "schema_ref")?.to_owned());
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
                let entry_plan = catalogue_claim(
                    ctx,
                    &claim,
                    location,
                    index + 1,
                    profiled,
                    selected_schema.as_deref(),
                )?;
                let entry_bound = entry_plan.bound()?;
                let entry_and_claim = claim_state
                    .checked_add(entry_bound)
                    .and_then(|sum| {
                        sum.checked_add(selected_schema.as_ref().map_or(0, String::len))
                    })
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim catalogue Claim state overflow",
                    ))?;
                inventory_live_preflight(whole_call, retained_inventory_state, entry_and_claim)?;
                let entry = entry_plan.construct()?;
                let added = retained_value_bytes(&entry)?
                    .checked_add(id.len())
                    .and_then(|sum| sum.checked_add(std::mem::size_of::<(String, JsonValue)>()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim catalog Claim state overflow",
                    ))?;
                inventory_live_preflight(
                    whole_call,
                    retained_inventory_state,
                    claim_state
                        .checked_add(added)
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim catalog Claim state overflow",
                        ))?,
                )?;
                retained_inventory_state = retained_inventory_state.checked_add(added).ok_or(
                    SourceCommandError::Unsupported("Claim catalog Claim state overflow"),
                )?;
                if prior.insert(id, entry).is_some() {
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
                inventory_live_preflight(whole_call, retained_inventory_state, raw.len())?;
                let payload = parse(raw)?;
                inventory_live_preflight(
                    whole_call,
                    retained_inventory_state,
                    retained_value_bytes(&payload)?,
                )?;
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
                let added = retained_value_bytes(&payload)?
                    .checked_mul(2)
                    .and_then(|sum| sum.checked_add(id.len() + location.len()))
                    .and_then(|sum| sum.checked_add(std::mem::size_of::<(JsonString, JsonValue)>()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim evidence index state overflow",
                    ))?;
                inventory_live_preflight(whole_call, retained_inventory_state, added)?;
                retained_inventory_state = retained_inventory_state.checked_add(added).ok_or(
                    SourceCommandError::Unsupported("Claim evidence index state overflow"),
                )?;
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
        profile_entities: whole_call.map(|_| entities),
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
    prepared_inventory: Option<RetainedGroundingInput>,
    retained_profile_bindings: &BTreeMap<String, (&'static str, JsonValue)>,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ClaimGrounding> {
    let (inventory, mut retained_budget) = match prepared_inventory {
        Some(prepared) => (prepared.inventory, Some(prepared.budget)),
        None => (
            maintained_inventory(ctx, executor, deadline, cancelled)?,
            None,
        ),
    };
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
    } = inventory;
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
    let mut identity_proposals = object(vec![]);
    let mut collection_orders = object(vec![]);
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
        let claim_id = text(claim, "claim_id")?;
        let special = if text(&descriptor, "reader")?.starts_with("identity-transition-") {
            Some("identity_proposals")
        } else if descriptor
            .object_get("object_reference_set")
            .and_then(|value| value.object_get("basis_adapter"))
            .and_then(JsonValue::as_str)
            == Some("collection-membership-versions-v1")
        {
            Some("collection_orders")
        } else {
            None
        };
        if let Some(kind) = special {
            let (observed_kind, observed) =
                retained_profile_bindings
                    .get(claim_id)
                    .ok_or(SourceCommandError::Conflict(
                        "exact retained Claim provenance binding absent from validation",
                    ))?;
            if *observed_kind != kind {
                return Err(SourceCommandError::Conflict(
                    "retained Claim provenance binding family changed",
                ));
            }
            set(
                if kind == "identity_proposals" {
                    &mut identity_proposals
                } else {
                    &mut collection_orders
                },
                claim_id,
                observed.clone(),
            )?;
        }
        let mut identities = BTreeSet::from([text(claim, "subject_ref")?.to_owned()]);
        if reader_kind == "identity" {
            if let Some(id) = field(claim, "object")?.as_str() {
                identities.insert(id.to_owned());
            }
        }
        if special == Some("identity_proposals") {
            for side in ["predecessors", "successors"] {
                for reference in array(field(claim, "object")?, side)? {
                    identities.insert(text(reference, "id")?.to_owned());
                }
            }
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
    // The maintained producer includes an empty identity_proposals object
    // whenever this batch has Collection orders, too.
    if !identity_proposals.as_object().unwrap().is_empty()
        || !collection_orders.as_object().unwrap().is_empty()
    {
        set(&mut bindings, "identity_proposals", identity_proposals)?;
    }
    if !collection_orders.as_object().unwrap().is_empty() {
        set(&mut bindings, "collection_orders", collection_orders)?;
    }
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
    if let Some(read) = retained_budget.as_mut() {
        let dependencies_state = retained_value_bytes(&dependencies)?;
        let result_state = retained_value_bytes(&grounding)?
            .checked_add(retained_value_bytes(&bindings)?)
            .and_then(|sum| sum.checked_add(dependencies_state))
            .ok_or(SourceCommandError::Unsupported(
                "Claim grounding output state overflow",
            ))?;
        read.debit(0, result_state)?;
    }
    Ok(ClaimGrounding {
        dependencies,
        bindings,
        retained_budget,
    })
}

fn account_retained_plan(
    grounding: &mut ClaimGrounding,
    response: &JsonValue,
    changes: &[SourceChange],
) -> SourceCommandResult<()> {
    if let Some(read) = grounding.retained_budget.as_mut() {
        let output_bytes = changes.iter().try_fold(0usize, |sum, change| {
            sum.checked_add(change.after.as_ref().map_or(0, Vec::len))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim prepared change state overflow",
                ))
        })?;
        let state = retained_value_bytes(response)?
            .checked_add(output_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "Claim prepared result state overflow",
            ))?;
        read.debit(0, state)?;
    }
    Ok(())
}

fn bounded_navigation(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.into()
    } else {
        format!("{}…", value.chars().take(limit - 1).collect::<String>())
    }
}
pub(crate) fn python_bytes_blank(raw: &[u8]) -> bool {
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
/// Read-only initial identity Claim validation over authenticated addressed proposal
/// inputs. No source-cut proof, source write, clock grant or complete inventory
/// is created. The publication observer separately authenticates receipt time.
pub(crate) fn validate_initial_identity_publication_claim(
    ctx: &CommandContext,
    claim: &JsonValue,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let (config, _, _, create, version) = config(ctx)?;
    if !create
        || version != 1
        || text(&config, "schema_version")? != "tos_local_claim_create_owner_v1"
    {
        return Err(SourceCommandError::Denied(
            "initial identity Claim publication owner",
        ));
    }
    grammar(&parse(&ctx.request_raw)?, true, false)?;
    claim_scope(&config, claim, true, 1)?;
    let (_, profile) = profile(ctx, text(claim, "predicate")?)?;
    if text(&profile, "reader")? != "identity-relation-v1" {
        return Err(SourceCommandError::Unsupported(
            "initial identity Claim publication profile",
        ));
    }
    let route = array(&profile, "schemas")?
        .iter()
        .find(|route| route.object_get("schema_version") == claim.object_get("schema_version"))
        .ok_or(SourceCommandError::Unsupported(
            "initial identity Claim publication schema",
        ))?;
    let schema = text(route, "schema_ref")?.to_owned();
    if validate_ground(
        ctx, None, None, &config, claim, 1, executor, deadline, cancelled,
    )?
    .is_some()
    {
        return Err(SourceCommandError::Unsupported(
            "initial identity Claim retained grounding",
        ));
    }
    Ok(schema)
}

fn validate_ground(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    mut retained: Option<&mut RetainedProfileRead>,
    config: &JsonValue,
    claim: &JsonValue,
    version: u8,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<(&'static str, JsonValue)>> {
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
        return Ok(Some((
            "identity_proposals",
            validate_identity_ground(
                ctx,
                cut,
                retained.as_deref_mut(),
                config,
                claim,
                reader,
                executor,
                deadline,
                cancelled,
            )?,
        )));
    }
    let mut retained_profile_binding = None;
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
                    retained_profile_binding = Some((
                        "collection_orders",
                        ground_collection_membership(
                            ctx,
                            cut,
                            retained.as_deref_mut(),
                            claim,
                            executor,
                            deadline,
                            cancelled,
                        )?,
                    ));
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
        let (record, _loc) = if let Some(read) = retained.as_deref_mut() {
            let location = text(
                read.inventory()?
                    .objects
                    .get(&id)
                    .ok_or(SourceCommandError::Conflict(
                        "Claim endpoint absent from complete selected inventory",
                    ))?,
                "source_record_ref",
            )?
            .to_owned();
            let record = parse(selected(ctx, &location)?)?;
            if text(&metadata_subject(&record)?, "id")? != id {
                return Err(SourceCommandError::Conflict(
                    "Claim endpoint selected identity changed",
                ));
            }
            (record, location)
        } else {
            find_record(ctx, &id)?
        };
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
            let (verified, source) = if retained.is_some() {
                let (verified, source, _, _, _) = exact_metadata_version(
                    ctx,
                    cut,
                    retained.as_deref_mut(),
                    &reference,
                    executor,
                    deadline,
                    cancelled,
                )?;
                (verified, source)
            } else {
                crate::source_revisions::resolve_record_version(
                    ctx, &reference, executor, deadline, cancelled,
                )?
            };
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
    Ok(retained_profile_binding)
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

fn archive_accounted(
    ctx: &CommandContext,
    config: &JsonValue,
    receipt: &JsonValue,
    read: &mut RetainedProfileRead,
    live_package_bytes: usize,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let location = text(receipt, "archive_path")?;
    let prefix = format!("{location}/");
    let mut archive_bytes = 0usize;
    let mut members = 0usize;
    for file in &ctx.files {
        if let Some(name) = file.path.as_str().strip_prefix(&prefix) {
            if name.contains('/') {
                continue;
            }
            archive_bytes = archive_bytes
                .checked_add(file.raw.len())
                .filter(|sum| *sum <= 8_388_608)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim archive package byte budget",
                ))?;
            members += 1;
        }
    }
    if members > 64 || archive_bytes as u64 > read.remaining_read_bytes()? {
        return Err(SourceCommandError::Unsupported(
            "Claim retained archive read budget",
        ));
    }
    let manifest_raw = selected(ctx, &format!("{location}/manifest.json"))?;
    read.check_temporary_state(live_package_bytes.checked_add(manifest_raw.len()).ok_or(
        SourceCommandError::Unsupported("Claim archive manifest state overflow"),
    )?)?;
    let manifest_state = retained_value_bytes(&parse(manifest_raw)?)?;
    // archive() holds its copied blob package while restoring the bound
    // names into a second package. The current owner package is also live.
    let temporary = live_package_bytes
        .checked_add(archive_bytes)
        .and_then(|sum| sum.checked_add(archive_bytes))
        .and_then(|sum| sum.checked_add(manifest_state))
        .ok_or(SourceCommandError::Unsupported(
            "Claim archive temporary state overflow",
        ))?;
    read.check_temporary_state(temporary)?;
    let restored = archive(ctx, config, receipt)?;
    read.debit(archive_bytes as u64, 0)?;
    Ok(restored)
}

fn archive_for_claim_call(
    ctx: &CommandContext,
    config: &JsonValue,
    receipt: &JsonValue,
    budget: &Rc<RefCell<ClaimCallBudget>>,
    live_package_bytes: usize,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let prefix = format!("{}/", text(receipt, "archive_path")?);
    let manifest_raw = selected(ctx, &format!("{prefix}manifest.json"))?;
    budget.borrow().check_live(
        live_package_bytes
            .checked_add(manifest_raw.len().saturating_mul(2))
            .ok_or(SourceCommandError::Unsupported(
                "Claim archive manifest state overflow",
            ))?,
    )?;
    let manifest_state = retained_value_bytes(&parse(manifest_raw)?)?;
    let archive_bytes = ctx
        .files
        .iter()
        .filter(|file| {
            file.path
                .as_str()
                .strip_prefix(&prefix)
                .is_some_and(|name| !name.contains('/'))
        })
        .try_fold(0usize, |sum, file| {
            sum.checked_add(file.raw.len())
                .filter(|value| *value <= 8_388_608)
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim archive package byte budget",
        ))?;
    {
        let mut budget = budget.borrow_mut();
        budget.check_live(
            live_package_bytes
                .checked_add(archive_bytes.saturating_mul(2))
                .and_then(|sum| sum.checked_add(manifest_state))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim archive temporary state overflow",
                ))?,
        )?;
        budget.read(archive_bytes as u64)?;
    }
    archive(ctx, config, receipt)
}

fn blob_name(raw: &[u8]) -> String {
    format!("{}.blob", Digest256::of_bytes(raw).to_hex())
}
pub(crate) fn archive_refs(files: &BTreeMap<String, Vec<u8>>) -> JsonValue {
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
    verify_claim_history_inner(ctx, config, p, files, None, None, 0, None)
}

fn verify_claim_history_inner(
    ctx: &CommandContext,
    config: &JsonValue,
    p: &RelativePath,
    files: &BTreeMap<String, Vec<u8>>,
    mut read: Option<&mut RetainedProfileRead>,
    whole_call: Option<&Rc<RefCell<ClaimCallBudget>>>,
    live_package_bytes: usize,
    mut first_archive: Option<&mut Option<BTreeMap<String, Vec<u8>>>>,
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
    if let Some(budget) = whole_call {
        budget
            .borrow_mut()
            .retain(retained_value_bytes(&history)?)?;
    }
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
        let retained = if let Some(read) = read.as_deref_mut() {
            archive_accounted(
                ctx,
                config,
                receipt,
                read,
                live_package_bytes
                    .checked_add(expected.as_ref().map_or(0, Vec::len))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim history temporary state overflow",
                    ))?,
            )?
        } else if let Some(budget) = whole_call {
            archive_for_claim_call(
                ctx,
                config,
                receipt,
                budget,
                live_package_bytes
                    .checked_add(expected.as_ref().map_or(0, Vec::len))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim history temporary state overflow",
                    ))?,
            )?
        } else {
            archive(ctx, config, receipt)?
        };
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
        if let Some(output) = first_archive.take() {
            if let Some(budget) = whole_call {
                let bytes = retained
                    .values()
                    .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
                    .ok_or(SourceCommandError::Unsupported(
                        "Claim first archive state overflow",
                    ))?;
                budget.borrow_mut().retain(bytes)?;
            }
            *output = Some(retained);
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
fn exact_selected_catalog_entry(
    ctx: &CommandContext,
    mut retained: Option<&mut RetainedProfileRead>,
    catalog_path: &str,
    identity: &str,
    identity_field: &str,
    expected: &JsonValue,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let capture = retained
        .as_deref()
        .and_then(|read| read.catalog_capture.as_ref())
        .cloned();
    let captured_raw: Option<Arc<[u8]>> = if let Some(selected) = capture.as_ref() {
        let read = retained
            .as_deref_mut()
            .ok_or(SourceCommandError::Conflict("Claim catalog budget absent"))?;
        let allowance = read
            .remaining_read_bytes()?
            .min(read.remaining_state_bytes()? as u64) as usize;
        let (raw, newly_read) =
            selected
                .borrow_mut()
                .route(catalog_path, max_bytes, allowance, deadline, cancelled)?;
        read.debit(newly_read as u64, newly_read)?;
        Some(raw)
    } else {
        None
    };
    let raw = match captured_raw.as_deref() {
        Some(raw) => raw,
        None => selected(ctx, catalog_path)?,
    };
    if raw.len() > max_bytes {
        return Err(SourceCommandError::Unsupported(
            "retained catalog byte budget",
        ));
    }
    if let Some(cached) = retained
        .as_deref()
        .and_then(|read| read.catalogs.get(catalog_path))
    {
        let (start, len, line) = cached
            .by_id
            .get(identity)
            .ok_or(SourceCommandError::Conflict(
                "exact retained identity absent from selected public catalog",
            ))?;
        if !same(&parse(&raw[*start..*start + *len])?, expected)? {
            return Err(SourceCommandError::Conflict(
                "retained catalog current source binding differs",
            ));
        }
        return Ok(object(vec![
            ("source_ref", string(catalog_path)),
            ("line", number(*line)),
            ("sha256", string(&cached.sha256)),
        ]));
    }
    let mut publication_bytes = 0u64;
    let control_ref = "ToS/source-witnesses/.metadata-publication.json";
    let captured = capture.as_ref().map(|capture| capture.borrow());
    let control_raw = match captured.as_ref() {
        Some(capture) => capture.control(),
        None => ctx.file(&path(control_ref)?)?,
    };
    publication_bytes += control_raw.map_or(0, |raw| raw.len() as u64);
    let control = control_raw.map(parse).transpose()?;
    let token = if let Some(control) = &control {
        exact_keys(
            control,
            &[
                "schema_version",
                "generation",
                "transition_id",
                "phase",
                "transaction_id",
                "manifest_sha256",
                "outcome",
                "recovery_authorization",
                "token",
            ],
        )?;
        if text(control, "schema_version")? != "tos_source_metadata_publication_v1"
            || integer(control, "generation")? == 0
            || integer(control, "generation")? > 9_007_199_254_740_991
            || text(control, "phase")? != "ready"
            || !["committed", "rolled-back"].contains(&text(control, "outcome")?)
        {
            return Err(SourceCommandError::Conflict(
                "retained catalog publication control state",
            ));
        }
        let authenticated = object(
            [
                "schema_version",
                "generation",
                "transition_id",
                "phase",
                "transaction_id",
                "manifest_sha256",
                "outcome",
                "recovery_authorization",
            ]
            .iter()
            .map(|key| Ok((*key, field(control, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
        );
        if text(control, "token")? != Digest256::of_bytes(&canonical(&authenticated)?).to_prefixed()
        {
            return Err(SourceCommandError::Conflict(
                "retained catalog publication control token",
            ));
        }
        Some(text(control, "token")?)
    } else {
        None
    };
    let manifest_ref = "ToS/source-witnesses/catalog/catalog.manifest.json";
    let manifest_raw = match captured.as_ref() {
        Some(capture) => capture.manifest(),
        None => ctx.file(&path(manifest_ref)?)?,
    };
    publication_bytes += manifest_raw.map_or(0, |raw| raw.len() as u64);
    let manifest = manifest_raw.map(parse).transpose()?;
    if let Some(manifest) = &manifest {
        if text(manifest, "schema_version")? != "tos_source_witness_catalog_v3" {
            return Err(SourceCommandError::Unsupported(
                "retained catalog manifest profile",
            ));
        }
        let route_selected = text(manifest, "claim_file")? == catalog_path
            || field(manifest, "record_files")?
                .as_object()
                .ok_or(SourceCommandError::Invalid("retained catalog routes"))?
                .iter()
                .any(|(_, value)| value.as_str() == Some(catalog_path));
        if !route_selected {
            return Err(SourceCommandError::Conflict(
                "retained catalog route absent from manifest",
            ));
        }
        let binding = manifest.object_get("selected_metadata_publication");
        match (binding, token) {
            (None, None) => (),
            (Some(binding), Some(token)) => {
                exact_keys(binding, &["protocol", "token", "files"])?;
                if text(binding, "protocol")? != "tos_selected_source_metadata_v1"
                    || text(binding, "token")? != token
                {
                    return Err(SourceCommandError::Conflict(
                        "retained catalog publication binding",
                    ));
                }
                let mut routes = field(manifest, "record_files")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid("retained catalog routes"))?
                    .iter()
                    .map(|(_, value)| {
                        Ok(value
                            .as_str()
                            .ok_or(SourceCommandError::Invalid("retained catalog route"))?
                            .to_owned())
                    })
                    .collect::<SourceCommandResult<Vec<_>>>()?;
                routes.push(text(manifest, "claim_file")?.to_owned());
                let expected = routes.into_iter().collect::<BTreeSet<_>>();
                let files =
                    field(binding, "files")?
                        .as_object()
                        .ok_or(SourceCommandError::Invalid(
                            "retained catalog publication files",
                        ))?;
                let actual = files
                    .iter()
                    .map(|(key, _)| {
                        key.as_str()
                            .map(str::to_owned)
                            .ok_or(SourceCommandError::Invalid(
                                "retained catalog publication path",
                            ))
                    })
                    .collect::<SourceCommandResult<BTreeSet<_>>>()?;
                if expected != actual {
                    return Err(SourceCommandError::Conflict(
                        "retained catalog published file closure",
                    ));
                }
                let digest = files
                    .iter()
                    .find(|(path, _)| path.as_str() == Some(catalog_path))
                    .map(|(_, digest)| digest)
                    .ok_or(SourceCommandError::Conflict(
                        "retained catalog selected digest absent",
                    ))?;
                if digest.as_str() != Some(Digest256::of_bytes(raw).to_hex().as_str()) {
                    return Err(SourceCommandError::Conflict(
                        "retained catalog published file bytes",
                    ));
                }
            }
            _ => {
                return Err(SourceCommandError::Conflict(
                    "retained catalog publication absent",
                ));
            }
        }
    } else if token.is_some() {
        return Err(SourceCommandError::Conflict(
            "retained catalog manifest absent for selected publication",
        ));
    }
    let mut found = None;
    let mut identities = BTreeSet::new();
    let mut positions = BTreeMap::new();
    let mut rows_seen = 0usize;
    let mut offset = 0usize;
    for (index, line) in raw.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let start = offset;
        offset = offset
            .checked_add(line.len())
            .ok_or(SourceCommandError::Unsupported(
                "retained catalog offset overflow",
            ))?;
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let decoded = std::str::from_utf8(line)
            .map_err(|_| SourceCommandError::Invalid("retained catalog UTF-8"))?;
        if stripped(decoded)?.is_empty() {
            continue;
        }
        rows_seen += 1;
        if rows_seen > 8192 || line.len() > 1_048_576 {
            return Err(SourceCommandError::Unsupported(
                "retained catalog row budget",
            ));
        }
        let entry = parse(line)?;
        let id = text(&entry, identity_field)?;
        if !identities.insert(id.to_owned()) {
            return Err(SourceCommandError::Conflict(
                "retained catalog identity duplicated",
            ));
        }
        positions.insert(id.to_owned(), (start, line.len(), (index + 1) as u64));
        if id == identity {
            if !same(&entry, expected)? {
                return Err(SourceCommandError::Conflict(
                    "retained catalog current source binding differs",
                ));
            }
            found = Some((index + 1) as u64);
        }
    }
    let line = found.ok_or(SourceCommandError::Conflict(
        "exact retained identity absent from selected public catalog",
    ))?;
    let sha256 = Digest256::of_bytes(raw).to_prefixed();
    if let Some(read) = retained.as_deref_mut() {
        let indexed_state = positions
            .keys()
            .try_fold(0usize, |sum, id| {
                sum.checked_add(id.len() + std::mem::size_of::<(String, (usize, usize, u64))>())
            })
            .ok_or(SourceCommandError::Unsupported(
                "retained catalog index state overflow",
            ))?;
        read.debit(
            if capture.is_some() {
                0
            } else {
                (raw.len() as u64).checked_add(publication_bytes).ok_or(
                    SourceCommandError::Unsupported("retained catalog read overflow"),
                )?
            },
            indexed_state
                .checked_add(catalog_path.len() + sha256.len())
                .ok_or(SourceCommandError::Unsupported(
                    "retained catalog index state overflow",
                ))?,
        )?;
        read.catalogs.insert(
            catalog_path.to_owned(),
            RetainedCatalogRows {
                by_id: positions,
                sha256: sha256.clone(),
            },
        );
    }
    Ok(object(vec![
        ("source_ref", string(catalog_path)),
        ("line", number(line)),
        ("sha256", string(&sha256)),
    ]))
}

fn exact_metadata_version(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    mut retained: Option<&mut RetainedProfileRead>,
    reference: &JsonValue,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String, JsonValue, JsonValue, &'static str)> {
    exact_ref(reference)?;
    let key = record_digest(reference)?.to_hex();
    if let Some(cached) = retained
        .as_deref_mut()
        .and_then(|read| read.metadata.get(&key))
    {
        return Ok(cached.clone());
    }
    let mut resolved = if let (Some(cut), Some(read)) = (cut, retained.as_deref_mut()) {
        let owner_path = text(
            read.inventory()?
                .objects
                .get(text(reference, "id")?)
                .ok_or(SourceCommandError::Conflict(
                    "retained metadata owner absent from complete inventory",
                ))?,
            "source_record_ref",
        )?
        .to_owned();
        let remaining_read = read.remaining_read_bytes()?;
        crate::source_revisions::resolve_record_version_evidence_at_from_cut(
            ctx,
            cut,
            &owner_path,
            tos_validation::item_rules::ItemLimits {
                max_member_bytes: 8_388_608usize.min(remaining_read as usize),
                max_total_bytes: remaining_read,
                max_state_bytes: read.remaining_state_bytes()?,
                max_issues: 256,
                deadline,
            },
            reference,
            executor,
            deadline,
            cancelled,
        )?
    } else if let Some(cut) = cut {
        crate::source_revisions::resolve_record_version_evidence_from_cut(
            ctx, cut, reference, executor, deadline, cancelled,
        )?
    } else {
        crate::source_revisions::resolve_record_version_evidence(
            ctx, reference, executor, deadline, cancelled,
        )?
    };
    if let Some(read) = retained.as_deref_mut() {
        read.debit(resolved.bytes_read, resolved.returned_state_bytes)?;
        read.observed_reads.append(&mut resolved.reads);
    }
    let current_record = resolved.current_record.as_ref().unwrap_or(&resolved.record);
    let kind = native_record_type(current_record)?;
    let schema_version = text(current_record, "schema_version")?;
    let (adapter, filename, schema_ref, expected_schema) = if schema_version
        == "tos_corpus_record_v1"
    {
        let filename = match kind {
            "agent" => "agents.jsonl",
            "place" => "places.jsonl",
            "organization" => "organizations.jsonl",
            "work" => "works.jsonl",
            "expression" => "expressions.jsonl",
            "edition" => "editions.jsonl",
            "collection" => "collections.jsonl",
            "item" => "items.jsonl",
            _ => return Err(SourceCommandError::Unsupported("metadata catalog family")),
        };
        (
            "native-corpus",
            filename,
            "ToS/contracts/corpus-record.schema.json",
            None,
        )
    } else {
        let route = array(&resolved.route_profile, "schemas")?
            .iter()
            .find(|route| {
                route.object_get("schema_version") == current_record.object_get("schema_version")
            })
            .ok_or(SourceCommandError::Unsupported(
                "retained metadata profile schema route",
            ))?;
        (
            "declared-profile",
            text(&resolved.route_profile, "catalog_filename")?,
            text(route, "schema_ref")?,
            Some(text(route, "schema_ref")?),
        )
    };
    let entities_owned = retained
        .is_none()
        .then(|| json_file(ctx, ENTITIES))
        .transpose()?;
    let entities = retained
        .as_deref()
        .map(|read| &read.entities)
        .or(entities_owned.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "retained entity registry unavailable",
        ))?;
    let matches = array(entities, "types")?
        .iter()
        .filter(|entry| {
            if adapter == "declared-profile" {
                entry
                    .object_get("source_record_profile")
                    .and_then(|profile| profile.object_get("record_type"))
                    .and_then(JsonValue::as_str)
                    == Some(kind)
            } else {
                entry
                    .object_get("source_mappings")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|mappings| {
                        mappings.iter().any(|mapping| {
                            mapping
                                .object_get("source_graph")
                                .and_then(JsonValue::as_str)
                                == Some("source-navigation")
                                && mapping
                                    .object_get("source_kind_id")
                                    .and_then(JsonValue::as_str)
                                    == Some(kind)
                        })
                    })
            }
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "metadata identity has no unique selected type descriptor",
        ));
    }
    let type_id = field(matches[0], "type_id")?.clone();
    let expected_plan = catalogue_record(current_record, &resolved.source_path, expected_schema)?;
    if let Some(read) = retained.as_deref() {
        read.check_temporary_state(expected_plan.bound()?)?;
    }
    let expected = expected_plan.construct()?;
    let catalog_path = format!("ToS/source-witnesses/catalog/{filename}");
    let mut catalog = exact_selected_catalog_entry(
        ctx,
        retained.as_deref_mut(),
        &catalog_path,
        text(reference, "id")?,
        "record_id",
        &expected,
        8_388_608,
        deadline,
        cancelled,
    )?;
    set(
        &mut catalog,
        "source_record_ref",
        string(&resolved.source_path),
    )?;
    set(
        &mut catalog,
        "current_record_ref",
        resolved.current_ref.clone(),
    )?;
    let descriptor = object(vec![
        ("adapter", string(adapter)),
        ("record_type", string(kind)),
        ("profile_type_id", type_id.clone()),
        ("source_schema_ref", string(schema_ref)),
        ("source_schema_version", string(schema_version)),
        ("source_scope", string("public_metadata_only")),
        ("record_kind", string("subject")),
        ("identity_field", string("record_id")),
        (
            "source_basename",
            field(&resolved.route_profile, "source_basename")?.clone(),
        ),
        ("schema_version", string(schema_version)),
        ("schema_ref", string(schema_ref)),
        ("type_id", type_id),
    ]);
    let provenance = object(vec![
        ("verification_scope", string("selected-record-chain")),
        ("all_package_bytes_verified", JsonValue::Bool(false)),
        ("catalog", catalog),
        ("descriptor", descriptor.clone()),
        ("history", resolved.history),
        ("source", resolved.source),
        ("transition", resolved.transition),
    ]);
    let result = (
        resolved.record,
        resolved.source_path,
        descriptor,
        provenance,
        resolved.version_status,
    );
    if let Some(read) = retained.as_deref_mut() {
        let provenance_state = retained_value_bytes(&result.3)?;
        let output_state = retained_value_bytes(&result.0)?
            .checked_add(retained_value_bytes(&result.2)?)
            .and_then(|n| n.checked_add(provenance_state))
            .and_then(|n| n.checked_add(result.1.len()))
            .ok_or(SourceCommandError::Unsupported(
                "retained metadata output state overflow",
            ))?;
        read.debit(
            0,
            output_state
                .checked_mul(2)
                .ok_or(SourceCommandError::Unsupported(
                    "retained metadata cached state overflow",
                ))?,
        )?;
        read.metadata.insert(key, result.clone());
    }
    Ok(result)
}

fn validate_identity_ground(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    mut retained: Option<&mut RetainedProfileRead>,
    config: &JsonValue,
    claim: &JsonValue,
    reader: &str,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
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
    let mut participants = Vec::new();
    for reference in left.iter().chain(right) {
        let (record, source, descriptor, provenance, _) = exact_metadata_version(
            ctx,
            cut,
            retained.as_deref_mut(),
            reference,
            executor,
            deadline,
            cancelled,
        )?;
        let kind = native_record_type(&record)?;
        let matching = types
            .iter()
            .filter(|entry| {
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
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Invalid(
                "identity participant source type",
            ));
        }
        let entry = matching[0];
        if field(entry, "type_id")? != field(&descriptor, "type_id")? {
            return Err(SourceCommandError::Conflict(
                "identity participant exact descriptor type differs from Claim mapping",
            ));
        }
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
                    || text(profile, "graph_layer")? != "source-profile"
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
        participants.push(object(vec![
            ("ref", reference.clone()),
            ("descriptor", descriptor),
            ("provenance", provenance),
        ]));
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
    let mut related_bindings = Vec::new();
    for reference in related {
        exact_ref(reference)?;
        grant(config, "allowed_related_claim_refs", reference)?;
        if text(reference, "id")? == text(claim, "claim_id")? {
            return Err(SourceCommandError::Invalid(
                "identity proposal cannot reference itself as retained assertion",
            ));
        }
        let retained_claim = resolve_claim_reference_evidence(
            ctx,
            retained.as_deref_mut(),
            reference,
            deadline,
            cancelled,
        )?;
        if same(reference, previous)? {
            let p = text(&retained_claim.record, "predicate")?;
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
                    route.object_get("schema_version")
                        == retained_claim.record.object_get("schema_version")
                })
                .ok_or(SourceCommandError::Invalid(
                    "identity predecessor schema route",
                ))?;
            schema_check(
                executor,
                text(config, "source_path")?,
                &retained_claim.record,
                text(route, "schema_ref")?,
                deadline,
                cancelled,
            )?;
        }
        related_bindings.push(object(vec![
            ("ref", reference.clone()),
            ("provenance", retained_claim.provenance),
        ]));
    }
    Ok(object(vec![
        ("participants", JsonValue::Array(participants)),
        ("claims", JsonValue::Array(related_bindings)),
    ]))
}
#[derive(Clone)]
struct ResolvedClaimReference {
    record: JsonValue,
    provenance: JsonValue,
    version_status: &'static str,
}

fn collection_membership_stream_path(location: &str) -> bool {
    let parts = location.split('/').collect::<Vec<_>>();
    parts.len() == 6
        && parts[..3] == ["ToS", "source-witnesses", "collections"]
        && parts[5] == "membership-claims.jsonl"
        && parts[3..5].iter().all(|part| {
            !part.is_empty()
                && part.split(['.', '-']).all(|segment| {
                    !segment.is_empty()
                        && segment
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                })
        })
}

fn claim_stream_line(raw: &[u8], id: &str) -> SourceCommandResult<u64> {
    for (index, line) in raw.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        if !python_bytes_blank(line) && text(&parse(line)?, "claim_id")? == id {
            return Ok((index + 1) as u64);
        }
    }
    Err(SourceCommandError::Conflict(
        "exact Claim row absent from verified stream",
    ))
}

fn resolve_claim_reference_evidence(
    ctx: &CommandContext,
    mut retained: Option<&mut RetainedProfileRead>,
    reference: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedClaimReference> {
    exact_ref(reference)?;
    let key = record_digest(reference)?.to_hex();
    if let Some(cached) = retained
        .as_deref_mut()
        .and_then(|read| read.claims.get(&key))
    {
        return Ok(cached.clone());
    }
    let id = text(reference, "id")?;
    let (current, p, current_raw) = if let Some(read) = retained.as_deref_mut() {
        let source_path = text(
            read.inventory()?
                .claims
                .get(id)
                .ok_or(SourceCommandError::Conflict(
                    "related Claim absent from complete selected inventory",
                ))?,
            "source_claim_file_ref",
        )?;
        let p = path(source_path)?;
        let raw = selected(ctx, source_path)?;
        let current = rows(raw)?.remove(id).ok_or(SourceCommandError::Conflict(
            "related Claim selected owner identity changed",
        ))?;
        (current, p, raw)
    } else {
        let mut found = None;
        for file in &ctx.files {
            if !file.path.as_str().starts_with("ToS/source-witnesses/")
                || !(file.path.as_str().ends_with("/source-claims.jsonl")
                    || collection_membership_stream_path(file.path.as_str()))
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
                found = Some((record.clone(), file.path.clone(), file.raw.as_slice()));
            }
        }
        found.ok_or(SourceCommandError::Unsupported(
            "related Claim owner source not selected",
        ))?
    };
    let legacy = collection_membership_stream_path(p.as_str());
    if legacy
        && (text(&current, "schema_version")? != "tos_claim_packet_v1"
            || text(&current, "claim_type")? != "bibliographic"
            || text(&current, "predicate")? != "contains_work"
            || text(&current, "assertion_layer")? != "bibliographic_assertion"
            || current.object_get("polarity").and_then(JsonValue::as_str) != Some("positive")
                && current.object_get("polarity").is_some()
            || !text(&current, "subject_ref")?.starts_with("tos.collection.")
            || !text(&current, "object")?.starts_with("tos.work."))
    {
        return Err(SourceCommandError::Conflict(
            "legacy Collection membership source contract differs",
        ));
    }
    let current_ref = metadata_subject(&current)?;
    let current_line = claim_stream_line(current_raw, id)?;
    let pre_plan_state = if let Some(read) = retained.as_deref() {
        let claim_state = retained_value_bytes(&current)?;
        let registry_state = if legacy {
            0
        } else {
            selected(ctx, RELATIONS)?.len().checked_mul(3).ok_or(
                SourceCommandError::Unsupported("Claim catalogue profile state overflow"),
            )?
        };
        let state =
            claim_state
                .checked_add(registry_state)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim catalogue profile state overflow",
                ))?;
        read.check_temporary_state(state)?;
        state
    } else {
        0
    };
    let expected_plan = catalogue_claim(
        ctx,
        &current,
        p.as_str(),
        current_line as usize,
        !legacy,
        None,
    )?;
    if let Some(read) = retained.as_deref() {
        let temporary = pre_plan_state.checked_add(expected_plan.bound()?).ok_or(
            SourceCommandError::Unsupported("Claim catalogue profile state overflow"),
        )?;
        read.check_temporary_state(temporary)?;
    }
    let expected_catalog = expected_plan.construct()?;
    let mut catalog = exact_selected_catalog_entry(
        ctx,
        retained.as_deref_mut(),
        "ToS/source-witnesses/catalog/claims.jsonl",
        id,
        "claim_id",
        &expected_catalog,
        33_554_432,
        deadline,
        cancelled,
    )?;
    set(&mut catalog, "source_claim_file_ref", string(p.as_str()))?;
    set(&mut catalog, "source_claim_line", number(current_line))?;
    set(&mut catalog, "current_record_ref", current_ref.clone())?;
    set(
        &mut catalog,
        "visibility",
        field(&current, "visibility")?.clone(),
    )?;
    let mut selected_record = current.clone();
    let mut selected_stream: Cow<'_, [u8]> = Cow::Borrowed(current_raw);
    let mut selected_line = current_line;
    let mut selected_revision = JsonValue::Null;
    let mut selected_blob = JsonValue::Null;
    let mut transition = JsonValue::Null;
    let mut version_status = "current";
    let history = if legacy {
        if !same(&current_ref, reference)? {
            return Err(SourceCommandError::Conflict(
                "legacy Collection membership has no retained predecessor",
            ));
        }
        object(vec![
            ("source_ref", JsonValue::Null),
            ("sha256", JsonValue::Null),
            ("receipt_count", number(0)),
            ("correction_chain_verified", JsonValue::Bool(false)),
            ("adapter", string("retained-collection-membership-v1")),
        ])
    } else {
        let mut config = parse(&ctx.configuration_raw)?;
        set(&mut config, "source_path", string(p.as_str()))?;
        set(&mut config, "claim_id", string(id))?;
        let (history, history_sha256) = if let Some(read) = retained.as_deref_mut() {
            let owner = read.claim_owner(ctx, &config, &p)?;
            selected_revision = owner.revision;
            (owner.history, owner.history_sha256)
        } else {
            let files = package(ctx, &p)?;
            let history = verify_claim_history(ctx, &config, &p, &files)?;
            selected_revision = revision(&files)?;
            (
                history,
                files
                    .get(CLAIM_HISTORY)
                    .map(|raw| Digest256::of_bytes(raw).to_prefixed()),
            )
        };
        if !same(&current_ref, reference)? {
            let mut selected_receipt = None;
            for receipt in array(&history, "receipts")? {
                if same(field(receipt, "previous_source")?, reference)? {
                    if selected_receipt.replace(receipt).is_some() {
                        return Err(SourceCommandError::Conflict(
                            "exact related Claim predecessor duplicated in history",
                        ));
                    }
                }
            }
            let receipt = selected_receipt.ok_or(SourceCommandError::Unsupported(
                "exact related Claim version not retained in selected owner history",
            ))?;
            let mut archived = if let Some(read) = retained.as_deref_mut() {
                archive_accounted(ctx, &config, receipt, read, 0)?
            } else {
                archive(ctx, &config, receipt)?
            };
            selected_stream = Cow::Owned(
                archived
                    .remove(CLAIM_STREAM)
                    .ok_or(SourceCommandError::Conflict("archived Claim stream absent"))?,
            );
            selected_record =
                rows(&selected_stream)?
                    .remove(id)
                    .ok_or(SourceCommandError::Conflict(
                        "exact related Claim absent in archive",
                    ))?;
            selected_line = claim_stream_line(&selected_stream, id)?;
            selected_revision = field(receipt, "previous_revision")?.clone();
            let manifest_path = format!("{}/manifest.json", text(receipt, "archive_path")?);
            let manifest = parse(selected(ctx, &manifest_path)?)?;
            let blob = text(field(field(&manifest, "files")?, CLAIM_STREAM)?, "blob")?;
            selected_blob = string(&format!("{}/{blob}", text(receipt, "archive_path")?));
            transition = object(
                [
                    "command_id",
                    "recorded_at",
                    "previous_source",
                    "source",
                    "request_digest",
                ]
                .iter()
                .map(|key| Ok((*key, field(receipt, key)?.clone())))
                .collect::<SourceCommandResult<Vec<_>>>()?,
            );
            version_status = "historical";
        }
        object(vec![
            (
                "source_ref",
                string(&format!(
                    "{}/{}",
                    p.as_str().rsplit_once('/').unwrap().0,
                    CLAIM_HISTORY
                )),
            ),
            (
                "sha256",
                history_sha256
                    .as_deref()
                    .map(string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "receipt_count",
                number(array(&history, "receipts")?.len() as u64),
            ),
            ("correction_chain_verified", JsonValue::Bool(true)),
        ])
    };
    if !same(&metadata_subject(&selected_record)?, reference)? {
        return Err(SourceCommandError::Conflict(
            "exact related Claim historical binding differs",
        ));
    }
    let source = object(vec![
        ("source_ref", string(p.as_str())),
        (
            "stream_sha256",
            string(&Digest256::of_bytes(&selected_stream).to_prefixed()),
        ),
        ("stream_bytes", number(selected_stream.len() as u64)),
        ("package_revision", selected_revision),
        ("archive_blob_ref", selected_blob),
        ("line", number(selected_line)),
    ]);
    let result = ResolvedClaimReference {
        record: selected_record,
        provenance: object(vec![
            ("catalog", catalog),
            ("source", source),
            ("history", history),
            ("transition", transition),
        ]),
        version_status,
    };
    if let Some(read) = retained.as_deref_mut() {
        let state = retained_value_bytes(&result.record)?
            .checked_add(retained_value_bytes(&result.provenance)?)
            .and_then(|n| n.checked_add(selected_stream.len()))
            .and_then(|n| n.checked_add(std::mem::size_of::<ResolvedClaimReference>()))
            .ok_or(SourceCommandError::Unsupported(
                "retained Claim evidence state overflow",
            ))?;
        read.debit(
            (current_raw.len() as u64)
                .checked_add(selected_stream.len() as u64)
                .ok_or(SourceCommandError::Unsupported(
                    "retained Claim evidence read overflow",
                ))?,
            state.checked_mul(2).ok_or(SourceCommandError::Unsupported(
                "retained Claim cached state overflow",
            ))?,
        )?;
        read.claims.insert(key, result.clone());
    }
    Ok(result)
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
    cut: Option<&CorpusCutReader>,
    mut retained: Option<&mut RetainedProfileRead>,
    claim: &JsonValue,
    executor: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let value = field(claim, "object")?;
    let collection_ref = field(value, "collection_version")?;
    exact_ref(collection_ref)?;
    let subject = text(claim, "subject_ref")?;
    if text(collection_ref, "id")? != subject || !subject.starts_with("tos.collection.") {
        return Err(SourceCommandError::Invalid(
            "Collection order exact own Collection version",
        ));
    }
    let (collection, _, _, collection_provenance, collection_status) = exact_metadata_version(
        ctx,
        cut,
        retained.as_deref_mut(),
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
    let mut bound = Vec::new();
    let mut input_digests = BTreeMap::new();
    bind_retained_version_inputs(&collection_provenance, &mut input_digests)?;
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
        let member = resolve_claim_reference_evidence(
            ctx,
            retained.as_deref_mut(),
            reference,
            deadline,
            cancelled,
        )?;
        let legacy = text(&member.record, "schema_version")? == "tos_claim_packet_v1"
            && text(&member.record, "claim_type")? == "bibliographic";
        let native = text(&member.record, "schema_version")? == "tos_source_relation_claim_v1"
            && text(&member.record, "claim_type")? == "relation";
        let member_object = text(&member.record, "object")?;
        let polarity = member
            .record
            .object_get("polarity")
            .and_then(JsonValue::as_str)
            .or(if legacy { Some("positive") } else { None });
        if !(legacy || native)
            || !["bibliographic_assertion", "scholarly_report"]
                .contains(&text(&member.record, "assertion_layer")?)
            || text(&member.record, "subject_ref")? != subject
            || text(&member.record, "predicate")? != "contains_work"
            || polarity != Some("positive")
            || !members.contains(member_object)
            || !resolved.insert(member_object.to_owned())
        {
            return Err(SourceCommandError::Invalid(
                "basis must be distinct positive membership in exact Collection",
            ));
        }
        bind_retained_version_inputs(&member.provenance, &mut input_digests)?;
        bound.push(object(vec![
            ("ref", reference.clone()),
            ("provenance", member.provenance),
            ("version_status", string(member.version_status)),
        ]));
    }
    if resolved != members.iter().map(|v| (*v).to_owned()).collect() {
        return Err(SourceCommandError::Invalid(
            "Collection membership basis does not close scoped member set",
        ));
    }
    Ok(object(vec![
        (
            "collection",
            object(vec![
                ("ref", collection_ref.clone()),
                ("provenance", collection_provenance),
                ("version_status", string(collection_status)),
            ]),
        ),
        ("memberships", JsonValue::Array(bound)),
        (
            "input_digests",
            JsonValue::Object(
                input_digests
                    .into_iter()
                    .map(|(key, value)| {
                        (tos_foundation::JsonString::from_utf8(&key), string(&value))
                    })
                    .collect(),
            ),
        ),
        ("establishes_membership", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
    ]))
}

fn bind_retained_version_inputs(
    provenance: &JsonValue,
    digests: &mut BTreeMap<String, String>,
) -> SourceCommandResult<()> {
    let mut bind = |location: &str, prefixed: &str| -> SourceCommandResult<()> {
        let digest = Digest256::from_prefixed(prefixed)
            .map_err(|_| SourceCommandError::Invalid("retained provenance digest"))?
            .to_hex();
        if let Some(previous) = digests.insert(location.to_owned(), digest.clone()) {
            if previous != digest {
                return Err(SourceCommandError::Conflict(
                    "retained provenance path has conflicting digests",
                ));
            }
        }
        Ok(())
    };
    for section in ["catalog", "history"] {
        let entry = field(provenance, section)?;
        if let (Some(source), Some(digest)) = (
            entry.object_get("source_ref").and_then(JsonValue::as_str),
            entry.object_get("sha256").and_then(JsonValue::as_str),
        ) {
            bind(source, digest)?;
        }
    }
    let source = field(provenance, "source")?;
    let location = source
        .object_get("archive_blob_ref")
        .and_then(JsonValue::as_str)
        .unwrap_or(text(source, "source_ref")?);
    let digest = source
        .object_get("record_sha256")
        .or_else(|| source.object_get("stream_sha256"))
        .and_then(JsonValue::as_str)
        .ok_or(SourceCommandError::Invalid(
            "retained source provenance digest",
        ))?;
    bind(location, digest)?;
    if let Some(manifest) = source
        .object_get("archive_manifest_ref")
        .and_then(JsonValue::as_str)
    {
        bind(manifest, text(source, "archive_manifest_sha256")?)?;
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
