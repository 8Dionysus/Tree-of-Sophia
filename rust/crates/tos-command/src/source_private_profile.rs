//! Owner-local semantic source-profile command semantics.
//!
//! This is a native computational adapter for the existing Python profile
//! command family.  It consumes one protected owner context and exact selected
//! source/software inputs; it never treats the private configuration or public
//! profile registry as a source-read grant.  Transport and publication remain
//! with `PrivateOwnerStore` and the native entry.

use crate::source_command::{self as cmd, *};
use crate::source_forms;
use crate::source_serialization::{self, PrivateMetadataFamily};
use crate::source_sign_native::{self, SignNativeRead};
use crate::source_text_owner::OwnerTextContext;
use crate::source_text_private_store::{
    PrivateArchiveReader, PrivateIdentityInputs, PrivatePackage,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    Digest256, JsonLimits, JsonString, JsonValue, RelativePath, emit_python_compact_json,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

pub(crate) const CONFIG: &str = "tos_local_owner_profile_command_v1";
pub(crate) const HISTORY: &str = "source-revision-history.json";
pub(crate) const CONFIG_FILE: &str = "source-create-owner-configuration.json";
pub(crate) const RECEIPT_FILE: &str = "source-create-receipt.json";
const CREATE_REQUEST: &str = "source-create-request.json";
const CREATE_ENVIRONMENT: &str = "source-create-environment.json";
const CREATE_PROVENANCE: &str = "source-create-provenance.jsonl";
const PROFILE_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const PROFILE_CONTRACT: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const CONTEXT_CONTRACT: &str = "ToS/contracts/owner-local-source-context.schema.json";
const CORPUS_CONTRACT: &str = "ToS/contracts/corpus-record.schema.json";
const METADATA_CONTRACT: &str = "ToS/contracts/source-metadata-record.schema.json";
const FORM_CONTRACTS: &[&str] = &[
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
const OPERATIONS: &[&str] = &[
    "source.create",
    "record.revise",
    "form.create",
    "form.revise",
];
const REVISION_FIELDS: &[&str] = &[
    "preferred_label",
    "variant_labels",
    "notes",
    "field_languages",
    "source_refs",
    "extensions",
    "semantic_content",
];
const IMPLEMENTATIONS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_profile_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_unit_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_owner_record_profiles.py",
    "scripts/source_record_profiles.py",
    "scripts/source_owner_context.py",
    "scripts/native_text_binding.py",
    "scripts/source_witness_human_forms.py",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

const MAX_REVISIONS: usize = 128;
const MAX_INPUTS: usize = 128;
const MAX_INPUT_BYTES: usize = 8_388_608;
const MAX_NATIVE_SNAPSHOT_BYTES: usize = 16_777_216;
const MAX_INVENTORY_FILES: usize = 2048;
const MAX_INVENTORY_BYTES: usize = 67_108_864;
const MAX_INVENTORY_FILE_BYTES: usize = 33_554_432;
const MAX_INVENTORY_ENTRIES: usize = 32_768;

/// Semantic plan only. The selected native owner decides whether to publish
/// `after` and the retained predecessor archive under its exact locks/fences.
pub(crate) struct PrivateProfilePlan {
    pub(crate) before: Option<PrivatePackage>,
    pub(crate) after: Option<PrivatePackage>,
    pub(crate) response: JsonValue,
    pub(crate) archive: Option<(String, PrivatePackage)>,
    pub(crate) reads: Vec<SourceFile>,
    pub(crate) replayed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectedProfileRecord {
    pub(crate) profile: JsonValue,
    pub(crate) record: JsonValue,
    pub(crate) source_path: String,
    pub(crate) source_digest: String,
    pub(crate) dependencies: Vec<SourceDependency>,
}

#[derive(Clone)]
struct Grant {
    value: JsonValue,
    digest: String,
    source_path: RelativePath,
    profile: JsonValue,
    profile_type_id: String,
    private_prefix: String,
    home: String,
    form_ids: Vec<String>,
    operations: Vec<String>,
    fields: Vec<String>,
    source_access: JsonValue,
    source_binding: Option<JsonValue>,
}

struct State {
    files: PrivatePackage,
    record: JsonValue,
    subject: JsonValue,
    history: JsonValue,
    forms: JsonValue,
}

struct Snapshot {
    profile_snapshot: String,
    rights_snapshot: Option<String>,
    native_summary: Option<JsonValue>,
    native_reads: Vec<SourceFile>,
    form_digests: JsonValue,
    identity_digest: String,
    implementation_digests: JsonValue,
    dependencies: String,
}

fn encode(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    let mut raw = cmd::canonical(value)?;
    raw.push(b'\n');
    Ok(raw)
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    crate::source_creation_store::active(deadline, cancelled)
}

fn digest_text(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|tail| {
        tail.len() == 64
            && tail
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn hash_map(values: BTreeMap<String, String>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(name, digest)| (JsonString::from_utf8(&name), cmd::string(&digest)))
            .collect(),
    )
}

fn text_array(value: &JsonValue, key: &str, max: usize) -> SourceCommandResult<Vec<String>> {
    let rows = cmd::array(value, key)?;
    if rows.len() > max {
        return Err(SourceCommandError::Invalid(
            "owner-local profile scope budget",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let item = row.as_str().ok_or(SourceCommandError::Invalid(
            "owner-local profile scope text",
        ))?;
        if !seen.insert(item.to_owned()) {
            return Err(SourceCommandError::Invalid(
                "duplicate profile scope member",
            ));
        }
        output.push(item.to_owned());
    }
    Ok(output)
}

fn form_id(value: &str) -> bool {
    let Some(tail) = value.strip_prefix("tos.form.") else {
        return false;
    };
    !tail.is_empty()
        && tail
            .split(['.', '-'])
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase_or_digit()))
}

fn event_id(value: &str) -> bool {
    let Some(tail) = value.strip_prefix("tos.event.") else {
        return false;
    };
    !tail.is_empty()
        && tail
            .split(['.', '-'])
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase_or_digit()))
}

trait LowerOrDigit {
    fn is_ascii_lowercase_or_digit(self) -> bool;
}
impl LowerOrDigit for u8 {
    fn is_ascii_lowercase_or_digit(self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}

/// Profile source names used by the maintained owner-local identity scan.
/// Corpus-only adapters and the public catalog are deliberately excluded.
pub(crate) fn profile_identity_basenames(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeSet<String>> {
    if cut.current().revision() != ctx.base_revision
        || worker.source_revision() != ctx.base_revision
    {
        return Err(SourceCommandError::Conflict(
            "profile registry worker and selected source cut differ",
        ));
    }
    let registry = crate::source_revisions::validate_source_profile_registry(
        worker, deadline, cancelled, ctx,
    )?;
    let mut result = BTreeSet::new();
    for entry in cmd::array(&registry, "types")? {
        let Some(profile) = entry.object_get("source_record_profile") else {
            continue;
        };
        if cmd::text(profile, "reader")? == "semantic-metadata-v1" {
            result.insert(cmd::text(profile, "source_basename")?.to_owned());
        }
    }
    Ok(result)
}

/// Exact Python profile `_inventory` basename selection, including its shared
/// claim stream. Claim-specific metadata readers can add only their own names.
pub(crate) fn identity_basenames(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeSet<String>> {
    let mut result = profile_identity_basenames(ctx, cut, worker, deadline, cancelled)?;
    result.insert("source-claims.jsonl".into());
    Ok(result)
}

/// Claim's SourceRecordProfiles inventory includes both declared reader kinds.
pub(crate) fn public_identity_basenames(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeSet<String>> {
    if cut.current().revision() != ctx.base_revision
        || worker.source_revision() != ctx.base_revision
    {
        return Err(SourceCommandError::Conflict(
            "profile registry worker and selected source cut differ",
        ));
    }
    let registry = crate::source_revisions::validate_source_profile_registry(
        worker, deadline, cancelled, ctx,
    )?;
    let mut result = BTreeSet::new();
    for entry in cmd::array(&registry, "types")? {
        if let Some(profile) = entry.object_get("source_record_profile") {
            result.insert(cmd::text(profile, "source_basename")?.to_owned());
        }
    }
    result.insert("source-claims.jsonl".into());
    Ok(result)
}

fn selected_profile(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    profile_type_id: &str,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "private profile selected cut differs",
        ));
    }
    let registry = crate::source_revisions::validate_source_profile_registry(
        worker, deadline, cancelled, ctx,
    )?;
    let entry = cmd::array(&registry, "types")?
        .iter()
        .find(|entry| cmd::text(entry, "type_id").ok() == Some(profile_type_id))
        .ok_or(SourceCommandError::Denied("private profile type is absent"))?;
    let profile = cmd::field(entry, "source_record_profile")?;
    if cmd::text(profile, "reader")? != "semantic-metadata-v1" {
        return Err(SourceCommandError::Denied(
            "private profile requires the exact semantic metadata reader",
        ));
    }
    Ok(profile.clone())
}

fn profile_schema_refs(
    profile: &JsonValue,
    schema_version: &str,
) -> SourceCommandResult<(String, Vec<String>)> {
    let routes = cmd::array(profile, "schemas")?
        .iter()
        .filter(|route| cmd::text(route, "schema_version").ok() == Some(schema_version))
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "private source profile exact schema route unavailable",
        ));
    }
    let route = routes[0];
    let root = cmd::text(route, "schema_ref")?.to_owned();
    let mut refs = vec![CORPUS_CONTRACT.to_owned()];
    refs.extend(text_array(route, "schema_dependencies", 128)?);
    refs.push(root.clone());
    let mut seen = BTreeSet::new();
    refs.retain(|item| seen.insert(item.clone()));
    if !refs.iter().any(|item| item == METADATA_CONTRACT) {
        return Err(SourceCommandError::Unsupported(
            "profile route omits shared source metadata contract",
        ));
    }
    if refs.len() > MAX_INPUTS {
        return Err(SourceCommandError::Invalid(
            "private profile schema reference budget",
        ));
    }
    Ok((root, refs))
}

fn schema_instance(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    source_path: &str,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "profile schema worker cut differs",
        ));
    }
    for name in refs {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("profile schema path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "selected profile schema absent",
        ))?;
        let schema = cmd::parse(raw)?;
        let expected_a = format!("https://tree-of-sophia.local/{name}");
        let expected_b = format!("https://treeofsophia.local/{name}");
        if !matches!(cmd::text(&schema, "$id")?, value if value == expected_a || value == expected_b)
            || worker.contract_digest(name) != Some(Digest256::of_bytes(raw))
        {
            return Err(SourceCommandError::Conflict(
                "profile schema identity or worker digest differs",
            ));
        }
    }
    let raw = cmd::canonical(instance)?;
    match worker.check(source_path, &raw, root, deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid(
            "private profile record schema rejected bytes",
        )),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: source_path.to_owned(),
            root: root.to_owned(),
            reason,
        }),
    }
}

fn validate_profile_record(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    profile_type_id: &str,
    expected_path: &str,
    selected_id: &str,
    raw: &[u8],
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedProfileRecord> {
    let profile = selected_profile(ctx, cut, profile_type_id, worker, deadline, cancelled)?;
    let record = cmd::parse(raw)?;
    let kind = cmd::text(&profile, "record_type")?;
    let base = expected_path
        .rsplit_once('/')
        .map(|(_, base)| base)
        .unwrap_or(expected_path);
    if base != cmd::text(&profile, "source_basename")?
        || cmd::text(&record, "record_type")? != kind
        || cmd::text(&record, "record_id")? != selected_id
        || cmd::text(&record, "visibility")? != "local_only"
        || !crate::source_revisions::valid_id(selected_id, cmd::text(&profile, "id_prefix")?, false)
        || !matches!(
            cmd::text(&record, "identity_status")?,
            "provisional" | "verified" | "disputed" | "superseded"
        )
        || cmd::text(&record, "preferred_label")?.trim().is_empty()
        || cmd::integer(&record, "record_version")? == 0
    {
        return Err(SourceCommandError::Denied(
            "private profile exact identity, visibility or metadata differs",
        ));
    }
    let schema_version = cmd::text(&record, "schema_version")?;
    let (root, refs) = profile_schema_refs(&profile, schema_version)?;
    schema_instance(
        ctx,
        worker,
        expected_path,
        &refs,
        &root,
        &record,
        deadline,
        cancelled,
    )?;
    let mut dependencies = BTreeMap::new();
    for reference in [PROFILE_REGISTRY, PROFILE_CONTRACT, CONTEXT_CONTRACT]
        .into_iter()
        .chain(refs.iter().map(String::as_str))
    {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("private profile dependency path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "selected profile dependency absent",
        ))?;
        dependencies.insert(path, Digest256::of_bytes(raw));
    }
    Ok(SelectedProfileRecord {
        profile,
        record,
        source_path: expected_path.to_owned(),
        source_digest: Digest256::of_bytes(raw).to_hex(),
        dependencies: dependencies
            .into_iter()
            .map(|(path, raw_sha256)| SourceDependency { path, raw_sha256 })
            .collect(),
    })
}

/// Validate one selected private semantic profile record for exact Claim
/// grounding. The caller retains access/binding/current-rights authority.
pub(crate) fn validate_identity_record(
    profile_type_id: &str,
    expected_path: &str,
    selected_id: &str,
    raw: &[u8],
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedProfileRecord> {
    validate_profile_record(
        ctx,
        cut,
        profile_type_id,
        expected_path,
        selected_id,
        raw,
        worker,
        deadline,
        cancelled,
    )
}

fn validate_context_selection(
    owner: &OwnerTextContext,
    context_config: &JsonValue,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        context_config,
        &[
            "schema_version",
            "store_id",
            "public_root",
            "private_root",
            "private_prefix",
        ],
    )?;
    if cmd::text(context_config, "schema_version")? != "tos_owner_local_source_context_v1"
        || cmd::text(context_config, "public_root")? != owner.public_root().to_str().unwrap_or("")
        || cmd::text(context_config, "private_root")? != owner.private_root().to_str().unwrap_or("")
    {
        return Err(SourceCommandError::Conflict(
            "owner-local profile context selection differs",
        ));
    }
    let selected_prefix = owner
        .private_identity_home()
        .strip_prefix(owner.private_root())
        .ok()
        .and_then(|path| path.to_str())
        .map(|path| format!("{path}/"))
        .ok_or(SourceCommandError::Conflict(
            "owner-local profile namespace is not selected",
        ))?;
    let store_id = cmd::text(context_config, "store_id")?;
    if store_id.len() != 36
        || !store_id.starts_with("sid-")
        || !store_id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit)
        || store_id.as_bytes()[4..].iter().any(u8::is_ascii_uppercase)
        || cmd::text(context_config, "private_prefix")?
            != format!("ToS/source-witnesses/owner-local/{store_id}/")
        || cmd::text(context_config, "private_prefix")? != selected_prefix
    {
        return Err(SourceCommandError::Invalid(
            "owner-local profile context prefix",
        ));
    }
    Ok(())
}

fn parse_grant(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Grant> {
    validate_context_selection(owner, context_config)?;
    if ctx.configuration_raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "private profile grant byte budget",
        ));
    }
    let value = cmd::parse(&ctx.configuration_raw)?;
    cmd::exact_keys(
        &value,
        &[
            "schema_version",
            "uid",
            "principal_id",
            "authority_ref",
            "expires_at",
            "source_context_ref",
            "source_path",
            "source_access",
            "source_binding",
            "profile_type_id",
            "record_id",
            "allowed_operations",
            "allowed_fields",
            "allowed_form_ids",
            "provenance_event_id",
        ],
    )?;
    if cmd::text(&value, "schema_version")? != CONFIG
        || cmd::integer(&value, "uid")? != ctx.effective_uid
        || !cmd::nonblank(cmd::text(&value, "principal_id")?)
        || !cmd::nonblank(cmd::text(&value, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied("private profile local identity"));
    }
    cmd::validate_expiry(cmd::text(&value, "expires_at")?, &ctx.recorded_at)?;
    if !cmd::text(&value, "source_context_ref")?.starts_with('/') {
        return Err(SourceCommandError::Denied(
            "private profile context reference",
        ));
    }
    let operations = text_array(&value, "allowed_operations", 32)?;
    let fields = text_array(&value, "allowed_fields", 32)?;
    let form_ids = text_array(&value, "allowed_form_ids", 32)?;
    if operations
        .iter()
        .any(|op| !OPERATIONS.contains(&op.as_str()))
        || fields
            .iter()
            .any(|field| !REVISION_FIELDS.contains(&field.as_str()))
        || form_ids.iter().any(|id| !form_id(id))
        || !event_id(cmd::text(&value, "provenance_event_id")?)
    {
        return Err(SourceCommandError::Denied(
            "private profile scope exceeds its exact grant",
        ));
    }
    let access = cmd::field(&value, "source_access")?;
    cmd::exact_keys(access, &["read_scope", "access_allowed", "authority_ref"])?;
    if !matches!(
        cmd::text(access, "read_scope")?,
        "metadata_only" | "exact_owner_local"
    ) || cmd::field(access, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(access, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied(
            "private profile source access selection",
        ));
    }
    let binding = cmd::field(&value, "source_binding")?;
    let source_binding = if binding.is_null() {
        None
    } else if binding.as_object().is_some() {
        Some(binding.clone())
    } else {
        return Err(SourceCommandError::Invalid(
            "private profile native binding object",
        ));
    };
    let profile_type_id = cmd::text(&value, "profile_type_id")?.to_owned();
    let profile = selected_profile(ctx, cut, &profile_type_id, worker, deadline, cancelled)?;
    if profile.object_get("creation_gate").is_some()
        && operations.iter().any(|op| op == "source.create")
    {
        return Err(SourceCommandError::Denied(
            "private source creation needs its explicit profile promotion adapter",
        ));
    }
    let source_path_text = cmd::text(&value, "source_path")?;
    let source_path = RelativePath::parse(source_path_text)
        .map_err(|_| SourceCommandError::Denied("unsafe private profile source path"))?;
    let private_prefix = cmd::text(context_config, "private_prefix")?;
    let relative =
        source_path_text
            .strip_prefix(private_prefix)
            .ok_or(SourceCommandError::Denied(
                "private profile source namespace",
            ))?;
    let relative_parts = relative.split('/').collect::<Vec<_>>();
    let expected_basename = cmd::text(&profile, "source_basename")?;
    if relative_parts.len() < 2
        || relative_parts.last().copied() != Some(expected_basename)
        || relative_parts.iter().any(|part| {
            part.is_empty()
                || part.starts_with('.')
                || matches!(*part, "payload" | "local-content" | "catalog")
        })
    {
        return Err(SourceCommandError::Denied(
            "private profile typed metadata path",
        ));
    }
    let record_id = cmd::text(&value, "record_id")?;
    if !crate::source_revisions::valid_id(record_id, cmd::text(&profile, "id_prefix")?, false) {
        return Err(SourceCommandError::Denied(
            "private profile record identity",
        ));
    }
    let access_scope = cmd::text(access, "read_scope")?;
    let adapter = profile.object_get("native_binding_adapter");
    match adapter {
        None => {
            if source_binding.is_some() || access_scope != "metadata_only" {
                return Err(SourceCommandError::Denied(
                    "non-native profile cannot acquire native text read scope",
                ));
            }
        }
        Some(adapter) => {
            if cmd::text(&profile, "record_type")? != "occurrence"
                || adapter.as_str() != Some("source-text-unit-v1")
                || source_binding.is_none()
                || access_scope != "exact_owner_local"
            {
                return Err(SourceCommandError::Denied(
                    "private occurrence needs its exact native binding adapter and read scope",
                ));
            }
        }
    }
    let context_snapshot = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let mut profile_inputs = BTreeMap::new();
    for reference in [PROFILE_REGISTRY, PROFILE_CONTRACT, CONTEXT_CONTRACT] {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("profile configuration input path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "profile configuration input absent",
        ))?;
        profile_inputs.insert(reference.to_owned(), Digest256::of_bytes(raw).to_hex());
    }
    let digest_basis = cmd::object(vec![
        (
            "configuration_bytes",
            cmd::string(&Digest256::of_bytes(&ctx.configuration_raw).to_prefixed()),
        ),
        ("context", cmd::string(&context_snapshot)),
        ("profile_inputs", hash_map(profile_inputs)),
    ]);
    let digest = cmd::record_digest(&digest_basis)?.to_prefixed();
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let source_access = access.clone();
    let home = format!(
        "{}{}/",
        private_prefix,
        relative_parts[..relative_parts.len() - 1].join("/")
    );
    Ok(Grant {
        value,
        digest,
        source_path,
        profile,
        profile_type_id,
        private_prefix: private_prefix.to_owned(),
        home,
        form_ids,
        operations,
        fields,
        source_access,
        source_binding,
    })
}

fn implementation_digests(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    if components.capture() != software.selection() {
        return Err(SourceCommandError::Conflict(
            "private profile implementation capture differs",
        ));
    }
    let mut result = BTreeMap::new();
    let mut remaining = 16_777_216usize;
    for reference in IMPLEMENTATIONS {
        active(deadline, cancelled)?;
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("profile implementation path"))?;
        let member = components
            .member(&path)
            .ok_or(SourceCommandError::Unsupported(
                "selected profile rule-source component absent",
            ))?;
        if member.size_bytes as usize > remaining {
            return Err(SourceCommandError::Invalid(
                "profile implementation byte budget",
            ));
        }
        let raw = software
            .read_selected_component(
                components,
                &path,
                MAX_INPUT_BYTES as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("profile rule-source capture changed"))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "profile rule-source bytes differ from selected software capture",
            ));
        }
        remaining = remaining.saturating_sub(raw.len());
        result.insert(
            (*reference).to_owned(),
            Digest256::of_bytes(&raw).to_prefixed(),
        );
    }
    Ok(hash_map(result))
}

pub(crate) fn form_grammar_digests(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let mut digests = BTreeMap::new();
    for reference in FORM_CONTRACTS {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("private form grammar path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "selected private form grammar absent",
        ))?;
        let schema = cmd::parse(raw)?;
        let id = cmd::text(&schema, "$id")?;
        if id != format!("https://tree-of-sophia.local/{reference}")
            && id != format!("https://treeofsophia.local/{reference}")
        {
            return Err(SourceCommandError::Invalid("private form schema identity"));
        }
        if worker.contract_digest(reference) != Some(Digest256::of_bytes(raw)) {
            return Err(SourceCommandError::Conflict(
                "private form schema worker differs",
            ));
        }
        digests.insert(
            (*reference).to_owned(),
            Digest256::of_bytes(raw).to_prefixed(),
        );
    }
    active(deadline, cancelled)?;
    Ok(hash_map(digests))
}

pub(crate) fn validate_form_set(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    source_path: &str,
    set: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let _ = form_grammar_digests(ctx, worker, deadline, cancelled)?;
    let mut forms = Vec::new();
    forms.extend(cmd::array(set, "forms")?.iter());
    forms.extend(cmd::array(set, "prior_forms")?.iter());
    for form in forms {
        let raw = cmd::canonical(form)?;
        match worker.check(
            source_path,
            &raw,
            "ToS/contracts/human-form.schema.json",
            deadline,
            cancelled,
        ) {
            Ok(true) => (),
            Ok(false) => {
                return Err(SourceCommandError::Invalid(
                    "private source form schema rejected bytes",
                ));
            }
            Err(reason) => {
                return Err(SourceCommandError::SchemaExecution {
                    path: source_path.to_owned(),
                    root: "ToS/contracts/human-form.schema.json".into(),
                    reason,
                });
            }
        }
        if let Some(assessment) = form
            .object_get("content")
            .and_then(|content| content.object_get("assessment"))
        {
            let raw = cmd::canonical(assessment)?;
            match worker.check(
                source_path,
                &raw,
                "ToS/contracts/knowledge-assessment.schema.json",
                deadline,
                cancelled,
            ) {
                Ok(true) => (),
                Ok(false) => {
                    return Err(SourceCommandError::Invalid(
                        "private source form assessment schema rejected bytes",
                    ));
                }
                Err(reason) => {
                    return Err(SourceCommandError::SchemaExecution {
                        path: source_path.to_owned(),
                        root: "ToS/contracts/knowledge-assessment.schema.json".into(),
                        reason,
                    });
                }
            }
        }
    }
    let raw = cmd::canonical(set)?;
    match worker.check(
        source_path,
        &raw,
        "ToS/contracts/human-form-set.schema.json",
        deadline,
        cancelled,
    ) {
        Ok(true) => {
            tos_validation::source_forms::inspect_lineage_raw(&cmd::published(set)?)
                .map_err(|_| SourceCommandError::Invalid("private form-set retained lineage"))?;
            Ok(())
        }
        Ok(false) => Err(SourceCommandError::Invalid(
            "private source form-set schema rejected bytes",
        )),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: source_path.to_owned(),
            root: "ToS/contracts/human-form-set.schema.json".into(),
            reason,
        }),
    }
}

fn package_revision(files: &PrivatePackage) -> SourceCommandResult<String> {
    crate::source_revisions::revision(files)
}

fn archive_reference(
    private_prefix: &str,
    record_id: &str,
    revision: &str,
) -> SourceCommandResult<String> {
    let revision = revision
        .strip_prefix("sha256:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or(SourceCommandError::Invalid(
            "private profile archive revision",
        ))?;
    let id = Digest256::of_bytes(record_id.as_bytes()).to_hex();
    Ok(format!(
        "{}.record-revisions/{}-{revision}",
        private_prefix, id
    ))
}

/// Exact archive locators needed for cold history validation. It does not read
/// or interpret archive bytes; `prepare` performs that bounded streamed work.
pub(crate) fn required_archives(
    context_config: &JsonValue,
    current: &PrivatePackage,
) -> SourceCommandResult<Vec<String>> {
    let config_raw = current.get(CONFIG_FILE).ok_or(SourceCommandError::Invalid(
        "private profile retained configuration absent",
    ))?;
    let config = cmd::parse(config_raw)?;
    let prefix = cmd::text(context_config, "private_prefix")?;
    let record_id = cmd::text(&config, "record_id")?;
    let history = if let Some(raw) = current.get(HISTORY) {
        cmd::parse(raw)?
    } else {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_revision_history_v1"),
            ),
            ("record_id", cmd::string(record_id)),
            ("receipts", JsonValue::Array(Vec::new())),
        ])
    };
    cmd::exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
    if !matches!(
        cmd::text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || cmd::text(&history, "record_id")? != record_id
    {
        return Err(SourceCommandError::Invalid(
            "private profile history header",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.len() > MAX_REVISIONS || current.contains_key(HISTORY) && receipts.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private profile revision history budget",
        ));
    }
    let mut refs = Vec::with_capacity(receipts.len());
    let mut seen = BTreeSet::new();
    for receipt in receipts {
        let revision = cmd::text(receipt, "previous_revision")?;
        let locator = archive_reference(prefix, record_id, revision)?;
        if cmd::text(receipt, "archive_path")? != locator || !seen.insert(locator.clone()) {
            return Err(SourceCommandError::Invalid(
                "private profile archive locator",
            ));
        }
        refs.push(locator);
    }
    Ok(refs)
}

fn validate_revision_history(
    files: &PrivatePackage,
    record: &JsonValue,
    grant: &Grant,
) -> SourceCommandResult<JsonValue> {
    let subject = source_forms::metadata_subject(record)?;
    let history = if let Some(raw) = files.get(HISTORY) {
        cmd::parse(raw)?
    } else {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_revision_history_v1"),
            ),
            ("record_id", cmd::string(&cmd::text(record, "record_id")?)),
            ("receipts", JsonValue::Array(Vec::new())),
        ])
    };
    cmd::exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
    if !matches!(
        cmd::text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || cmd::text(&history, "record_id")? != cmd::text(record, "record_id")?
    {
        return Err(SourceCommandError::Invalid(
            "private profile revision history identity",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.len() > MAX_REVISIONS || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private profile revision history count",
        ));
    }
    let mut previous: Option<JsonValue> = None;
    let mut commands = BTreeSet::new();
    for receipt in receipts {
        cmd::exact_keys(
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
                "changed_fields",
                "forms",
                "grants_admission",
                "request",
            ],
        )?;
        let command_id = cmd::text(receipt, "command_id")?;
        if !(1..=256).contains(&command_id.chars().count())
            || !commands.insert(command_id.to_owned())
        {
            return Err(SourceCommandError::Invalid(
                "private profile revision command identity",
            ));
        }
        for key in ["request_digest", "owner_configuration", "dependencies"] {
            if !digest_text(cmd::text(receipt, key)?) {
                return Err(SourceCommandError::Invalid(
                    "private profile revision digest",
                ));
            }
        }
        cmd::validate_instant(cmd::text(receipt, "recorded_at")?)?;
        let reason = cmd::text(receipt, "reason")?;
        if !cmd::nonblank(reason) || reason.chars().count() > 4096 {
            return Err(SourceCommandError::Invalid(
                "private profile revision reason",
            ));
        }
        if cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
            || !cmd::nonblank(cmd::text(receipt, "principal_id")?)
            || !cmd::nonblank(cmd::text(receipt, "authority_ref")?)
        {
            return Err(SourceCommandError::Invalid(
                "private profile revision receipt posture",
            ));
        }
        let request = cmd::field(receipt, "request")?;
        if cmd::text(request, "operation")? != "record.revise"
            || cmd::text(request, "command_id")? != command_id
            || Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()
                != cmd::text(receipt, "request_digest")?
        {
            return Err(SourceCommandError::Invalid(
                "private profile retained revision request",
            ));
        }
        let request_source = cmd::field(receipt, "previous_source")?.clone();
        if let Some(expected) = previous.as_ref()
            && !cmd::same(expected, &request_source)?
        {
            return Err(SourceCommandError::Invalid(
                "private profile revision source chain",
            ));
        }
        if previous.is_none() && cmd::field(&subject, "id")? != cmd::field(&request_source, "id")? {
            return Err(SourceCommandError::Invalid(
                "private profile revision record identity",
            ));
        }
        let changed = text_array(receipt, "changed_fields", 32)?;
        let fields = cmd::field(request, "fields")?;
        let field_names = fields
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "private profile revision fields",
            ))?
            .iter()
            .map(|(key, _)| {
                key.as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "private profile revision field name",
                    ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let mut sorted = field_names.clone();
        sorted.sort();
        let mut changed_sorted = changed.clone();
        changed_sorted.sort();
        if field_names.is_empty()
            || field_names
                .iter()
                .any(|name| !REVISION_FIELDS.contains(&name.as_str()))
            || sorted != changed_sorted
        {
            return Err(SourceCommandError::Invalid(
                "private profile revision field or form receipt",
            ));
        }
        if cmd::array(receipt, "forms")?.len() > 32 {
            return Err(SourceCommandError::Invalid(
                "private profile revision form receipt budget",
            ));
        }
        let expected_archive = archive_reference(
            &grant.private_prefix,
            cmd::text(&grant.value, "record_id")?,
            cmd::text(receipt, "previous_revision")?,
        )?;
        if cmd::text(receipt, "archive_path")? != expected_archive {
            return Err(SourceCommandError::Invalid(
                "private profile revision archive reference",
            ));
        }
        previous = Some(cmd::field(receipt, "source")?.clone());
    }
    if let Some(last) = previous
        && !cmd::same(&last, &subject)?
    {
        return Err(SourceCommandError::Invalid(
            "private profile current source is not history tip",
        ));
    }
    Ok(history)
}

fn decode_archive(
    reference: &str,
    raw_files: PrivatePackage,
    receipt: &JsonValue,
    grant: &Grant,
) -> SourceCommandResult<(PrivatePackage, JsonValue)> {
    if reference
        != archive_reference(
            &grant.private_prefix,
            cmd::text(&grant.value, "record_id")?,
            cmd::text(receipt, "previous_revision")?,
        )?
    {
        return Err(SourceCommandError::Invalid(
            "private profile archive reference differs",
        ));
    }
    let mut content = raw_files;
    let manifest_raw = content
        .remove("manifest.json")
        .ok_or(SourceCommandError::Invalid(
            "private profile archive manifest absent",
        ))?;
    let manifest = cmd::parse(&manifest_raw)?;
    cmd::exact_keys(
        &manifest,
        &[
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ],
    )?;
    if cmd::text(&manifest, "schema_version")? != "tos_source_package_archive_v1"
        || cmd::text(&manifest, "source_path")? != cmd::text(&grant.value, "source_path")?
        || !cmd::same(
            cmd::field(&manifest, "source")?,
            cmd::field(receipt, "previous_source")?,
        )?
        || cmd::text(&manifest, "revision")? != cmd::text(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Invalid(
            "private profile archive subject or revision",
        ));
    }
    let rows = cmd::field(&manifest, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "private profile archive file map",
        ))?;
    if rows.is_empty() || rows.len() > 64 {
        return Err(SourceCommandError::Invalid(
            "private profile archive file count",
        ));
    }
    let mut restored = BTreeMap::new();
    let mut locations = BTreeMap::new();
    let mut expected_blobs = BTreeSet::new();
    for (name, binding) in rows {
        let name = name.as_str().ok_or(SourceCommandError::Invalid(
            "private archive source file name",
        ))?;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(SourceCommandError::Invalid(
                "private archive source file name",
            ));
        }
        cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let sha = cmd::text(binding, "sha256")?;
        if !digest_text(sha) {
            return Err(SourceCommandError::Invalid("private archive source digest"));
        }
        let blob = format!("{}.blob", sha.trim_start_matches("sha256:"));
        if cmd::text(binding, "blob")? != blob || !expected_blobs.insert(blob.clone()) {
            return Err(SourceCommandError::Invalid("private archive blob name"));
        }
        let raw = content
            .get(&blob)
            .ok_or(SourceCommandError::Invalid("private archive blob absent"))?;
        if Digest256::of_bytes(raw).to_prefixed() != sha
            || cmd::integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "private archive blob fixity differs",
            ));
        }
        let archive_path = format!("{reference}/{blob}");
        locations.insert(
            name.to_owned(),
            cmd::object(vec![
                ("archive_path", cmd::string(&archive_path)),
                ("sha256", cmd::string(sha)),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        );
        restored.insert(name.to_owned(), raw.clone());
    }
    if content.keys().cloned().collect::<BTreeSet<_>>() != expected_blobs
        || package_revision(&restored)? != cmd::text(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "private profile archive has unbound bytes or wrong package revision",
        ));
    }
    let basename = cmd::text(&grant.value, "source_path")?
        .rsplit('/')
        .next()
        .ok_or(SourceCommandError::Invalid(
            "private profile source basename",
        ))?;
    let old_raw = restored.get(basename).ok_or(SourceCommandError::Invalid(
        "private profile archive source absent",
    ))?;
    let old = cmd::parse(old_raw)?;
    let old_subject = source_forms::metadata_subject(&old)?;
    if !cmd::same(&old_subject, cmd::field(receipt, "previous_source")?)? {
        return Err(SourceCommandError::Conflict(
            "private profile archived record differs from its exact source ref",
        ));
    }
    let request = cmd::field(receipt, "request")?;
    if let Some(fields) = request.object_get("fields") {
        let successor = revise_record(&old, fields)?;
        let subject = source_forms::metadata_subject(&successor)?;
        if !cmd::same(&subject, cmd::field(receipt, "source")?)? {
            return Err(SourceCommandError::Conflict(
                "private profile retained revision does not produce its recorded source",
            ));
        }
    }
    Ok((
        restored,
        JsonValue::Object(
            locations
                .into_iter()
                .map(|(name, value)| (JsonString::from_utf8(&name), value))
                .collect(),
        ),
    ))
}

fn archive_package(
    files: &PrivatePackage,
    private_prefix: &str,
    source_path: &str,
    subject: &JsonValue,
    revision: &str,
) -> SourceCommandResult<(String, PrivatePackage)> {
    let source_path = RelativePath::parse(source_path)
        .map_err(|_| SourceCommandError::Invalid("private profile archive source path"))?;
    let reference = archive_reference(
        private_prefix,
        cmd::text(
            &cmd::parse(files.get(CONFIG_FILE).ok_or(SourceCommandError::Invalid(
                "private profile archive configuration absent",
            ))?)?,
            "record_id",
        )?,
        revision,
    )?;
    let mut refs = BTreeMap::new();
    let mut content = BTreeMap::new();
    for (name, raw) in files {
        let digest = Digest256::of_bytes(raw).to_prefixed();
        let blob = format!("{}.blob", digest.trim_start_matches("sha256:"));
        refs.insert(
            name.clone(),
            cmd::object(vec![
                ("blob", cmd::string(&blob)),
                ("sha256", cmd::string(&digest)),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        );
        content.insert(blob, raw.clone());
    }
    let manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_source_package_archive_v1"),
        ),
        ("source_path", cmd::string(source_path.as_str())),
        ("source", subject.clone()),
        ("revision", cmd::string(revision)),
        (
            "files",
            JsonValue::Object(
                refs.into_iter()
                    .map(|(name, value)| (JsonString::from_utf8(&name), value))
                    .collect(),
            ),
        ),
    ]);
    content.insert("manifest.json".into(), encode(&manifest)?);
    Ok((reference, content))
}

fn formset_name(grant: &Grant) -> SourceCommandResult<String> {
    let base = grant
        .source_path
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(SourceCommandError::Invalid(
            "private profile source basename",
        ))?;
    let stem = base
        .strip_suffix(".json")
        .ok_or(SourceCommandError::Invalid("private profile JSON basename"))?;
    Ok(format!("{stem}.human-forms.json"))
}

fn form_refs(forms: &JsonValue) -> SourceCommandResult<Vec<JsonValue>> {
    cmd::array(forms, "forms")?
        .iter()
        .map(source_forms::form_reference)
        .collect()
}

fn parse_form_payload(
    ctx: &CommandContext,
    files: &PrivatePackage,
    grant: &Grant,
    record: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let name = formset_name(grant)?;
    let raw = files.get(&name).ok_or(SourceCommandError::Invalid(
        "private profile human-form set absent",
    ))?;
    let forms = cmd::parse(raw)?;
    let form_path = format!(
        "{}.human-forms.json",
        grant.source_path.as_str().trim_end_matches(".json")
    );
    validate_form_set(ctx, worker, &form_path, &forms, deadline, cancelled)?;
    let subject = source_forms::metadata_subject(record)?;
    if !cmd::same(cmd::field(&forms, "subject")?, &subject)? {
        return Err(SourceCommandError::Conflict(
            "private profile form subject differs",
        ));
    }
    Ok(forms)
}

fn form_selections(
    request: &JsonValue,
    grant: &Grant,
    record: &JsonValue,
    current: Option<&JsonValue>,
    rebind: bool,
) -> SourceCommandResult<(JsonValue, Vec<JsonValue>, Vec<JsonValue>)> {
    let selections = cmd::array(request, "forms")?;
    if !(1..=32).contains(&selections.len()) {
        return Err(SourceCommandError::Invalid(
            "private profile requires one to thirty-two form selections",
        ));
    }
    let mut seen = BTreeSet::new();
    for selection in selections {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let id = cmd::text(selection, "form_id")?;
        if !grant.form_ids.iter().any(|allowed| allowed == id) || !seen.insert(id.to_owned()) {
            return Err(SourceCommandError::Denied(
                "private profile form identity is not delegated or repeats",
            ));
        }
    }
    let subject = source_forms::metadata_subject(record)?;
    if let Some(current) = current {
        if rebind {
            for form in cmd::array(current, "forms")? {
                let id = cmd::text(form, "form_id")?;
                if !seen.contains(id) {
                    return Err(SourceCommandError::Invalid(
                        "private profile revision must rebind each current form",
                    ));
                }
            }
        }
    }
    let changes = selections
        .iter()
        .map(|selection| {
            source_forms::prepare_form_change(
                record,
                current,
                cmd::text(&grant.value, "principal_id")?,
                cmd::text(selection, "form_id")?,
                cmd::text(selection, "field_id")?,
            )
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    for change in &changes {
        let operation = cmd::text(change, "operation")?;
        if !grant.operations.iter().any(|allowed| allowed == operation) {
            return Err(SourceCommandError::Denied(
                "private profile form operation is outside its exact grant",
            ));
        }
    }
    let forms = source_forms::apply_form_changes(current, &subject, &changes)?;
    let refs = changes
        .iter()
        .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok((forms, changes, refs))
}

fn validate_ready_forms(
    record: &JsonValue,
    forms: &JsonValue,
) -> SourceCommandResult<Vec<JsonValue>> {
    let views = source_forms::materialize_source_forms(record, forms)?;
    if !views
        .iter()
        .all(|view| cmd::text(view, "state").ok() == Some("ready"))
        || !views
            .iter()
            .any(|view| cmd::text(view, "role").ok() == Some("name"))
    {
        return Err(SourceCommandError::Invalid(
            "private profile source-copy forms must be ready and include a name",
        ));
    }
    Ok(views)
}

fn validate_creation_receipt(receipt: &JsonValue, record: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        receipt,
        &[
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "source",
            "dependencies",
            "files",
            "grants_admission",
        ],
    )?;
    if cmd::text(receipt, "schema_version")? != "tos_local_source_create_receipt_v1"
        || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
        || !digest_text(cmd::text(receipt, "request_digest")?)
        || !digest_text(cmd::text(receipt, "owner_configuration")?)
        || !digest_text(cmd::text(receipt, "dependencies")?)
        || !(1..=256).contains(&cmd::text(receipt, "command_id")?.chars().count())
        || !cmd::nonblank(cmd::text(receipt, "principal_id")?)
        || !cmd::nonblank(cmd::text(receipt, "authority_ref")?)
        || cmd::text(receipt, "source_path")?.is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "private profile creation receipt posture",
        ));
    }
    cmd::validate_instant(cmd::text(receipt, "recorded_at")?)?;
    let subject = source_forms::metadata_subject(record)?;
    let source = cmd::field(receipt, "source")?;
    if cmd::field(source, "id")? != cmd::field(&subject, "id")?
        || cmd::integer(source, "version")? != 1
    {
        return Err(SourceCommandError::Conflict(
            "private profile creation receipt source identity differs",
        ));
    }
    let file_map = cmd::field(receipt, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "private profile creation receipt file map",
        ))?;
    if file_map.len() != 6
        || ![
            CONFIG_FILE,
            CREATE_REQUEST,
            CREATE_ENVIRONMENT,
            CREATE_PROVENANCE,
        ]
        .iter()
        .all(|key| file_map.iter().any(|(name, _)| name.as_str() == Some(key)))
    {
        return Err(SourceCommandError::Invalid(
            "private profile creation receipt file scope",
        ));
    }
    for (_, binding) in file_map {
        cmd::exact_keys(binding, &["sha256", "bytes"])?;
        if !digest_text(cmd::text(binding, "sha256")?) {
            return Err(SourceCommandError::Invalid(
                "private profile creation file digest",
            ));
        }
        let _ = cmd::integer(binding, "bytes")?;
    }
    Ok(())
}

fn inspect_state(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    grant: &Grant,
    files: &PrivatePackage,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<State> {
    let basename =
        grant
            .source_path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid(
                "private profile source basename",
            ))?;
    let required = [
        basename,
        &formset_name(grant)?,
        CONFIG_FILE,
        RECEIPT_FILE,
        CREATE_REQUEST,
        CREATE_ENVIRONMENT,
        CREATE_PROVENANCE,
    ];
    if required.iter().any(|name| !files.contains_key(*name)) {
        return Err(SourceCommandError::Invalid(
            "private profile package lacks its source, forms, request, capture or receipt",
        ));
    }
    if files.len() > 64
        || files.values().any(|raw| raw.len() > 8_388_608)
        || files
            .values()
            .try_fold(0usize, |total, raw| total.checked_add(raw.len()))
            .is_none_or(|total| total > 8_388_608)
    {
        return Err(SourceCommandError::Invalid(
            "private profile package byte budget",
        ));
    }
    let allowed = [
        basename.to_owned(),
        formset_name(grant)?,
        CONFIG_FILE.into(),
        RECEIPT_FILE.into(),
        CREATE_REQUEST.into(),
        CREATE_ENVIRONMENT.into(),
        CREATE_PROVENANCE.into(),
        HISTORY.into(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if files.keys().any(|name| !allowed.contains(name)) {
        return Err(SourceCommandError::Invalid(
            "private profile package contains an unowned file",
        ));
    }
    let retained_config = cmd::parse(&files[CONFIG_FILE])?;
    if cmd::text(&retained_config, "schema_version")? != CONFIG
        || cmd::text(&retained_config, "source_path")? != grant.source_path.as_str()
        || cmd::text(&retained_config, "record_id")? != cmd::text(&grant.value, "record_id")?
        || cmd::text(&retained_config, "profile_type_id")? != grant.profile_type_id
    {
        return Err(SourceCommandError::Conflict(
            "private profile retained owner configuration identifies another package",
        ));
    }
    let record_raw = files
        .get(basename)
        .ok_or(SourceCommandError::Invalid("private profile record absent"))?;
    let selected = validate_profile_record(
        ctx,
        cut,
        &grant.profile_type_id,
        grant.source_path.as_str(),
        cmd::text(&grant.value, "record_id")?,
        record_raw,
        worker,
        deadline,
        cancelled,
    )?;
    let record = selected.record;
    let subject = source_forms::metadata_subject(&record)?;
    let forms = parse_form_payload(ctx, files, grant, &record, worker, deadline, cancelled)?;
    let history = validate_revision_history(files, &record, grant)?;
    let receipt = cmd::parse(&files[RECEIPT_FILE])?;
    validate_creation_receipt(&receipt, &record)?;
    let initial_request = cmd::parse(&files[CREATE_REQUEST])?;
    if cmd::text(&initial_request, "operation")? != "source.create"
        || Digest256::of_bytes(&cmd::canonical(&initial_request)?).to_prefixed()
            != cmd::text(&receipt, "request_digest")?
    {
        return Err(SourceCommandError::Conflict(
            "private profile retained creation request differs from receipt",
        ));
    }
    let retained_refs = cmd::array(&forms, "forms")?
        .iter()
        .chain(cmd::array(&forms, "prior_forms")?.iter())
        .map(source_forms::form_reference)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    for revision in cmd::array(&history, "receipts")? {
        for reference in cmd::array(revision, "forms")? {
            if !retained_refs
                .iter()
                .any(|retained| cmd::same(retained, reference).unwrap_or(false))
            {
                return Err(SourceCommandError::Invalid(
                    "private profile revision form is absent from retained lineage",
                ));
            }
        }
    }
    let _ = source_forms::materialize_source_forms(&record, &forms)?;
    Ok(State {
        files: files.clone(),
        record,
        subject,
        history,
        forms,
    })
}

fn identity_file_selected(reference: &str, basenames: &BTreeSet<String>, creating: bool) -> bool {
    let base = reference.rsplit('/').next().unwrap_or(reference);
    basenames.contains(base)
        || base.ends_with(".human-forms.json")
        || base.starts_with("semantic-annotation") && base.ends_with(".json")
        || creating && base.contains("provenance") && base.ends_with(".jsonl")
}

fn reserved_ids(value: &JsonValue) -> SourceCommandResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for key in ["record_id", "claim_id", "event_id"] {
        if let Some(id) = value.object_get(key).and_then(JsonValue::as_str) {
            result.insert(id.to_owned());
        }
    }
    for (array_key, member_key) in [
        ("forms", "form_id"),
        ("prior_forms", "form_id"),
        ("occurrences", "occurrence_id"),
        ("lexemes", "lexeme_id"),
        ("senses", "sense_id"),
        ("signs", "sign_id"),
        ("concepts", "concept_id"),
        ("entities", "entity_id"),
        ("claims", "claim_id"),
    ] {
        let Some(values) = value.object_get(array_key) else {
            continue;
        };
        let values = values.as_array().ok_or(SourceCommandError::Invalid(
            "identity inventory record list",
        ))?;
        for row in values {
            if let Some(id) = row.object_get(member_key).and_then(JsonValue::as_str) {
                result.insert(id.to_owned());
            }
        }
    }
    Ok(result)
}

fn scan_identity_bytes(
    reference: &str,
    raw: &[u8],
    reserved: &BTreeSet<String>,
    digests: &mut BTreeMap<String, String>,
    count: &mut usize,
    remaining: &mut usize,
) -> SourceCommandResult<()> {
    if raw.len() > MAX_INVENTORY_FILE_BYTES || raw.len() > *remaining {
        return Err(SourceCommandError::Invalid(
            "profile identity file byte budget",
        ));
    }
    *remaining -= raw.len();
    *count += 1;
    if *count > MAX_INVENTORY_FILES {
        return Err(SourceCommandError::Invalid("profile identity file count"));
    }
    if digests
        .insert(reference.to_owned(), Digest256::of_bytes(raw).to_prefixed())
        .is_some()
    {
        return Err(SourceCommandError::Invalid(
            "duplicate profile identity input",
        ));
    }
    let mut rows = raw.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if rows.last().is_some_and(|row| row.is_empty()) {
        rows.pop();
    }
    if rows.len() > 65_536 {
        return Err(SourceCommandError::Invalid(
            "profile identity JSONL row budget",
        ));
    }
    for row in rows {
        if row.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value = cmd::parse(row)?;
        if !reserved.is_disjoint(&reserved_ids(&value)?) {
            return Err(SourceCommandError::Conflict(
                "private identity is already owned by another selected source",
            ));
        }
    }
    Ok(())
}

fn identity_snapshot(
    ctx: &CommandContext,
    private: &PrivateIdentityInputs,
    basenames: &BTreeSet<String>,
    reserved: &BTreeSet<String>,
    creating: bool,
) -> SourceCommandResult<String> {
    let mut digests = BTreeMap::new();
    let mut count = 0usize;
    let mut remaining = MAX_INVENTORY_BYTES;
    let public_entries = ctx
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/source-witnesses/"))
        .count();
    if public_entries.saturating_add(private.visited_entries) > MAX_INVENTORY_ENTRIES {
        return Err(SourceCommandError::Invalid(
            "profile identity directory-entry budget",
        ));
    }
    for file in &ctx.files {
        let reference = file.path.as_str();
        if !reference.starts_with("ToS/source-witnesses/")
            || reference.split('/').any(|part| {
                part.starts_with('.')
                    || matches!(
                        part,
                        "owner-local" | "payload" | "local-content" | "catalog"
                    )
            })
            || !identity_file_selected(reference, basenames, creating)
        {
            continue;
        }
        scan_identity_bytes(
            reference,
            &file.raw,
            reserved,
            &mut digests,
            &mut count,
            &mut remaining,
        )?;
    }
    for (reference, raw) in &private.files {
        if !reference.starts_with("ToS/source-witnesses/owner-local/")
            || reference.split('/').any(|part| {
                part.starts_with('.') || matches!(part, "payload" | "local-content" | "catalog")
            })
            || !identity_file_selected(reference, basenames, creating)
        {
            continue;
        }
        scan_identity_bytes(
            reference,
            raw,
            reserved,
            &mut digests,
            &mut count,
            &mut remaining,
        )?;
    }
    Ok(cmd::record_digest(&hash_map(digests))?.to_prefixed())
}

fn validate_native_profile(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    grant: &Grant,
    record: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    context_config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    Option<String>,
    Option<String>,
    Option<JsonValue>,
    Vec<SourceFile>,
    BTreeMap<String, String>,
)> {
    let native_binding_adapter = grant.profile.object_get("native_binding_adapter");
    if native_binding_adapter.is_none() {
        if record.object_get("native_text_binding").is_some()
            || grant.source_binding.is_some()
            || cmd::text(&grant.source_access, "read_scope")? != "metadata_only"
        {
            return Err(SourceCommandError::Denied(
                "non-native private profile cannot acquire source content access",
            ));
        }
        return Ok((None, None, None, Vec::new(), BTreeMap::new()));
    }
    if native_binding_adapter.and_then(JsonValue::as_str) != Some("source-text-unit-v1")
        || cmd::text(&grant.profile, "record_type")? != "occurrence"
        || cmd::text(&grant.source_access, "read_scope")? != "exact_owner_local"
    {
        return Err(SourceCommandError::Unsupported(
            "private profile native binding adapter is not understood",
        ));
    }
    let binding = grant
        .source_binding
        .as_ref()
        .ok_or(SourceCommandError::Denied(
            "private occurrence binding is absent",
        ))?;
    let record_binding = cmd::field(record, "native_text_binding")?;
    if !cmd::same(binding, record_binding)? {
        return Err(SourceCommandError::Denied(
            "private occurrence binding differs from its exact delegated selection",
        ));
    }
    let mut native_reader = ProfileNativeReader(owner);
    let (mut resolved, metadata_inputs) = source_sign_native::resolve_owner_profile_binding(
        &mut native_reader,
        worker,
        binding,
        deadline,
        cancelled,
    )?;
    let private_prefix = cmd::text(context_config, "private_prefix")?;
    let mut reads = BTreeMap::<String, SourceFile>::new();
    owner.snapshot(deadline, cancelled)?;
    let mut owner_local_transport = false;
    for input in &resolved.inputs {
        active(deadline, cancelled)?;
        let private = native_reader.owner_local(&input.reference)?;
        owner_local_transport |= private;
        if !private {
            let path = RelativePath::parse(&input.reference)
                .map_err(|_| SourceCommandError::Invalid("public native input path"))?;
            let raw = ctx.file(&path)?.ok_or(SourceCommandError::Conflict(
                "native public binding input is absent from the selected corpus cut",
            ))?;
            if raw.len() != input.raw_size || Digest256::of_bytes(raw) != input.raw_sha256 {
                return Err(SourceCommandError::Conflict(
                    "native public binding input differs from the selected corpus cut",
                ));
            }
            continue;
        }
        if !input.reference.starts_with(private_prefix) {
            return Err(SourceCommandError::Denied(
                "native binding leaves the exact selected private namespace",
            ));
        }
        let raw = owner.read(&input.reference, input.raw_size, deadline, cancelled)?;
        if raw.len() != input.raw_size || Digest256::of_bytes(&raw) != input.raw_sha256 {
            return Err(SourceCommandError::Conflict(
                "private native binding input changed during profile resolution",
            ));
        }
        let path = RelativePath::parse(&input.reference)
            .map_err(|_| SourceCommandError::Invalid("private native input path"))?;
        if reads
            .insert(input.reference.clone(), SourceFile { path, raw })
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate private native binding input",
            ));
        }
    }
    owner.snapshot(deadline, cancelled)?;
    let snapshot =
        owner_metadata_snapshot(owner, &native_reader, &resolved.inputs, deadline, cancelled)?;
    let metadata_snapshot =
        owner_metadata_snapshot(owner, &native_reader, &metadata_inputs, deadline, cancelled)?;
    let summary = owner_metadata_summary(
        &mut resolved.summary,
        &resolved.packet,
        owner_local_transport,
    )?;
    let schemas = resolved
        .schema_digests
        .into_iter()
        .map(|(reference, digest)| (reference, digest.to_hex()))
        .collect();
    Ok((
        Some(snapshot),
        Some(metadata_snapshot),
        Some(summary),
        reads.into_values().collect(),
        schemas,
    ))
}

/// Preserve `NativeTextBindingResolver.snapshot()`'s owner-context JSON shape.
/// Its object uses Python insertion order (`owner_context`, then `inputs`), so
/// hash the ordered compact bytes directly instead of building Sign's sorted
/// snapshot value.
fn owner_metadata_snapshot(
    owner: &OwnerTextContext,
    reader: &ProfileNativeReader<'_>,
    inputs: &[source_sign_native::NativeInput],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let context = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let mut rows = Vec::with_capacity(inputs.len());
    for input in inputs {
        let role = if reader.owner_local(&input.reference)? {
            "owner-local-root"
        } else {
            "source-contract-root"
        };
        rows.push(JsonValue::Array(vec![
            cmd::string(&input.reference),
            cmd::string(input.category),
            cmd::string(role),
            cmd::string(&input.raw_sha256.to_hex()),
        ]));
    }
    let value = cmd::object(vec![
        ("owner_context", cmd::string(&context)),
        ("inputs", JsonValue::Array(rows)),
    ]);
    let raw = emit_python_compact_json(
        &value,
        JsonLimits {
            max_bytes: MAX_NATIVE_SNAPSHOT_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("owner metadata snapshot encoding"))?;
    let snapshot = source_sign_native::ascii_snapshot_bytes(&raw)?;
    if owner.snapshot(deadline, cancelled)?.to_prefixed() != context {
        return Err(SourceCommandError::Conflict(
            "owner metadata context changed during exact input snapshot",
        ));
    }
    Ok(snapshot)
}

fn owner_metadata_summary(
    summary: &mut JsonValue,
    packet: &JsonValue,
    owner_local_transport: bool,
) -> SourceCommandResult<JsonValue> {
    let rights = cmd::field(packet, "rights_and_visibility")?;
    let effective_visibility = cmd::text(rights, "effective_visibility")?;
    let declared_public = cmd::field(summary, "public_content_declared")? == &JsonValue::Bool(true);
    let public_content_declared = declared_public && !owner_local_transport;
    let reported_visibility = if owner_local_transport
        && matches!(
            effective_visibility,
            "public" | "public_metadata_only" | "controlled"
        ) {
        "local_only"
    } else {
        effective_visibility
    };
    cmd::set(
        summary,
        "effective_visibility",
        cmd::string(reported_visibility),
    )?;
    cmd::set(
        summary,
        "public_content_declared",
        JsonValue::Bool(public_content_declared),
    )?;
    let verified = cmd::field(summary, "content_verified")? == &JsonValue::Bool(true);
    cmd::set(
        summary,
        "public_content_available",
        JsonValue::Bool(verified && public_content_declared),
    )?;
    cmd::set(
        summary,
        "owner_local_transport",
        JsonValue::Bool(owner_local_transport),
    )?;
    Ok(summary.clone())
}

/// Borrow the selected owner transport immutably for the fixed OwnerText
/// metadata route. The mutable trait surface is historical; this adapter
/// issues no writes and every read remains selected and bounded.
struct ProfileNativeReader<'a>(&'a OwnerTextContext);

impl SignNativeRead for ProfileNativeReader<'_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: crate::source_sign_native::NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.0.read(reference, max_bytes, deadline, cancelled)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.0.snapshot(deadline, cancelled).map(|_| ())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        SignNativeRead::owner_local(self.0, reference)
    }
}

fn profile_snapshot(
    owner: &OwnerTextContext,
    grant: &Grant,
    selected: &SelectedProfileRecord,
    context_config: &JsonValue,
    native_snapshot: Option<&str>,
    native_metadata_snapshot: Option<&str>,
    native_contracts: &BTreeMap<String, String>,
    worker: &CutWorkerSchemaExecutor,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let context = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let mut contracts = BTreeMap::new();
    for dependency in &selected.dependencies {
        contracts.insert(
            dependency.path.as_str().to_owned(),
            dependency.raw_sha256.to_hex(),
        );
    }
    for reference in [PROFILE_REGISTRY, PROFILE_CONTRACT, CONTEXT_CONTRACT] {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("profile snapshot contract path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "profile snapshot contract absent",
        ))?;
        // The selected registry is authored data; only schemas belong to the worker.
        if reference != PROFILE_REGISTRY
            && worker.contract_digest(reference) != Some(Digest256::of_bytes(raw))
        {
            return Err(SourceCommandError::Conflict(
                "profile snapshot contract changed",
            ));
        }
        contracts.insert(reference.to_owned(), Digest256::of_bytes(raw).to_hex());
    }
    for (reference, expected) in native_contracts {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("private native contract path"))?;
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "selected private native contract absent",
        ))?;
        if Digest256::of_bytes(raw).to_hex() != *expected
            || worker.contract_digest(reference) != Some(Digest256::of_bytes(raw))
        {
            return Err(SourceCommandError::Conflict(
                "private native schema changed",
            ));
        }
        contracts.insert(reference.clone(), expected.clone());
    }
    let binding_digest = grant
        .source_binding
        .as_ref()
        .map(|value| cmd::canonical(value).map(|raw| Digest256::of_bytes(&raw).to_hex()))
        .transpose()?;
    let native = cmd::object(vec![
        (
            "metadata",
            native_metadata_snapshot
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        (
            "exact",
            native_snapshot.map(cmd::string).unwrap_or(JsonValue::Null),
        ),
    ]);
    let snapshot_basis = cmd::object(vec![
        ("context", cmd::string(&context)),
        ("source_access", grant.source_access.clone()),
        (
            "source_binding",
            binding_digest
                .map(|d| cmd::string(&d))
                .unwrap_or(JsonValue::Null),
        ),
        ("contracts", hash_map(contracts)),
        ("sources", hash_map(BTreeMap::new())),
        ("native", native.clone()),
    ]);
    let _ = (
        context_config,
        selected.source_digest.as_str(),
        selected.source_path.as_str(),
    );
    Ok(cmd::record_digest(&snapshot_basis)?.to_prefixed())
}

fn dependencies(
    grant: &Grant,
    profile_digest: &str,
    rights_snapshot: Option<&str>,
    forms: &JsonValue,
    identity: &str,
    implementation: &JsonValue,
) -> SourceCommandResult<String> {
    let basis = cmd::object(vec![
        ("profiles", cmd::string(profile_digest)),
        (
            "rights",
            rights_snapshot.map(cmd::string).unwrap_or(JsonValue::Null),
        ),
        ("form_grammar", forms.clone()),
        ("identity", cmd::string(identity)),
        ("implementation", implementation.clone()),
    ]);
    let _ = grant;
    Ok(cmd::record_digest(&basis)?.to_prefixed())
}

fn package_file_refs(files: &PrivatePackage) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    JsonString::from_utf8(name),
                    cmd::object(vec![
                        (
                            "sha256",
                            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                        ),
                        ("bytes", cmd::number(raw.len() as u64)),
                    ]),
                )
            })
            .collect(),
    )
}

fn request_digest(request: &JsonValue) -> SourceCommandResult<String> {
    Ok(Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed())
}

fn request_shape(request: &JsonValue) -> SourceCommandResult<String> {
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid(
            "private profile request schema",
        ));
    }
    let operation = cmd::text(request, "operation")?;
    let mut keys = vec!["schema_version", "operation"];
    match operation {
        "describe" => (),
        "prepare-create" => keys.extend(["record", "forms"]),
        "source.create" => keys.extend([
            "record",
            "forms",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]),
        "prepare-revise" => keys.extend(["fields", "forms", "reason"]),
        "record.revise" => keys.extend([
            "fields",
            "forms",
            "reason",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]),
        "prepare" => keys.extend(["form_id", "field_id"]),
        "apply" => keys.extend([
            "changes",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]),
        "inspect-version" => keys.push("source"),
        _ => return Err(SourceCommandError::Unsupported("private profile operation")),
    }
    cmd::exact_keys(request, &keys)?;
    Ok(operation.to_owned())
}

fn check_command_id(request: &JsonValue) -> SourceCommandResult<&str> {
    let command_id = cmd::text(request, "command_id")?;
    if !(1..=256).contains(&command_id.chars().count()) {
        return Err(SourceCommandError::Invalid(
            "source command identity length",
        ));
    }
    Ok(command_id)
}

fn stripped_char_count(value: &str) -> SourceCommandResult<usize> {
    let stripped = tos_foundation::python_strip_unicode16_v1(value, value.len())
        .map_err(|_| SourceCommandError::Invalid("private profile bounded text"))?;
    Ok(stripped.chars().count())
}

fn request_commit_matches(
    request: &JsonValue,
    grant: &Grant,
    source: Option<&JsonValue>,
    revision: Option<&str>,
    deps: &str,
) -> SourceCommandResult<()> {
    check_command_id(request)?;
    if cmd::text(request, "expected_configuration")? != grant.digest
        || !cmd::same(
            cmd::field(request, "expected_source")?,
            &source.cloned().unwrap_or(JsonValue::Null),
        )?
        || !cmd::same(
            cmd::field(request, "expected_revision")?,
            &revision.map(cmd::string).unwrap_or(JsonValue::Null),
        )?
        || cmd::text(request, "expected_dependencies")? != deps
    {
        return Err(SourceCommandError::Conflict(
            "private profile source, configuration or dependencies are stale",
        ));
    }
    if !digest_text(cmd::text(request, "expected_dependencies")?) {
        return Err(SourceCommandError::Invalid(
            "private profile expected dependency digest",
        ));
    }
    Ok(())
}

fn exact_form_path(grant: &Grant) -> SourceCommandResult<String> {
    Ok(format!(
        "{}.human-forms.json",
        grant.source_path.as_str().trim_end_matches(".json")
    ))
}

fn reservation_ids(grant: &Grant, creating: bool) -> SourceCommandResult<BTreeSet<String>> {
    let mut ids = BTreeSet::from([cmd::text(&grant.value, "record_id")?.to_owned()]);
    ids.extend(grant.form_ids.iter().cloned());
    if creating {
        ids.insert(cmd::text(&grant.value, "provenance_event_id")?.to_owned());
    }
    Ok(ids)
}

fn make_snapshot(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &Grant,
    record: &JsonValue,
    inventory: &PrivateIdentityInputs,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    creating: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Snapshot> {
    let raw = cmd::published(record)?;
    let selected = validate_profile_record(
        ctx,
        cut,
        &grant.profile_type_id,
        grant.source_path.as_str(),
        cmd::text(&grant.value, "record_id")?,
        &raw,
        worker,
        deadline,
        cancelled,
    )?;
    let (rights, native_metadata, native_summary, native_reads, native_contracts) =
        validate_native_profile(
            ctx,
            owner,
            grant,
            &selected.record,
            worker,
            context_config,
            deadline,
            cancelled,
        )?;
    let profile_digest = profile_snapshot(
        owner,
        grant,
        &selected,
        context_config,
        rights.as_deref(),
        native_metadata.as_deref(),
        &native_contracts,
        worker,
        ctx,
        deadline,
        cancelled,
    )?;
    let form_digests = form_grammar_digests(ctx, worker, deadline, cancelled)?;
    let basenames = identity_basenames(ctx, cut, worker, deadline, cancelled)?;
    let identity_digest = identity_snapshot(
        ctx,
        inventory,
        &basenames,
        &reservation_ids(grant, creating)?,
        creating,
    )?;
    let implementation_digests = implementation_digests(software, components, deadline, cancelled)?;
    let dependencies = dependencies(
        grant,
        &profile_digest,
        rights.as_deref(),
        &form_digests,
        &identity_digest,
        &implementation_digests,
    )?;
    // Cold archive reads are deliberately streamed by the lifecycle verifier.
    let _ = archive_reader;
    Ok(Snapshot {
        profile_snapshot: profile_digest,
        rights_snapshot: rights,
        native_summary,
        native_reads,
        form_digests,
        identity_digest,
        implementation_digests,
        dependencies,
    })
}

fn revise_record(record: &JsonValue, fields: &JsonValue) -> SourceCommandResult<JsonValue> {
    let updates = fields.as_object().ok_or(SourceCommandError::Invalid(
        "private profile revision fields",
    ))?;
    let next_version = cmd::integer(record, "record_version")?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid(
            "private profile record version overflow",
        ))?;
    let mut revised = record.clone();
    for (key, value) in updates {
        let key = key.as_str().ok_or(SourceCommandError::Invalid(
            "private profile revision field key",
        ))?;
        if !REVISION_FIELDS.contains(&key) {
            return Err(SourceCommandError::Denied(
                "private profile field outside grant",
            ));
        }
        cmd::set(&mut revised, key, value.clone())?;
    }
    cmd::set(&mut revised, "record_version", cmd::number(next_version))?;
    Ok(revised)
}

fn validate_revision_scope(request: &JsonValue, grant: &Grant) -> SourceCommandResult<()> {
    if !grant
        .operations
        .iter()
        .any(|operation| operation == "record.revise")
    {
        return Err(SourceCommandError::Denied(
            "private record revision is not delegated",
        ));
    }
    let fields = cmd::field(request, "fields")?;
    let members = fields.as_object().ok_or(SourceCommandError::Invalid(
        "private profile revision fields object",
    ))?;
    if members.is_empty() || members.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private profile revision field count",
        ));
    }
    for (key, _) in members {
        let key = key.as_str().ok_or(SourceCommandError::Invalid(
            "private profile revision field name",
        ))?;
        if !grant.fields.iter().any(|allowed| allowed == key) || !REVISION_FIELDS.contains(&key) {
            return Err(SourceCommandError::Denied(
                "private profile field outside grant",
            ));
        }
    }
    let reason_count = stripped_char_count(cmd::text(request, "reason")?)?;
    if !(1..=4096).contains(&reason_count) {
        return Err(SourceCommandError::Invalid(
            "private profile revision reason",
        ));
    }
    Ok(())
}

fn check_form_changes(changes: &[JsonValue], grant: &Grant) -> SourceCommandResult<Vec<JsonValue>> {
    if !(1..=32).contains(&changes.len()) {
        return Err(SourceCommandError::Invalid("private form change count"));
    }
    let mut ids = BTreeSet::new();
    for change in changes {
        cmd::exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = cmd::field(change, "form")?;
        let id = cmd::text(form, "form_id")?;
        let operation = cmd::text(change, "operation")?;
        if !grant.form_ids.iter().any(|allowed| allowed == id)
            || !ids.insert(id.to_owned())
            || !grant.operations.iter().any(|allowed| allowed == operation)
            || !matches!(operation, "form.create" | "form.revise")
            || cmd::text(form, "creator_id")? != cmd::text(&grant.value, "principal_id")?
        {
            return Err(SourceCommandError::Denied(
                "private form change is outside its exact grant",
            ));
        }
    }
    Ok(changes.to_vec())
}

fn validate_form_path_set(
    ctx: &CommandContext,
    grant: &Grant,
    forms: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    validate_form_set(
        ctx,
        worker,
        &exact_form_path(grant)?,
        forms,
        deadline,
        cancelled,
    )
}

fn prepared_create(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    grant: &Grant,
    request: &JsonValue,
    inventory: &PrivateIdentityInputs,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    JsonValue,
    PrivatePackage,
    String,
    Vec<JsonValue>,
    Vec<SourceFile>,
)> {
    if grant.profile.object_get("creation_gate").is_some() {
        return Err(SourceCommandError::Denied(
            "this profile requires its explicit promotion adapter",
        ));
    }
    let record = cmd::field(request, "record")?.clone();
    if cmd::text(&record, "record_id")? != cmd::text(&grant.value, "record_id")?
        || cmd::integer(&record, "record_version")? != 1
        || cmd::text(&record, "identity_status")? != "provisional"
        || cmd::text(&record, "same_as_posture")? != "no_equivalence_claim"
        || record
            .object_get("supersedes_ref")
            .is_some_and(|value| !value.is_null())
    {
        return Err(SourceCommandError::Denied(
            "source creation needs the exact provisional unlinked record posture",
        ));
    }
    let snapshot = make_snapshot(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        grant,
        &record,
        inventory,
        archive_reader,
        worker,
        true,
        deadline,
        cancelled,
    )?;
    let (forms, _, form_refs) = form_selections(request, grant, &record, None, false)?;
    validate_form_path_set(ctx, grant, &forms, worker, deadline, cancelled)?;
    let views = validate_ready_forms(&record, &forms)?;
    let source_raw = cmd::published(&record)?;
    let selected = validate_profile_record(
        ctx,
        cut,
        &grant.profile_type_id,
        grant.source_path.as_str(),
        cmd::text(&grant.value, "record_id")?,
        &source_raw,
        worker,
        deadline,
        cancelled,
    )?;
    let mut files = PrivatePackage::new();
    files.insert(
        grant
            .source_path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid("private source basename"))?
            .to_owned(),
        source_raw,
    );
    files.insert(formset_name(grant)?, cmd::published(&forms)?);
    files.insert(CONFIG_FILE.into(), cmd::published(&grant.value)?);
    let _ = form_refs;
    Ok((
        selected.record,
        files,
        snapshot.dependencies,
        views,
        snapshot.native_reads,
    ))
}

fn profile_result(
    grant: &Grant,
    files: Option<&PrivatePackage>,
    state: Option<&State>,
    receipt: Option<&JsonValue>,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let mut result = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_command_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&grant.digest)),
        ("source_path", cmd::string(grant.source_path.as_str())),
        ("record_id", cmd::field(&grant.value, "record_id")?.clone()),
        ("profile_type_id", cmd::string(&grant.profile_type_id)),
        ("target_exists", JsonValue::Bool(files.is_some())),
        (
            "supported_operations",
            JsonValue::Array(OPERATIONS.iter().map(|value| cmd::string(value)).collect()),
        ),
        (
            "allowed_operations",
            cmd::field(&grant.value, "allowed_operations")?.clone(),
        ),
        (
            "command_operations",
            JsonValue::Array(
                [
                    "describe",
                    "prepare-create",
                    "source.create",
                    "prepare-revise",
                    "record.revise",
                    "prepare",
                    "apply",
                    "inspect-version",
                ]
                .iter()
                .map(|value| cmd::string(value))
                .collect(),
            ),
        ),
        (
            "allowed_form_ids",
            cmd::field(&grant.value, "allowed_form_ids")?.clone(),
        ),
        (
            "allowed_fields",
            cmd::field(&grant.value, "allowed_fields")?.clone(),
        ),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
        ("receipt", receipt.cloned().unwrap_or(JsonValue::Null)),
        ("replayed", JsonValue::Bool(replayed)),
        (
            "replay_input_posture",
            if replayed {
                cmd::string("historical_request_current_validation")
            } else {
                JsonValue::Null
            },
        ),
        ("source", JsonValue::Null),
        ("revision", JsonValue::Null),
        ("materializations", JsonValue::Array(Vec::new())),
    ]);
    if let Some(state) = state {
        let materializations = source_forms::materialize_source_forms(&state.record, &state.forms)?;
        let source_fields = source_forms::metadata_fields(&state.record)?
            .iter()
            .map(source_forms::FormField::public)
            .collect::<Vec<_>>();
        cmd::set(&mut result, "source", state.subject.clone())?;
        cmd::set(
            &mut result,
            "revision",
            cmd::string(&package_revision(&state.files)?),
        )?;
        cmd::set(
            &mut result,
            "source_fields",
            JsonValue::Array(source_fields),
        )?;
        cmd::set(
            &mut result,
            "forms",
            JsonValue::Array(form_refs(&state.forms)?),
        )?;
        cmd::set(
            &mut result,
            "materializations",
            JsonValue::Array(materializations),
        )?;
    }
    Ok(result)
}

fn add_expected_dependencies(
    result: &mut JsonValue,
    dependencies: &str,
) -> SourceCommandResult<()> {
    cmd::set(result, "expected_dependencies", cmd::string(dependencies))
}

fn verify_creation_file_refs(
    grant: &Grant,
    receipt: &JsonValue,
    files: &PrivatePackage,
) -> SourceCommandResult<()> {
    let source_name =
        grant
            .source_path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid(
                "private profile source basename",
            ))?;
    let expected = BTreeSet::from([
        source_name.to_owned(),
        formset_name(grant)?,
        CONFIG_FILE.to_owned(),
        CREATE_REQUEST.to_owned(),
        CREATE_ENVIRONMENT.to_owned(),
        CREATE_PROVENANCE.to_owned(),
    ]);
    let rows = cmd::field(receipt, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "private creation file reference map",
        ))?;
    let names = rows
        .iter()
        .map(|(name, _)| {
            name.as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid("private creation file name"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if names != expected {
        return Err(SourceCommandError::Conflict(
            "private creation receipt file scope differs",
        ));
    }
    for (name, binding) in rows {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("private creation file name"))?;
        let raw = files
            .get(name)
            .ok_or(SourceCommandError::Invalid("private creation file absent"))?;
        if cmd::text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "private creation receipt file fixity differs",
            ));
        }
    }
    Ok(())
}

fn revision_fields_sorted(fields: &JsonValue) -> SourceCommandResult<JsonValue> {
    let members = fields
        .as_object()
        .ok_or(SourceCommandError::Invalid("private revision fields"))?;
    let mut names = members
        .iter()
        .map(|(name, _)| {
            name.as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid("private revision field name"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    names.sort();
    Ok(JsonValue::Array(
        names.iter().map(|name| cmd::string(name)).collect(),
    ))
}

fn history_with_receipts(
    history: &JsonValue,
    receipts: Vec<JsonValue>,
) -> SourceCommandResult<JsonValue> {
    let mut next = history.clone();
    cmd::set(&mut next, "receipts", JsonValue::Array(receipts))?;
    Ok(next)
}

fn validate_retained_request(receipt: &JsonValue, request: &JsonValue) -> SourceCommandResult<()> {
    let _ = request_shape(request)?;
    if request_digest(request)? != cmd::text(receipt, "request_digest")? {
        return Err(SourceCommandError::Conflict(
            "private retained request digest differs",
        ));
    }
    Ok(())
}

fn validate_form_growth_history(
    forms: &JsonValue,
    retained_forms: &[JsonValue],
) -> SourceCommandResult<Vec<JsonValue>> {
    let receipts = match forms.object_get("growth_history") {
        Some(value) => value
            .as_array()
            .ok_or(SourceCommandError::Invalid("private form growth history"))?,
        None => &[],
    };
    if receipts.len() > 4096 {
        return Err(SourceCommandError::Invalid("private form receipt budget"));
    }
    let mut commands = BTreeSet::new();
    for receipt in receipts {
        cmd::exact_keys(
            receipt,
            &[
                "command_id",
                "request_digest",
                "principal_id",
                "authority_ref",
                "owner_configuration",
                "recorded_at",
                "source",
                "previous_revision",
                "results",
            ],
        )?;
        let command_id = cmd::text(receipt, "command_id")?;
        if !(1..=256).contains(&command_id.chars().count())
            || !commands.insert(command_id.to_owned())
            || !digest_text(cmd::text(receipt, "request_digest")?)
            || !digest_text(cmd::text(receipt, "owner_configuration")?)
            || !digest_text(cmd::text(receipt, "previous_revision")?)
            || !cmd::nonblank(cmd::text(receipt, "principal_id")?)
            || !cmd::nonblank(cmd::text(receipt, "authority_ref")?)
        {
            return Err(SourceCommandError::Invalid("private form receipt identity"));
        }
        cmd::validate_instant(cmd::text(receipt, "recorded_at")?)?;
        let source = cmd::field(receipt, "source")?;
        if cmd::text(source, "id")? != cmd::text(cmd::field(forms, "subject")?, "id")? {
            return Err(SourceCommandError::Conflict(
                "private form receipt source identity",
            ));
        }
        let refs = cmd::array(receipt, "results")?;
        if refs.is_empty() || refs.len() > 32 {
            return Err(SourceCommandError::Invalid(
                "private form receipt result budget",
            ));
        }
        for reference in refs {
            if !retained_forms.iter().any(|form| {
                source_forms::form_reference(form)
                    .is_ok_and(|r| cmd::same(&r, reference).unwrap_or(false))
            }) {
                return Err(SourceCommandError::Conflict(
                    "private form receipt output is absent from retained history",
                ));
            }
        }
    }
    Ok(receipts.to_vec())
}

fn validate_revision_request_receipt(
    grant: &Grant,
    receipt: &JsonValue,
    old_state: &State,
    current_state: &State,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let request = cmd::field(receipt, "request")?;
    validate_retained_request(receipt, request)?;
    if request_shape(request)? != "record.revise"
        || cmd::text(request, "command_id")? != cmd::text(receipt, "command_id")?
        || cmd::text(request, "expected_configuration")?
            != cmd::text(receipt, "owner_configuration")?
        || !cmd::same(
            cmd::field(request, "expected_source")?,
            cmd::field(receipt, "previous_source")?,
        )?
        || cmd::text(request, "expected_revision")? != cmd::text(receipt, "previous_revision")?
        || cmd::text(request, "expected_dependencies")? != cmd::text(receipt, "dependencies")?
        || cmd::text(request, "reason")? != cmd::text(receipt, "reason")?
        || !cmd::same(
            &revision_fields_sorted(cmd::field(request, "fields")?)?,
            cmd::field(receipt, "changed_fields")?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "private revision receipt differs from its exact request",
        ));
    }
    let revised = revise_record(&old_state.record, cmd::field(request, "fields")?)?;
    let revised_raw = cmd::published(&revised)?;
    let revised = validate_profile_record(
        ctx,
        cut,
        &grant.profile_type_id,
        grant.source_path.as_str(),
        cmd::text(&grant.value, "record_id")?,
        &revised_raw,
        worker,
        deadline,
        cancelled,
    )?
    .record;
    let subject = source_forms::metadata_subject(&revised)?;
    if !cmd::same(&subject, cmd::field(receipt, "source")?)? {
        return Err(SourceCommandError::Conflict(
            "private revision request does not produce its exact successor",
        ));
    }
    let (_, _, refs) = form_selections(request, grant, &revised, Some(&old_state.forms), true)?;
    if !cmd::same(&JsonValue::Array(refs), cmd::field(receipt, "forms")?)? {
        return Err(SourceCommandError::Conflict(
            "private revision request does not produce its retained form references",
        ));
    }
    let current_refs = cmd::array(&current_state.forms, "forms")?
        .iter()
        .chain(cmd::array(&current_state.forms, "prior_forms")?.iter())
        .map(source_forms::form_reference)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    for reference in cmd::array(receipt, "forms")? {
        if !current_refs
            .iter()
            .any(|retained| cmd::same(retained, reference).unwrap_or(false))
        {
            return Err(SourceCommandError::Conflict(
                "private revision form output is absent from retained current lineage",
            ));
        }
    }
    Ok(())
}

fn verify_cold_history(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    context_config: &JsonValue,
    grant: &Grant,
    current: &PrivatePackage,
    state: &State,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let references = required_archives(context_config, current)?;
    let receipts = cmd::array(&state.history, "receipts")?;
    if references.len() != receipts.len() {
        return Err(SourceCommandError::Invalid(
            "private archive/history cardinality",
        ));
    }
    let current_creation = current
        .get(RECEIPT_FILE)
        .ok_or(SourceCommandError::Invalid(
            "private creation receipt absent",
        ))?;
    for (index, (reference, receipt)) in references.iter().zip(receipts).enumerate() {
        active(deadline, cancelled)?;
        let raw = archive_reader.read_archive(reference, deadline, cancelled)?;
        let (files, _) = decode_archive(reference, raw, receipt, grant)?;
        let old = inspect_state(ctx, cut, grant, &files, worker, deadline, cancelled)?;
        if files.get(RECEIPT_FILE) != Some(current_creation) {
            return Err(SourceCommandError::Conflict(
                "private creation receipt changed across archived history",
            ));
        }
        let prefix = cmd::array(&old.history, "receipts")?;
        if prefix.len() != index {
            return Err(SourceCommandError::Conflict(
                "private archived history is not a continuous predecessor prefix",
            ));
        }
        for (actual, expected) in prefix.iter().zip(&receipts[..index]) {
            if !cmd::same(actual, expected)? {
                return Err(SourceCommandError::Conflict(
                    "private archived history differs from its retained current prefix",
                ));
            }
        }
        if cmd::field(receipt, "previous_revision")? != &cmd::string(&package_revision(&files)?)
            || !cmd::same(cmd::field(receipt, "previous_source")?, &old.subject)?
        {
            return Err(SourceCommandError::Conflict(
                "private archive does not match its exact predecessor receipt",
            ));
        }
        if index == 0 {
            let creation = cmd::parse(current_creation)?;
            verify_creation_file_refs(grant, &creation, &files)?;
        }
        validate_revision_request_receipt(
            grant, receipt, &old, state, ctx, cut, worker, deadline, cancelled,
        )?;
    }
    let retained_forms = cmd::array(&state.forms, "forms")?
        .iter()
        .chain(cmd::array(&state.forms, "prior_forms")?.iter())
        .cloned()
        .collect::<Vec<_>>();
    let _ = validate_form_growth_history(&state.forms, &retained_forms)?;
    Ok(())
}

fn revision_proposal(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    grant: &Grant,
    state: &State,
    request: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, PrivatePackage, Vec<JsonValue>, Vec<JsonValue>)> {
    validate_revision_scope(request, grant)?;
    if cmd::array(&state.history, "receipts")?.len() >= MAX_REVISIONS {
        return Err(SourceCommandError::Invalid(
            "private source revision history capacity reached",
        ));
    }
    let proposed = revise_record(&state.record, cmd::field(request, "fields")?)?;
    let proposed_raw = cmd::published(&proposed)?;
    let proposed = validate_profile_record(
        ctx,
        cut,
        &grant.profile_type_id,
        grant.source_path.as_str(),
        cmd::text(&grant.value, "record_id")?,
        &proposed_raw,
        worker,
        deadline,
        cancelled,
    )?
    .record;
    let (forms, _, refs) = form_selections(request, grant, &proposed, Some(&state.forms), true)?;
    validate_form_path_set(ctx, grant, &forms, worker, deadline, cancelled)?;
    let views = validate_ready_forms(&proposed, &forms)?;
    let mut output = state.files.clone();
    let basename =
        grant
            .source_path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid(
                "private profile source basename",
            ))?;
    output.insert(basename.to_owned(), cmd::published(&proposed)?);
    output.insert(formset_name(grant)?, cmd::published(&forms)?);
    Ok((proposed, output, refs, views))
}

fn merge_reads(
    groups: impl IntoIterator<Item = Vec<SourceFile>>,
) -> SourceCommandResult<Vec<SourceFile>> {
    let mut merged = BTreeMap::new();
    for group in groups {
        for read in group {
            let key = read.path.as_str().to_owned();
            if let Some(previous) = merged.insert(key, read.raw.clone())
                && previous != read.raw
            {
                return Err(SourceCommandError::Conflict(
                    "private profile binding input changed between semantic checks",
                ));
            }
        }
    }
    merged
        .into_iter()
        .map(|(reference, raw)| {
            Ok(SourceFile {
                path: RelativePath::parse(&reference)
                    .map_err(|_| SourceCommandError::Invalid("private profile read path"))?,
                raw,
            })
        })
        .collect()
}

fn create_replay(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &Grant,
    request: &JsonValue,
    current: &PrivatePackage,
    current_state: &State,
    inventory: &PrivateIdentityInputs,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Vec<SourceFile>)> {
    let receipt = cmd::parse(
        current
            .get(RECEIPT_FILE)
            .ok_or(SourceCommandError::Invalid(
                "private creation receipt absent",
            ))?,
    )?;
    validate_creation_receipt(&receipt, &current_state.record)?;
    let digest = request_digest(request)?;
    if cmd::text(&receipt, "command_id")? != check_command_id(request)?
        || cmd::text(&receipt, "request_digest")? != digest
        || cmd::text(&receipt, "source_path")? != grant.source_path.as_str()
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || cmd::text(&receipt, "owner_configuration")? != grant.digest
        || cmd::text(&request, "expected_configuration")? != grant.digest
        || !cmd::field(request, "expected_source")?.is_null()
        || !cmd::field(request, "expected_revision")?.is_null()
        || cmd::text(&receipt, "principal_id")? != cmd::text(&grant.value, "principal_id")?
        || cmd::text(&receipt, "authority_ref")? != cmd::text(&grant.value, "authority_ref")?
    {
        return Err(SourceCommandError::Conflict(
            "owner-local source creation identity is already occupied",
        ));
    }
    let (initial_record, initial_files, _initial_dependencies, _, create_reads) = prepared_create(
        ctx,
        cut,
        grant,
        request,
        inventory,
        software,
        components,
        owner,
        context_config,
        archive_reader,
        worker,
        deadline,
        cancelled,
    )?;
    if cmd::text(&receipt, "dependencies")? != cmd::text(request, "expected_dependencies")?
        || !cmd::same(
            cmd::field(&receipt, "source")?,
            &source_forms::metadata_subject(&initial_record)?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "owner-local creation receipt differs from its original source or dependencies",
        ));
    }
    let create_raw = current
        .get(CREATE_REQUEST)
        .ok_or(SourceCommandError::Invalid(
            "retained private creation request absent",
        ))?;
    if cmd::parse(create_raw)? != *request {
        return Err(SourceCommandError::Conflict(
            "retained private creation request differs",
        ));
    }
    let mut initial_package = initial_files;
    initial_package.insert(CREATE_REQUEST.into(), create_raw.clone());
    initial_package.insert(
        CREATE_ENVIRONMENT.into(),
        current
            .get(CREATE_ENVIRONMENT)
            .ok_or(SourceCommandError::Invalid(
                "private creation environment absent",
            ))?
            .clone(),
    );
    initial_package.insert(
        CREATE_PROVENANCE.into(),
        current
            .get(CREATE_PROVENANCE)
            .ok_or(SourceCommandError::Invalid(
                "private creation provenance absent",
            ))?
            .clone(),
    );
    verify_creation_file_refs(grant, &receipt, &initial_package)?;
    if let Some(first_receipt) = cmd::array(&current_state.history, "receipts")?.first() {
        let reference = cmd::text(first_receipt, "archive_path")?;
        let archived_raw = archive_reader.read_archive(reference, deadline, cancelled)?;
        let (archived_files, _) = decode_archive(reference, archived_raw, first_receipt, grant)?;
        let source_name =
            grant
                .source_path
                .as_str()
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid(
                    "private profile source basename",
                ))?;
        let archived_source =
            archived_files
                .get(source_name)
                .ok_or(SourceCommandError::Invalid(
                    "private profile archive source absent",
                ))?;
        let initial_source =
            initial_package
                .get(source_name)
                .ok_or(SourceCommandError::Invalid(
                    "private profile initial source absent",
                ))?;
        if archived_source != initial_source {
            return Err(SourceCommandError::Conflict(
                "owner-local initial source bytes differ from the retained creation request",
            ));
        }
    } else {
        let source_name =
            grant
                .source_path
                .as_str()
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid(
                    "private profile source basename",
                ))?;
        if current.get(source_name) != initial_package.get(source_name) {
            return Err(SourceCommandError::Conflict(
                "owner-local initial source bytes differ from the retained creation request",
            ));
        }
    }
    let retained = cmd::array(&current_state.forms, "forms")?
        .iter()
        .chain(cmd::array(&current_state.forms, "prior_forms")?.iter())
        .collect::<Vec<_>>();
    for form in cmd::array(
        &cmd::parse(&initial_package[&formset_name(grant)?])?,
        "forms",
    )? {
        let reference = source_forms::form_reference(form)?;
        if !retained
            .iter()
            .any(|candidate| cmd::same(candidate, form).unwrap_or(false))
            && !retained.iter().any(|candidate| {
                source_forms::form_reference(candidate).is_ok_and(|candidate_ref| {
                    cmd::same(&candidate_ref, &reference).unwrap_or(false)
                })
            })
        {
            return Err(SourceCommandError::Conflict(
                "owner-local initial form is absent from retained form lineage",
            ));
        }
    }
    let current_snapshot = make_snapshot(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        grant,
        &current_state.record,
        inventory,
        archive_reader,
        worker,
        false,
        deadline,
        cancelled,
    )?;
    let result = profile_result(
        grant,
        Some(current),
        Some(current_state),
        Some(&receipt),
        true,
    )?;
    merge_reads([create_reads, current_snapshot.native_reads]).map(|reads| (result, reads))
}

fn create_package(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &Grant,
    request: &JsonValue,
    inventory: &PrivateIdentityInputs,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(PrivatePackage, JsonValue, Vec<SourceFile>)> {
    let (record, mut files, dependencies, views, reads) = prepared_create(
        ctx,
        cut,
        grant,
        request,
        inventory,
        software,
        components,
        owner,
        context_config,
        archive_reader,
        worker,
        deadline,
        cancelled,
    )?;
    request_commit_matches(request, grant, None, None, &dependencies)?;
    source_serialization::capture_private_metadata(
        PrivateMetadataFamily::Profile,
        request,
        cmd::text(&grant.value, "provenance_event_id")?,
        &grant.home,
        &mut files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let subject = source_forms::metadata_subject(&record)?;
    let receipt = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_create_receipt_v1"),
        ),
        ("command_id", cmd::string(check_command_id(request)?)),
        ("request_digest", cmd::string(&request_digest(request)?)),
        (
            "principal_id",
            cmd::field(&grant.value, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&grant.value, "authority_ref")?.clone(),
        ),
        ("owner_configuration", cmd::string(&grant.digest)),
        (
            "recorded_at",
            cmd::string(&source_serialization::instant()?),
        ),
        ("source_path", cmd::string(grant.source_path.as_str())),
        ("source", subject.clone()),
        ("dependencies", cmd::string(&dependencies)),
        ("files", package_file_refs(&files)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    files.insert(RECEIPT_FILE.into(), encode(&receipt)?);
    verify_creation_file_refs(grant, &receipt, &files)?;
    let state = inspect_state(ctx, cut, grant, &files, worker, deadline, cancelled)?;
    if state.subject != subject
        || !views
            .iter()
            .all(|view| cmd::text(view, "state").ok() == Some("ready"))
    {
        return Err(SourceCommandError::Conflict(
            "private source creation output differs",
        ));
    }
    let result = profile_result(grant, Some(&files), Some(&state), Some(&receipt), false)?;
    Ok((files, result, reads))
}

fn current_snapshot(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &Grant,
    state: &State,
    inventory: &PrivateIdentityInputs,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Snapshot> {
    make_snapshot(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        grant,
        &state.record,
        inventory,
        archive_reader,
        worker,
        false,
        deadline,
        cancelled,
    )
}

fn retained_command(
    state: &State,
    request: &JsonValue,
    grant: &Grant,
    _current_dependencies: &str,
) -> SourceCommandResult<Option<JsonValue>> {
    let command_id = check_command_id(request)?;
    let creation = cmd::parse(state.files.get(RECEIPT_FILE).ok_or(
        SourceCommandError::Invalid("private source creation receipt absent"),
    )?)?;
    if cmd::text(&creation, "command_id")? == command_id {
        return Err(SourceCommandError::Conflict(
            "private command identity was used by source creation",
        ));
    }
    let forms_receipts = match state.forms.object_get("growth_history") {
        Some(value) => value
            .as_array()
            .ok_or(SourceCommandError::Invalid("private form receipt history"))?,
        None => &[],
    };
    let operation = cmd::text(request, "operation")?;
    let receipts = if operation == "record.revise" {
        &forms_receipts[..]
    } else {
        cmd::array(&state.history, "receipts")?
    };
    if receipts
        .iter()
        .any(|receipt| cmd::text(receipt, "command_id").ok() == Some(command_id))
    {
        return Err(SourceCommandError::Conflict(
            "private command identity belongs to another operation",
        ));
    }
    let target_receipts = if operation == "record.revise" {
        cmd::array(&state.history, "receipts")?
    } else {
        &forms_receipts[..]
    };
    let digest = request_digest(request)?;
    if let Some(receipt) = target_receipts.iter().find(|receipt| {
        receipt.object_get("command_id").and_then(JsonValue::as_str) == Some(command_id)
    }) {
        if cmd::text(receipt, "request_digest")? != digest {
            return Err(SourceCommandError::Conflict(
                "private command identity reused",
            ));
        }
        if cmd::text(receipt, "owner_configuration")? != grant.digest
            || cmd::text(request, "expected_configuration")? != grant.digest
            || cmd::text(receipt, "principal_id")? != cmd::text(&grant.value, "principal_id")?
            || cmd::text(receipt, "authority_ref")? != cmd::text(&grant.value, "authority_ref")?
        {
            return Err(SourceCommandError::Conflict(
                "private operation retry has stale delegation",
            ));
        }
        if operation == "record.revise" {
            if cmd::text(receipt, "dependencies")? != cmd::text(request, "expected_dependencies")? {
                return Err(SourceCommandError::Conflict(
                    "private revision retry differs from its original dependencies",
                ));
            }
        } else {
            let changes = check_form_changes(cmd::array(request, "changes")?, grant)?;
            let refs = changes
                .iter()
                .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
                .collect::<SourceCommandResult<Vec<_>>>()?;
            if !cmd::same(cmd::field(receipt, "results")?, &JsonValue::Array(refs))?
                || !cmd::same(
                    cmd::field(receipt, "source")?,
                    cmd::field(request, "expected_source")?,
                )?
                || !cmd::same(
                    cmd::field(receipt, "previous_revision")?,
                    cmd::field(request, "expected_revision")?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "private form retry differs from its original results",
                ));
            }
        }
        return Ok(Some(receipt.clone()));
    }
    Ok(None)
}

fn apply_form_successor(
    ctx: &CommandContext,
    grant: &Grant,
    state: &State,
    request: &JsonValue,
    snapshot: &Snapshot,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(PrivatePackage, JsonValue, Vec<JsonValue>)> {
    let changes = check_form_changes(cmd::array(request, "changes")?, grant)?;
    let subject = source_forms::metadata_subject(&state.record)?;
    let mut next_forms = source_forms::apply_form_changes(Some(&state.forms), &subject, &changes)?;
    validate_form_path_set(ctx, grant, &next_forms, worker, deadline, cancelled)?;
    let views = source_forms::materialize_source_forms(&state.record, &next_forms)?;
    for change in &changes {
        let form = cmd::field(change, "form")?;
        if cmd::text(cmd::field(form, "content")?, "kind")? == "source-copy" {
            let form_id = cmd::text(form, "form_id")?;
            let view = views
                .iter()
                .find(|view| {
                    view.object_get("form")
                        .and_then(|value| value.object_get("id"))
                        .and_then(JsonValue::as_str)
                        == Some(form_id)
                })
                .ok_or(SourceCommandError::Invalid(
                    "private source-copy view absent",
                ))?;
            if cmd::text(view, "state")? != "ready" {
                return Err(SourceCommandError::Invalid(
                    "private source-copy form omits mandatory source context",
                ));
            }
        }
    }
    let refs = changes
        .iter()
        .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let receipt = cmd::object(vec![
        ("command_id", cmd::string(check_command_id(request)?)),
        ("request_digest", cmd::string(&request_digest(request)?)),
        (
            "principal_id",
            cmd::field(&grant.value, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&grant.value, "authority_ref")?.clone(),
        ),
        ("owner_configuration", cmd::string(&grant.digest)),
        ("recorded_at", cmd::string(&ctx.recorded_at)),
        ("source", subject),
        (
            "previous_revision",
            cmd::field(request, "expected_revision")?.clone(),
        ),
        ("results", JsonValue::Array(refs)),
    ]);
    let mut history = next_forms
        .object_get("growth_history")
        .and_then(JsonValue::as_array)
        .unwrap_or(&[])
        .to_vec();
    history.push(receipt.clone());
    cmd::set(&mut next_forms, "growth_history", JsonValue::Array(history))?;
    validate_form_path_set(ctx, grant, &next_forms, worker, deadline, cancelled)?;
    let mut files = state.files.clone();
    files.insert(formset_name(grant)?, cmd::published(&next_forms)?);
    let _ = snapshot;
    Ok((files, receipt, views))
}

pub(crate) fn prepare(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    current: Option<&PrivatePackage>,
    inventory: &PrivateIdentityInputs,
    archive_reader: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivateProfilePlan> {
    active(deadline, cancelled)?;
    let grant = parse_grant(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        worker,
        deadline,
        cancelled,
    )?;
    let request = cmd::parse(&ctx.request_raw)?;
    let operation = request_shape(&request)?;
    let before = current.cloned();
    let readonly = |response, reads, replayed| PrivateProfilePlan {
        before: before.clone(),
        after: None,
        response,
        archive: None,
        reads,
        replayed,
    };

    match operation.as_str() {
        "describe" if current.is_none() => {
            let response = profile_result(&grant, None, None, None, false)?;
            Ok(readonly(response, Vec::new(), false))
        }
        "describe" => {
            let package = current.ok_or(SourceCommandError::Invalid(
                "private profile target disappeared",
            ))?;
            let state = inspect_state(ctx, cut, &grant, package, worker, deadline, cancelled)?;
            verify_cold_history(
                ctx,
                cut,
                context_config,
                &grant,
                package,
                &state,
                archive_reader,
                worker,
                deadline,
                cancelled,
            )?;
            let snapshot = current_snapshot(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &grant,
                &state,
                inventory,
                archive_reader,
                worker,
                deadline,
                cancelled,
            )?;
            let mut response = profile_result(&grant, Some(package), Some(&state), None, false)?;
            add_expected_dependencies(&mut response, &snapshot.dependencies)?;
            Ok(readonly(response, snapshot.native_reads, false))
        }
        "prepare-create" => {
            if !grant
                .operations
                .iter()
                .any(|value| value == "source.create")
            {
                return Err(SourceCommandError::Denied(
                    "owner-local source creation is not delegated",
                ));
            }
            let (record, files, dependencies, views, reads) = prepared_create(
                ctx,
                cut,
                &grant,
                &request,
                inventory,
                software,
                components,
                owner,
                context_config,
                archive_reader,
                worker,
                deadline,
                cancelled,
            )?;
            let subject = source_forms::metadata_subject(&record)?;
            let mut response = profile_result(&grant, current, None, None, false)?;
            cmd::set(&mut response, "prepared_source", subject)?;
            cmd::set(&mut response, "expected_source", JsonValue::Null)?;
            cmd::set(&mut response, "expected_revision", JsonValue::Null)?;
            add_expected_dependencies(&mut response, &dependencies)?;
            cmd::set(&mut response, "prepared_files", package_file_refs(&files))?;
            cmd::set(
                &mut response,
                "prepared_materializations",
                JsonValue::Array(views),
            )?;
            Ok(readonly(response, reads, false))
        }
        "source.create" => {
            if !grant
                .operations
                .iter()
                .any(|value| value == "source.create")
            {
                return Err(SourceCommandError::Denied(
                    "owner-local source creation is not delegated",
                ));
            }
            match current {
                None => {
                    let (files, response, reads) = create_package(
                        ctx,
                        cut,
                        software,
                        components,
                        owner,
                        context_config,
                        &grant,
                        &request,
                        inventory,
                        archive_reader,
                        worker,
                        deadline,
                        cancelled,
                    )?;
                    Ok(PrivateProfilePlan {
                        before: None,
                        after: Some(files),
                        response,
                        archive: None,
                        reads,
                        replayed: false,
                    })
                }
                Some(package) => {
                    let state =
                        inspect_state(ctx, cut, &grant, package, worker, deadline, cancelled)?;
                    verify_cold_history(
                        ctx,
                        cut,
                        context_config,
                        &grant,
                        package,
                        &state,
                        archive_reader,
                        worker,
                        deadline,
                        cancelled,
                    )?;
                    let (response, reads) = create_replay(
                        ctx,
                        cut,
                        software,
                        components,
                        owner,
                        context_config,
                        &grant,
                        &request,
                        package,
                        &state,
                        inventory,
                        archive_reader,
                        worker,
                        deadline,
                        cancelled,
                    )?;
                    Ok(readonly(response, reads, true))
                }
            }
        }
        "prepare-revise" | "record.revise" | "prepare" | "apply" | "inspect-version" => {
            let package = current.ok_or(SourceCommandError::Invalid(
                "private profile package is absent",
            ))?;
            let state = inspect_state(ctx, cut, &grant, package, worker, deadline, cancelled)?;
            verify_cold_history(
                ctx,
                cut,
                context_config,
                &grant,
                package,
                &state,
                archive_reader,
                worker,
                deadline,
                cancelled,
            )?;
            let snapshot = current_snapshot(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &grant,
                &state,
                inventory,
                archive_reader,
                worker,
                deadline,
                cancelled,
            )?;
            let reads = snapshot.native_reads.clone();
            match operation.as_str() {
                "prepare-revise" => {
                    let (proposed, _, refs, views) = revision_proposal(
                        ctx, cut, &grant, &state, &request, worker, deadline, cancelled,
                    )?;
                    let mut response =
                        profile_result(&grant, Some(package), Some(&state), None, false)?;
                    cmd::set(
                        &mut response,
                        "prepared_source",
                        source_forms::metadata_subject(&proposed)?,
                    )?;
                    cmd::set(&mut response, "prepared_forms", JsonValue::Array(refs))?;
                    add_expected_dependencies(&mut response, &snapshot.dependencies)?;
                    let _ = views;
                    Ok(readonly(response, reads, false))
                }
                "record.revise" => {
                    if let Some(receipt) =
                        retained_command(&state, &request, &grant, &snapshot.dependencies)?
                    {
                        let response = profile_result(
                            &grant,
                            Some(package),
                            Some(&state),
                            Some(&receipt),
                            true,
                        )?;
                        return Ok(readonly(response, reads, true));
                    }
                    let revision = package_revision(package)?;
                    request_commit_matches(
                        &request,
                        &grant,
                        Some(&state.subject),
                        Some(&revision),
                        &snapshot.dependencies,
                    )?;
                    let (proposed, mut output, refs, views) = revision_proposal(
                        ctx, cut, &grant, &state, &request, worker, deadline, cancelled,
                    )?;
                    let receipt = cmd::object(vec![
                        ("command_id", cmd::string(check_command_id(&request)?)),
                        ("request_digest", cmd::string(&request_digest(&request)?)),
                        (
                            "principal_id",
                            cmd::field(&grant.value, "principal_id")?.clone(),
                        ),
                        (
                            "authority_ref",
                            cmd::field(&grant.value, "authority_ref")?.clone(),
                        ),
                        ("owner_configuration", cmd::string(&grant.digest)),
                        ("recorded_at", cmd::string(&ctx.recorded_at)),
                        ("reason", cmd::field(&request, "reason")?.clone()),
                        ("previous_source", state.subject.clone()),
                        ("source", source_forms::metadata_subject(&proposed)?),
                        ("previous_revision", cmd::string(&revision)),
                        (
                            "archive_path",
                            cmd::string(&archive_reference(
                                &grant.private_prefix,
                                cmd::text(&grant.value, "record_id")?,
                                &revision,
                            )?),
                        ),
                        ("dependencies", cmd::string(&snapshot.dependencies)),
                        (
                            "changed_fields",
                            revision_fields_sorted(cmd::field(&request, "fields")?)?,
                        ),
                        ("forms", JsonValue::Array(refs)),
                        ("grants_admission", JsonValue::Bool(false)),
                        ("request", request.clone()),
                    ]);
                    let mut receipts = cmd::array(&state.history, "receipts")?.to_vec();
                    receipts.push(receipt.clone());
                    let history = history_with_receipts(&state.history, receipts)?;
                    output.insert(HISTORY.into(), encode(&history)?);
                    let (archive_path, archived) = archive_package(
                        &state.files,
                        &grant.private_prefix,
                        grant.source_path.as_str(),
                        &state.subject,
                        &revision,
                    )?;
                    let next_state =
                        inspect_state(ctx, cut, &grant, &output, worker, deadline, cancelled)?;
                    let response = profile_result(
                        &grant,
                        Some(&output),
                        Some(&next_state),
                        Some(&receipt),
                        false,
                    )?;
                    let _ = views;
                    Ok(PrivateProfilePlan {
                        before: Some(package.clone()),
                        after: Some(output),
                        response,
                        archive: Some((archive_path, archived)),
                        reads,
                        replayed: false,
                    })
                }
                "prepare" => {
                    let form_id = cmd::text(&request, "form_id")?;
                    if !grant.form_ids.iter().any(|value| value == form_id) {
                        return Err(SourceCommandError::Denied(
                            "prepared form identity is outside delegation",
                        ));
                    }
                    let change = source_forms::prepare_form_change(
                        &state.record,
                        Some(&state.forms),
                        cmd::text(&grant.value, "principal_id")?,
                        form_id,
                        cmd::text(&request, "field_id")?,
                    )?;
                    if !grant.operations.iter().any(|allowed| {
                        Some(allowed.as_str())
                            == change.object_get("operation").and_then(JsonValue::as_str)
                    }) {
                        return Err(SourceCommandError::Denied(
                            "prepared form operation is outside delegation",
                        ));
                    }
                    let preview_set = source_forms::apply_form_changes(
                        Some(&state.forms),
                        &state.subject,
                        std::slice::from_ref(&change),
                    )?;
                    validate_form_path_set(ctx, &grant, &preview_set, worker, deadline, cancelled)?;
                    let views =
                        source_forms::materialize_source_forms(&state.record, &preview_set)?;
                    let preview = views
                        .into_iter()
                        .find(|view| {
                            view.object_get("form")
                                .and_then(|form| form.object_get("id"))
                                .and_then(JsonValue::as_str)
                                == Some(form_id)
                        })
                        .ok_or(SourceCommandError::Invalid(
                            "prepared private form materialization absent",
                        ))?;
                    if cmd::text(&preview, "state")? != "ready" {
                        return Err(SourceCommandError::Invalid(
                            "prepared private exact source-copy is not ready",
                        ));
                    }
                    let mut response =
                        profile_result(&grant, Some(package), Some(&state), None, false)?;
                    cmd::set(&mut response, "prepared_change", change)?;
                    add_expected_dependencies(&mut response, &snapshot.dependencies)?;
                    Ok(readonly(response, reads, false))
                }
                "apply" => {
                    if let Some(receipt) =
                        retained_command(&state, &request, &grant, &snapshot.dependencies)?
                    {
                        let response = profile_result(
                            &grant,
                            Some(package),
                            Some(&state),
                            Some(&receipt),
                            true,
                        )?;
                        return Ok(readonly(response, reads, true));
                    }
                    let revision = package_revision(package)?;
                    request_commit_matches(
                        &request,
                        &grant,
                        Some(&state.subject),
                        Some(&revision),
                        &snapshot.dependencies,
                    )?;
                    let (output, receipt, _) = apply_form_successor(
                        ctx, &grant, &state, &request, &snapshot, worker, deadline, cancelled,
                    )?;
                    let next_state =
                        inspect_state(ctx, cut, &grant, &output, worker, deadline, cancelled)?;
                    let response = profile_result(
                        &grant,
                        Some(&output),
                        Some(&next_state),
                        Some(&receipt),
                        false,
                    )?;
                    Ok(PrivateProfilePlan {
                        before: Some(package.clone()),
                        after: Some(output),
                        response,
                        archive: None,
                        reads,
                        replayed: false,
                    })
                }
                "inspect-version" => {
                    let requested = cmd::field(&request, "source")?;
                    let receipts = cmd::array(&state.history, "receipts")?;
                    let receipt = receipts
                        .iter()
                        .find(|receipt| {
                            receipt
                                .object_get("previous_source")
                                .is_some_and(|source| cmd::same(source, requested).unwrap_or(false))
                        })
                        .ok_or(SourceCommandError::Conflict(
                            "exact private source version is not retained",
                        ))?;
                    let reference = cmd::text(receipt, "archive_path")?;
                    let archived_raw =
                        archive_reader.read_archive(reference, deadline, cancelled)?;
                    let (archived, locations) =
                        decode_archive(reference, archived_raw, receipt, &grant)?;
                    let basename = grant.source_path.as_str().rsplit('/').next().ok_or(
                        SourceCommandError::Invalid("private profile source basename"),
                    )?;
                    let archived_record = cmd::parse(archived.get(basename).ok_or(
                        SourceCommandError::Invalid("private archived source record absent"),
                    )?)?;
                    let mut response =
                        profile_result(&grant, Some(package), Some(&state), None, false)?;
                    cmd::set(&mut response, "record", archived_record)?;
                    cmd::set(&mut response, "inspected_source", requested.clone())?;
                    cmd::set(&mut response, "files", locations)?;
                    Ok(readonly(response, reads, false))
                }
                _ => Err(SourceCommandError::Unsupported("private profile operation")),
            }
        }
        _ => Err(SourceCommandError::Unsupported("private profile operation")),
    }
}
