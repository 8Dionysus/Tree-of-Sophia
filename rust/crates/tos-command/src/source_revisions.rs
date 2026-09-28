//! Maintained record revision preparation (handlers 22–27).
//!
//! Exact selected bytes produce proposed record/form/history and retained
//! predecessor writes. Native reservation can read exact anchored manifest
//! members; this module performs no ambient discovery or publication. Schema
//! probes and current account observations do not grant
//! admission; `PreparedCommand::commit` remains a production-admission refusal.

use crate::source_command::{self as cmd, *};
use crate::source_forms;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath, python_strip_unicode16_v1};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const HISTORY: &str = "source-revision-history.json";
const PROTOCOL: &str = "tos_selected_source_metadata_v1";
const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CORPUS_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
const BASE_FIELDS: &[&str] = &[
    "preferred_label",
    "variant_labels",
    "notes",
    "field_languages",
    "source_refs",
    "extensions",
    "semantic_content",
];
const CORPUS_FIELDS: &[&str] = &["preferred_label", "notes", "field_languages", "source_refs"];
const DEPENDENCIES: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_record_profiles.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/source_witness_human_forms.py",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
const SELECTED_DEPENDENCIES: &[&str] = &[
    "scripts/source_metadata_snapshot.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_selected_revisions.py",
];

/// Native v1/v2 grants stay separate; v3 alone admits descriptive Edition,
/// Collection and Item corrections. Native witnesses never become Corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionFamily {
    Historical,
    PublicProfile,
    PublicProfileScope,
    CorpusFlat,
    CorpusSelectedV2,
    CorpusSelectedV3,
    NativeSelected,
}
impl RevisionFamily {
    pub fn handler_id(self) -> &'static str {
        match self {
            Self::Historical => "historical-source-revision",
            Self::PublicProfile => "public-profile-revision",
            Self::PublicProfileScope => "public-profile-scope-revision",
            Self::CorpusFlat => "native-corpus-flat-revision",
            Self::CorpusSelectedV2 | Self::CorpusSelectedV3 => "native-corpus-selected-revision",
            Self::NativeSelected => "native-witness-link-selected-revision",
        }
    }
    fn selected(self) -> bool {
        matches!(
            self,
            Self::CorpusSelectedV2 | Self::CorpusSelectedV3 | Self::NativeSelected
        )
    }
    fn profile(self) -> bool {
        matches!(self, Self::PublicProfile | Self::PublicProfileScope)
    }
    fn parse(schema: &str) -> SourceCommandResult<Self> {
        Ok(match schema {
            "tos_local_source_revision_owner_v1" => Self::Historical,
            "tos_local_profile_revision_owner_v1" => Self::PublicProfile,
            "tos_local_profile_revision_owner_v2" => Self::PublicProfileScope,
            "tos_local_corpus_revision_owner_v1" => Self::CorpusFlat,
            "tos_local_corpus_revision_owner_v2" => Self::CorpusSelectedV2,
            "tos_local_corpus_revision_owner_v3" => Self::CorpusSelectedV3,
            "tos_local_native_metadata_revision_owner_v1" => Self::NativeSelected,
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "record revision owner schema",
                ));
            }
        })
    }
}

/// Evidence selected by the publication owner, never supplied by request prose.
/// Every retained transaction must contain its exact original revision request
/// and before/after bytes. Recovery reconstructs these bytes independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedRevisionTransaction {
    pub transaction_id: String,
    pub status: RevisionTransactionStatus,
    pub authorization_raw: Vec<u8>,
    pub before: Vec<SourceFile>,
    pub after: Vec<SourceFile>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionTransactionStatus {
    Pending,
    Committed,
    RolledBack,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RevisionPublication {
    /// Exact observed cooperating-reader token (None is the legacy baseline).
    pub token: Option<String>,
    pub transactions: Vec<RetainedRevisionTransaction>,
}

pub(crate) type Package = BTreeMap<String, Vec<u8>>;
struct Inspection {
    files: Package,
    record: JsonValue,
    subject: JsonValue,
    history: JsonValue,
    profile: JsonValue,
    schemas: Vec<String>,
    native_identity_snapshot: Option<String>,
    native_text_snapshot: Option<String>,
}

fn path(value: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(value).map_err(|_| SourceCommandError::Invalid("unsafe source path"))
}
fn required<'a>(ctx: &'a CommandContext, name: &str) -> SourceCommandResult<&'a [u8]> {
    ctx.file(&path(name)?)?
        .ok_or(SourceCommandError::Unsupported(
            "required exact source contract or retained bytes absent",
        ))
}
fn strip(value: &str) -> SourceCommandResult<&str> {
    python_strip_unicode16_v1(value, 1_048_576)
        .map_err(|_| SourceCommandError::Invalid("bounded Python string strip"))
}
fn split(name: &str) -> SourceCommandResult<(&str, &str)> {
    name.rsplit_once('/').ok_or(SourceCommandError::Invalid(
        "source path has no owner parent",
    ))
}
fn names(source_path: &str) -> SourceCommandResult<[String; 3]> {
    let (_, base) = split(source_path)?;
    let stem = base
        .strip_suffix(".json")
        .ok_or(SourceCommandError::Invalid("source record basename"))?;
    Ok([
        base.into(),
        format!("{stem}.human-forms.json"),
        HISTORY.into(),
    ])
}
fn texts(v: &JsonValue, key: &str, max: usize) -> SourceCommandResult<Vec<String>> {
    let values = cmd::array(v, key)?;
    if values.len() > max {
        return Err(SourceCommandError::Invalid("scope array budget"));
    }
    let result = values
        .iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or(SourceCommandError::Invalid("scope text"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(SourceCommandError::Invalid("duplicate scope member"));
    }
    Ok(result)
}
fn has(v: &JsonValue, key: &str, name: &str) -> SourceCommandResult<bool> {
    Ok(texts(v, key, 128)?.iter().any(|s| s == name))
}
pub(crate) fn valid_id(id: &str, prefix: &str, form: bool) -> bool {
    let Some(tail) = id.strip_prefix(prefix) else {
        return false;
    };
    !tail.is_empty()
        && tail.as_bytes()[0].is_ascii_lowercase_or_digit()
        && tail.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'.'
                || b == b'-'
                || form && b == b'_'
        })
        && (form || !tail.split(['.', '-']).any(str::is_empty))
}
trait LowerOrDigit {
    fn is_ascii_lowercase_or_digit(self) -> bool;
}
impl LowerOrDigit for u8 {
    fn is_ascii_lowercase_or_digit(self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}
fn digest_text(value: &JsonValue) -> SourceCommandResult<&str> {
    let text = value
        .as_str()
        .ok_or(SourceCommandError::Invalid("digest text"))?;
    let suffix = text
        .strip_prefix("sha256:")
        .ok_or(SourceCommandError::Invalid("digest prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(SourceCommandError::Invalid("digest grammar"));
    }
    Ok(text)
}
fn configuration(ctx: &CommandContext) -> SourceCommandResult<(JsonValue, RevisionFamily)> {
    ctx.check()?;
    let config = cmd::parse(&ctx.configuration_raw)?;
    let family = RevisionFamily::parse(cmd::text(&config, "schema_version")?)?;
    let mut keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "source_root",
        "source_path",
        "authority_ref",
        "allowed_form_ids",
        "allowed_operations",
        "expires_at",
        "record_id",
        "allowed_fields",
    ];
    if family.profile() {
        keys.push("profile_type_id");
    }
    if matches!(
        family,
        RevisionFamily::CorpusFlat
            | RevisionFamily::CorpusSelectedV2
            | RevisionFamily::CorpusSelectedV3
            | RevisionFamily::NativeSelected
    ) {
        keys.push("record_type");
    }
    if family == RevisionFamily::NativeSelected {
        keys.push("record_schema_version");
    }
    cmd::exact_keys(&config, &keys)?;
    if cmd::integer(&config, "uid")? != ctx.effective_uid
        || ["principal_id", "authority_ref"].iter().any(|k| {
            cmd::text(&config, k)
                .and_then(strip)
                .map_or(true, str::is_empty)
        })
    {
        return Err(SourceCommandError::Denied(
            "current Unix account or owner identity",
        ));
    }
    cmd::validate_expiry(cmd::text(&config, "expires_at")?, &ctx.recorded_at)?;
    if !cmd::text(&config, "source_root")?.starts_with('/') {
        return Err(SourceCommandError::Denied("source root must be absolute"));
    }
    let source_path = cmd::text(&config, "source_path")?;
    path(source_path)?;
    let parts: Vec<_> = source_path.split('/').collect();
    if parts.len() < 5
        || parts[..2] != ["ToS", "source-witnesses"]
        || parts
            .iter()
            .any(|p| matches!(*p, "payload" | "local-content" | "catalog" | "owner-local"))
        || !source_path.ends_with(".json")
        || source_path.ends_with(".human-forms.json")
    {
        return Err(SourceCommandError::Denied(
            "exact public metadata source path",
        ));
    }
    if family == RevisionFamily::NativeSelected && parts.iter().any(|p| p.starts_with('.')) {
        return Err(SourceCommandError::Denied(
            "native source path hidden component",
        ));
    }
    let operations = texts(&config, "allowed_operations", 32)?;
    if operations
        .iter()
        .any(|op| op != "record.revise" && !(family.selected() && op == "record.recover"))
    {
        return Err(SourceCommandError::Denied(
            "record revision operation grant",
        ));
    }
    let forms = texts(&config, "allowed_form_ids", 32)?;
    if forms.iter().any(|id| !valid_id(id, "tos.form.", true))
        || family == RevisionFamily::NativeSelected && forms.is_empty()
    {
        return Err(SourceCommandError::Denied("record revision form scope"));
    }
    let allowed = allowed_fields(family, &config)?;
    if texts(&config, "allowed_fields", 128)?
        .iter()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(SourceCommandError::Denied("record revision field grant"));
    }
    if family == RevisionFamily::Historical {
        let id = cmd::text(&config, "record_id")?;
        let kind = ["historical-event", "historical-process", "historical-state"]
            .into_iter()
            .find(|kind| valid_id(id, &format!("tos.{kind}."), false))
            .ok_or(SourceCommandError::Denied("historical source identity"))?;
        if parts.last() != Some(&format!("{kind}.json").as_str()) {
            return Err(SourceCommandError::Denied("historical typed basename"));
        }
    }
    Ok((config, family))
}

fn history(files: &Package, record: &JsonValue) -> SourceCommandResult<JsonValue> {
    let subject = source_forms::metadata_subject(record)?;
    let value = match files.get(HISTORY) {
        Some(raw) => cmd::parse(raw)?,
        None => cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_revision_history_v1"),
            ),
            ("record_id", cmd::field(&subject, "id")?.clone()),
            ("receipts", JsonValue::Array(vec![])),
        ]),
    };
    cmd::exact_keys(&value, &["schema_version", "record_id", "receipts"])?;
    let v2 = cmd::text(&value, "schema_version")? == "tos_source_revision_history_v2";
    if !v2 && cmd::text(&value, "schema_version")? != "tos_source_revision_history_v1"
        || cmd::field(&value, "record_id")? != cmd::field(&subject, "id")?
    {
        return Err(SourceCommandError::Invalid(
            "record revision history identity or schema",
        ));
    }
    let receipts = cmd::array(&value, "receipts")?;
    if receipts.len() > 128 || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(SourceCommandError::Invalid(
            "retained revision history capacity or empty ledger",
        ));
    }
    let mut commands = BTreeSet::new();
    let mut previous: Option<&JsonValue> = None;
    for receipt in receipts {
        let selected = receipt.object_get("publication").is_some();
        let mut keys = vec![
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
        ];
        if selected {
            keys.push("publication");
        }
        cmd::exact_keys(receipt, &keys)?;
        if selected && !v2 {
            return Err(SourceCommandError::Invalid(
                "selected revision requires v2 history",
            ));
        }
        let request = cmd::field(receipt, "request")?;
        if cmd::text(request, "operation")? != "record.revise" {
            return Err(SourceCommandError::Unsupported(
                "compound parent history requires its typed owner receipt verifier",
            ));
        }
        exact_ref(cmd::field(receipt, "previous_source")?)?;
        exact_ref(cmd::field(receipt, "source")?)?;
        let fields = cmd::field(request, "fields")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("retained fields"))?;
        let mut changed = fields
            .iter()
            .map(|(k, _)| {
                k.as_str()
                    .map(String::from)
                    .ok_or(SourceCommandError::Invalid("retained field name"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        changed.sort();
        let changed = JsonValue::Array(changed.iter().map(|s| cmd::string(s)).collect());
        if !commands.insert(cmd::text(receipt, "command_id")?)
            || cmd::text(receipt, "request_digest")? != cmd::record_digest(request)?.to_prefixed()
            || cmd::field(receipt, "command_id")? != cmd::field(request, "command_id")?
            || !cmd::same(
                cmd::field(receipt, "previous_source")?,
                cmd::field(request, "expected_source")?,
            )?
            || cmd::field(receipt, "previous_revision")?
                != cmd::field(request, "expected_revision")?
            || cmd::field(receipt, "owner_configuration")?
                != cmd::field(request, "expected_configuration")?
            || cmd::field(receipt, "dependencies")? != cmd::field(request, "expected_dependencies")?
            || cmd::field(receipt, "reason")? != cmd::field(request, "reason")?
            || cmd::field(receipt, "changed_fields")? != &changed
            || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
            || cmd::field(cmd::field(receipt, "source")?, "id")? != cmd::field(&subject, "id")?
            || cmd::field(cmd::field(receipt, "previous_source")?, "id")?
                != cmd::field(&subject, "id")?
            || cmd::integer(cmd::field(receipt, "previous_source")?, "version")?.checked_add(1)
                != Some(cmd::integer(cmd::field(receipt, "source")?, "version")?)
            || previous
                .map(|p| cmd::same(p, cmd::field(receipt, "previous_source")?))
                .transpose()?
                .is_some_and(|same| !same)
        {
            return Err(SourceCommandError::Conflict(
                "broken retained source revision lineage",
            ));
        }
        // Timestamp parser is shared with the protected owner configuration;
        // a receipt need only be valid, not unexpired.
        tos_validation::retirement_rules::observed_instant_order(
            cmd::text(receipt, "recorded_at")?,
            cmd::text(receipt, "recorded_at")?,
        )
        .map_err(|_| SourceCommandError::Invalid("retained receipt instant"))?;
        if selected {
            let publication = cmd::field(receipt, "publication")?;
            cmd::exact_keys(
                publication,
                &["protocol", "transaction_id", "selected_files"],
            )?;
            if cmd::text(publication, "protocol")? != PROTOCOL {
                return Err(SourceCommandError::Invalid("selected publication protocol"));
            }
            digest_text(cmd::field(publication, "transaction_id")?)?;
            let names = texts(publication, "selected_files", 3)?;
            if names.len() != 3
                || !names.iter().any(|n| n == HISTORY)
                || names.iter().any(|n| n.contains('/') || n.is_empty())
            {
                return Err(SourceCommandError::Invalid("selected publication files"));
            }
        }
        previous = Some(cmd::field(receipt, "source")?);
    }
    if let Some(previous) = previous
        && !cmd::same(previous, &subject)?
    {
        return Err(SourceCommandError::Conflict(
            "current record is not retained revision head",
        ));
    }
    Ok(value)
}

fn read_archive(
    ctx: &CommandContext,
    config: &JsonValue,
    receipt: &JsonValue,
) -> SourceCommandResult<(Package, JsonValue)> {
    let location = archive_path(config, cmd::text(receipt, "previous_revision")?)?;
    if cmd::text(receipt, "archive_path")? != location {
        return Err(SourceCommandError::Conflict(
            "archive locator is not derived from exact subject and package",
        ));
    }
    let manifest = cmd::parse(required(ctx, &format!("{location}/manifest.json"))?)?;
    let selected = cmd::text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected {
        keys.push("publication_protocol");
    }
    cmd::exact_keys(&manifest, &keys)?;
    if !selected && cmd::text(&manifest, "schema_version")? != "tos_source_package_archive_v1"
        || selected && cmd::text(&manifest, "publication_protocol")? != PROTOCOL
        || cmd::field(&manifest, "source_path")? != cmd::field(config, "source_path")?
        || !cmd::same(
            cmd::field(&manifest, "source")?,
            cmd::field(receipt, "previous_source")?,
        )?
        || cmd::field(&manifest, "revision")? != cmd::field(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "archive manifest binding differs",
        ));
    }
    let refs = cmd::field(&manifest, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("archive files map"))?;
    if refs.is_empty() || refs.len() > 64 {
        return Err(SourceCommandError::Invalid("archive package file budget"));
    }
    let mut files = Package::new();
    let mut locations = Vec::new();
    let mut blobs = BTreeSet::new();
    for (name, binding) in refs {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("archive filename"))?;
        if name.contains('/') || name.is_empty() || matches!(name, "." | "..") {
            return Err(SourceCommandError::Invalid("archive basename"));
        }
        cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let digest = digest_text(cmd::field(binding, "sha256")?)?;
        let blob = cmd::text(binding, "blob")?;
        if blob != format!("{}.blob", &digest[7..]) {
            return Err(SourceCommandError::Invalid("archive blob locator"));
        }
        let blob_path = format!("{location}/{blob}");
        let raw = required(ctx, &blob_path)?;
        if raw.len() > 2_097_152
            || Digest256::of_bytes(raw).to_prefixed() != digest
            || raw.len() as u64 != cmd::integer(binding, "bytes")?
        {
            return Err(SourceCommandError::Conflict("archive byte binding differs"));
        }
        blobs.insert(blob.to_string());
        files.insert(name.into(), raw.to_vec());
        locations.push((
            JsonString::from_utf8(name),
            cmd::object(vec![
                ("archive_path", cmd::string(&blob_path)),
                ("sha256", cmd::string(digest)),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        ));
    }
    let prefix = format!("{location}/");
    for file in &ctx.files {
        if let Some(name) = file.path.as_str().strip_prefix(&prefix) {
            if name != "manifest.json" && !blobs.contains(name) {
                return Err(SourceCommandError::Conflict(
                    "archive contains unbound selected file",
                ));
            }
        }
    }
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    if selected
        && (!files.contains_key(base)
            || files
                .keys()
                .any(|n| !names(source_path).is_ok_and(|names| names.contains(n))))
    {
        return Err(SourceCommandError::Conflict(
            "selected archive exceeds exact metadata scope",
        ));
    }
    if files.values().map(Vec::len).sum::<usize>() > 8_388_608
        || revision(&files)? != cmd::text(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "archive package digest or budget",
        ));
    }
    let old = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Conflict("archive record missing"))?,
    )?;
    if !cmd::same(
        &source_forms::metadata_subject(&old)?,
        cmd::field(receipt, "previous_source")?,
    )? {
        return Err(SourceCommandError::Conflict(
            "archive exact source mismatch",
        ));
    }
    if let Some(request) = receipt.object_get("request") {
        let successor = revised(&old, request)?;
        if !cmd::same(
            &source_forms::metadata_subject(&successor)?,
            cmd::field(receipt, "source")?,
        )? {
            return Err(SourceCommandError::Conflict(
                "retained request does not reconstruct successor",
            ));
        }
    }
    Ok((files, JsonValue::Object(locations)))
}
fn verify_history(
    ctx: &CommandContext,
    config: &JsonValue,
    files: &Package,
    record: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    let value = history(files, record)?;
    let (_, base) = split(cmd::text(config, "source_path")?)?;
    let receipts = cmd::array(&value, "receipts")?;
    for (index, receipt) in receipts.iter().enumerate() {
        let (archived, _) = read_archive(ctx, config, receipt)?;
        let previous = cmd::parse(
            archived
                .get(base)
                .ok_or(SourceCommandError::Conflict("archived source missing"))?,
        )?;
        let retained = history(&archived, &previous)?;
        if !cmd::same(
            cmd::field(&retained, "receipts")?,
            &JsonValue::Array(receipts[..index].to_vec()),
        )? {
            return Err(SourceCommandError::Conflict(
                "retained predecessor history prefix differs",
            ));
        }
    }
    Ok(value)
}
fn inspect(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
) -> SourceCommandResult<Inspection> {
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    let files = package(ctx, source_path, family.selected())?;
    let record = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Invalid("record missing"))?,
    )?;
    let (profile, schemas, native_identity_snapshot, native_text_snapshot) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &record,
    )?;
    let subject = source_forms::metadata_subject(&record)?;
    let history = verify_history(ctx, config, &files, &record)?;
    Ok(Inspection {
        files,
        record,
        subject,
        history,
        profile,
        schemas,
        native_identity_snapshot,
        native_text_snapshot,
    })
}
fn dependencies(
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
) -> SourceCommandResult<String> {
    let mut names: BTreeSet<String> = DEPENDENCIES.iter().map(|s| s.to_string()).collect();
    if !family.profile() {
        names.extend(inspection.schemas.iter().cloned());
    }
    if family.selected() {
        names.extend(SELECTED_DEPENDENCIES.iter().map(|s| s.to_string()));
    }
    if family == RevisionFamily::NativeSelected {
        names.extend(["mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_native_metadata_commands.py".into(),"scripts/build_source_witness_catalog.py".into()]);
    }
    let mut entries = names
        .iter()
        .map(|name| {
            Ok((
                JsonString::from_utf8(name),
                cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
            ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if family.profile() {
        let contracts = inspection
            .schemas
            .iter()
            .map(|name| {
                Ok((
                    JsonString::from_utf8(name),
                    cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
                ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        entries.push((
            JsonString::from_utf8("source_contracts"),
            JsonValue::Object(contracts),
        ));
    }
    if let Some(snapshot) = &inspection.native_identity_snapshot {
        entries.push((
            JsonString::from_utf8("native_semantic_identity_snapshot"),
            cmd::string(snapshot),
        ));
    }
    if let Some(snapshot) = &inspection.native_text_snapshot {
        entries.push((
            JsonString::from_utf8("native_text_binding_snapshot"),
            cmd::string(snapshot),
        ));
        entries.push((
            JsonString::from_utf8("native_binding_implementation"),
            JsonValue::Object(
                [
                    "scripts/native_text_binding.py",
                    "scripts/source_owner_context.py",
                ]
                .iter()
                .map(|name| {
                    Ok((
                        JsonString::from_utf8(name),
                        cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
                    ))
                })
                .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        ));
    }
    let _ = config;
    Ok(cmd::record_digest(&JsonValue::Object(entries))?.to_prefixed())
}
fn revised(record: &JsonValue, request: &JsonValue) -> SourceCommandResult<JsonValue> {
    let mut revised = record.clone();
    for (key, value) in cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields"))?
    {
        cmd::set(
            &mut revised,
            key.as_str()
                .ok_or(SourceCommandError::Invalid("revision field name"))?,
            value.clone(),
        )?;
    }
    let version = cmd::integer(record, "record_version")?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid("record version exhausted"))?;
    cmd::set(&mut revised, "record_version", cmd::number(version))?;
    Ok(revised)
}
fn proposal(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    scope_operation: &str,
) -> SourceCommandResult<(
    JsonValue,
    JsonValue,
    Package,
    Vec<JsonValue>,
    Vec<JsonValue>,
)> {
    scope(config, request, scope_operation)?;
    let revised = revised(&inspection.record, request)?;
    let (_, schemas, _, _) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &revised,
    )?;
    if schemas != inspection.schemas {
        return Err(SourceCommandError::Denied(
            "correction changed source schema selection",
        ));
    }
    if family == RevisionFamily::NativeSelected && cmd::text(config, "record_type")? == "artifact" {
        for field in ["basis", "provider_independent"] {
            if cmd::field(cmd::field(&inspection.record, "path_identity")?, field)?
                != cmd::field(cmd::field(&revised, "path_identity")?, field)?
            {
                return Err(SourceCommandError::Denied(
                    "physical identity path basis changed",
                ));
            }
        }
    }
    let source_path = cmd::text(config, "source_path")?;
    let [basename, formname, _] = names(source_path)?;
    let current = inspection
        .files
        .get(&formname)
        .map(|b| cmd::parse(b))
        .transpose()?;
    if let Some(current) = &current {
        validate_forms(worker, deadline, cancelled, ctx, current)?;
        let selected = cmd::array(request, "forms")?
            .iter()
            .map(|v| cmd::text(v, "form_id"))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        for form in cmd::array(current, "forms")? {
            if !selected.contains(cmd::text(form, "form_id")?) {
                return Err(SourceCommandError::Invalid(
                    "revision must rebind every current form",
                ));
            }
        }
    }
    let subject = source_forms::metadata_subject(&revised)?;
    let changes = cmd::array(request, "forms")?
        .iter()
        .map(|selection| {
            source_forms::prepare_form_change(
                &revised,
                current.as_ref(),
                cmd::text(config, "principal_id")?,
                cmd::text(selection, "form_id")?,
                cmd::text(selection, "field_id")?,
            )
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let forms = source_forms::apply_form_changes(current.as_ref(), &subject, &changes)?;
    validate_forms(worker, deadline, cancelled, ctx, &forms)?;
    let views = source_forms::materialize_source_forms(&revised, &forms)?;
    if views
        .iter()
        .any(|v| cmd::text(v, "state").ok() != Some("ready"))
        || !views
            .iter()
            .any(|v| cmd::text(v, "role").ok() == Some("name"))
    {
        return Err(SourceCommandError::Invalid(
            "revision forms must be ready copies including a name",
        ));
    }
    let refs = changes
        .iter()
        .map(|change| cmd::reference(cmd::field(change, "form")?, "form_id", "form_version"))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let mut output = inspection.files.clone();
    let raw = cmd::published(&revised)?;
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "revised metadata reader budget",
        ));
    }
    output.insert(basename, raw);
    output.insert(formname, cmd::published(&forms)?);
    Ok((revised, subject, output, views, refs))
}
fn validate_forms(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    forms: &JsonValue,
) -> SourceCommandResult<()> {
    let refs = [
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
    ]
    .map(String::from);
    schema(
        worker,
        deadline,
        cancelled,
        ctx,
        &refs,
        "ToS/contracts/human-form-set.schema.json",
        forms,
    )?;
    tos_validation::source_forms::inspect_lineage_raw(&cmd::published(forms)?)
        .map_err(|_| SourceCommandError::Conflict("human form retained lineage"))?;
    Ok(())
}
fn result(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    publication: Option<&RevisionPublication>,
    receipt: Option<&JsonValue>,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let [_, formname, _] = names(cmd::text(config, "source_path")?)?;
    let views = match inspection.files.get(&formname) {
        Some(raw) => {
            let forms = cmd::parse(raw)?;
            validate_forms(worker, deadline, cancelled, ctx, &forms)?;
            source_forms::materialize_source_forms(&inspection.record, &forms)?
        }
        None => vec![],
    };
    let mut operations = vec!["describe", "prepare-revise", "record.revise"];
    let mut supported = vec!["record.revise"];
    if family.selected() {
        operations.push("record.recover");
        supported.push("record.recover");
    }
    operations.push("inspect-version");
    let mut value = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.selected() {
                "tos_local_source_revision_result_v2"
            } else {
                "tos_local_source_revision_result_v1"
            }),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        ("source_path", cmd::field(config, "source_path")?.clone()),
        ("source", inspection.subject.clone()),
        ("revision", cmd::string(&revision(&inspection.files)?)),
        (
            "command_operations",
            JsonValue::Array(operations.into_iter().map(cmd::string).collect()),
        ),
        (
            "supported_operations",
            JsonValue::Array(supported.into_iter().map(cmd::string).collect()),
        ),
        (
            "allowed_operations",
            cmd::field(config, "allowed_operations")?.clone(),
        ),
        (
            "allowed_fields",
            cmd::field(config, "allowed_fields")?.clone(),
        ),
        (
            "allowed_form_ids",
            cmd::field(config, "allowed_form_ids")?.clone(),
        ),
        ("receipt", receipt.cloned().unwrap_or(JsonValue::Null)),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
        ("materializations", JsonValue::Array(views)),
    ]);
    if family.selected() {
        let publication = publication.ok_or(SourceCommandError::Unsupported(
            "selected cooperating-reader publication evidence required",
        ))?;
        cmd::set(
            &mut value,
            "publication_snapshot",
            publication
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        )?;
        cmd::set(&mut value, "publication_protocol", cmd::string(PROTOCOL))?;
        let mut selected = names(cmd::text(config, "source_path")?)?.to_vec();
        selected.sort();
        cmd::set(
            &mut value,
            "selected_files",
            JsonValue::Array(selected.iter().map(|v| cmd::string(v)).collect()),
        )?;
        cmd::set(&mut value, "recovery", JsonValue::Null)?;
    }
    if family.profile() {
        cmd::set(
            &mut value,
            "profile_type_id",
            cmd::field(config, "profile_type_id")?.clone(),
        )?;
        cmd::set(
            &mut value,
            "source_record_profile",
            inspection.profile.clone(),
        )?;
    } else if family != RevisionFamily::Historical {
        cmd::set(
            &mut value,
            "record_type",
            cmd::field(&inspection.profile, "record_type")?.clone(),
        )?;
        cmd::set(&mut value, "source_profile", inspection.profile.clone())?;
    }
    Ok(value)
}

fn transaction_id(request: &JsonValue) -> SourceCommandResult<String> {
    let binding = cmd::object(vec![
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "expected_configuration",
            cmd::field(request, "expected_configuration")?.clone(),
        ),
    ]);
    let mut raw = cmd::canonical(&binding)?;
    raw.extend(cmd::canonical(request)?);
    Ok(Digest256::of_bytes(&raw).to_prefixed())
}
fn receipt(
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    subject: &JsonValue,
    refs: &[JsonValue],
    instant: &str,
) -> SourceCommandResult<JsonValue> {
    let fields = cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields"))?;
    let mut changed = fields
        .iter()
        .map(|(k, _)| {
            k.as_str()
                .map(String::from)
                .ok_or(SourceCommandError::Invalid("revision field name"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    changed.sort();
    let mut value = cmd::object(vec![
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "request_digest",
            cmd::string(&cmd::record_digest(request)?.to_prefixed()),
        ),
        ("principal_id", cmd::field(config, "principal_id")?.clone()),
        (
            "authority_ref",
            cmd::field(config, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::field(request, "expected_configuration")?.clone(),
        ),
        ("recorded_at", cmd::string(instant)),
        ("reason", cmd::field(request, "reason")?.clone()),
        ("previous_source", inspection.subject.clone()),
        ("source", subject.clone()),
        (
            "previous_revision",
            cmd::field(request, "expected_revision")?.clone(),
        ),
        (
            "archive_path",
            cmd::string(&archive_path(
                config,
                cmd::text(request, "expected_revision")?,
            )?),
        ),
        (
            "dependencies",
            cmd::field(request, "expected_dependencies")?.clone(),
        ),
        (
            "changed_fields",
            JsonValue::Array(changed.iter().map(|s| cmd::string(s)).collect()),
        ),
        ("forms", JsonValue::Array(refs.to_vec())),
        ("grants_admission", JsonValue::Bool(false)),
        ("request", request.clone()),
    ]);
    if family.selected() {
        let mut files = names(cmd::text(config, "source_path")?)?.to_vec();
        files.sort();
        cmd::set(
            &mut value,
            "publication",
            cmd::object(vec![
                ("protocol", cmd::string(PROTOCOL)),
                ("transaction_id", cmd::string(&transaction_id(request)?)),
                (
                    "selected_files",
                    JsonValue::Array(files.iter().map(|s| cmd::string(s)).collect()),
                ),
            ]),
        )?;
    }
    Ok(value)
}
fn successor(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    instant: &str,
    scope_operation: &str,
) -> SourceCommandResult<(Inspection, JsonValue)> {
    if cmd::array(&inspection.history, "receipts")?.len() >= 128 {
        return Err(SourceCommandError::Invalid(
            "source revision history capacity reached",
        ));
    }
    let (record, subject, mut files, _, refs) = proposal(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        config,
        family,
        inspection,
        request,
        scope_operation,
    )?;
    let receipt = receipt(
        config, family, inspection, request, &subject, &refs, instant,
    )?;
    let mut history = inspection.history.clone();
    let mut receipts = cmd::array(&history, "receipts")?.to_vec();
    receipts.push(receipt.clone());
    if family.selected() {
        cmd::set(
            &mut history,
            "schema_version",
            cmd::string("tos_source_revision_history_v2"),
        )?;
    }
    cmd::set(&mut history, "receipts", JsonValue::Array(receipts))?;
    files.insert(HISTORY.into(), cmd::published(&history)?);
    if files.len() > 64
        || files.values().any(|b| b.len() > 2_097_152)
        || files.values().map(Vec::len).sum::<usize>() > 8_388_608
    {
        return Err(SourceCommandError::Invalid("revised package budget"));
    }
    Ok((
        Inspection {
            files,
            record,
            subject,
            history,
            profile: inspection.profile.clone(),
            schemas: inspection.schemas.clone(),
            native_identity_snapshot: inspection.native_identity_snapshot.clone(),
            native_text_snapshot: inspection.native_text_snapshot.clone(),
        },
        receipt,
    ))
}
fn source_changes(
    ctx: &CommandContext,
    config: &JsonValue,
    output: &Package,
) -> SourceCommandResult<Vec<SourceChange>> {
    let (parent, _) = split(cmd::text(config, "source_path")?)?;
    output
        .iter()
        .map(|(name, raw)| {
            let path = path(&format!("{parent}/{name}"))?;
            Ok(SourceChange {
                before: ctx.file(&path)?.map(Digest256::of_bytes),
                path,
                after: Some(raw.clone()),
            })
        })
        .collect()
}
fn retain_archive(
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
) -> SourceCommandResult<Vec<SourceChange>> {
    let revision = revision(&inspection.files)?;
    let location = archive_path(config, &revision)?;
    let source_path = cmd::text(config, "source_path")?;
    let mut manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.selected() {
                "tos_source_package_archive_v2"
            } else {
                "tos_source_package_archive_v1"
            }),
        ),
        ("source_path", cmd::string(source_path)),
        ("source", inspection.subject.clone()),
        ("revision", cmd::string(&revision)),
        ("files", file_refs(&inspection.files, true)),
    ]);
    if family.selected() {
        cmd::set(&mut manifest, "publication_protocol", cmd::string(PROTOCOL))?;
    }
    let mut writes = BTreeMap::new();
    for raw in inspection.files.values() {
        writes.insert(
            format!("{}.blob", Digest256::of_bytes(raw).to_hex()),
            raw.clone(),
        );
    }
    writes.insert("manifest.json".into(), cmd::published(&manifest)?);
    let existing_manifest = ctx
        .file(&path(&format!("{location}/manifest.json"))?)?
        .is_some();
    if existing_manifest {
        let stub = cmd::object(vec![
            ("previous_revision", cmd::string(&revision)),
            ("previous_source", inspection.subject.clone()),
            ("archive_path", cmd::string(&location)),
        ]);
        if read_archive(ctx, config, &stub)?.0 != inspection.files {
            return Err(SourceCommandError::Conflict(
                "existing archive differs from predecessor bytes",
            ));
        }
        return Ok(vec![]);
    }
    let mut changes = Vec::new();
    for (name, raw) in writes {
        let path = path(&format!("{location}/{name}"))?;
        if ctx.file(&path)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "partial archive cannot be overwritten",
            ));
        }
        changes.push(SourceChange {
            path,
            before: None,
            after: Some(raw),
        });
    }
    Ok(changes)
}
fn retained_package(
    files: &[SourceFile],
    config: &JsonValue,
    require_all: bool,
) -> SourceCommandResult<Package> {
    let source_path = cmd::text(config, "source_path")?;
    let (parent, base) = split(source_path)?;
    let names = names(source_path)?;
    let mut result = Package::new();
    for file in files {
        let prefix = format!("{parent}/");
        let name = file
            .path
            .as_str()
            .strip_prefix(&prefix)
            .ok_or(SourceCommandError::Denied(
                "retained transaction outside selected home",
            ))?;
        if !names.iter().any(|s| s == name)
            || result.insert(name.into(), file.raw.clone()).is_some()
        {
            return Err(SourceCommandError::Denied(
                "retained transaction outside exact selected files",
            ));
        }
    }
    if !result.contains_key(base) || require_all && result.len() != 3 {
        return Err(SourceCommandError::Invalid(
            "retained selected before/after cardinality",
        ));
    }
    if result.values().any(|r| r.len() > 2_097_152)
        || result.values().map(Vec::len).sum::<usize>() > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "retained transaction byte budget",
        ));
    }
    Ok(result)
}
fn reconstruct_transaction(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    transaction: &RetainedRevisionTransaction,
    scope_operation: &str,
) -> SourceCommandResult<(JsonValue, Inspection, Inspection, JsonValue)> {
    let authorization = cmd::parse(&transaction.authorization_raw)?;
    cmd::exact_keys(
        &authorization,
        &[
            "schema_version",
            "principal_id",
            "authority_ref",
            "source_path",
            "record_id",
            "record_type",
            "request",
        ],
    )?;
    if cmd::text(&authorization, "schema_version")?
        != "tos_selected_metadata_revision_authorization_v1"
    {
        return Err(SourceCommandError::Denied("pending transaction adapter"));
    }
    for field in [
        "principal_id",
        "authority_ref",
        "source_path",
        "record_id",
        "record_type",
    ] {
        if cmd::field(&authorization, field)? != cmd::field(config, field)? {
            return Err(SourceCommandError::Denied(
                "retained transaction current owner scope differs",
            ));
        }
    }
    let original = cmd::field(&authorization, "request")?;
    if request(original, family)? != "record.revise"
        || transaction.transaction_id != transaction_id(original)?
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction request identity",
        ));
    }
    scope(config, original, scope_operation)?;
    if cmd::text(original, "expected_configuration")? != cmd::record_digest(config)?.to_prefixed() {
        return Err(SourceCommandError::Conflict(
            "retained transaction configuration changed",
        ));
    }
    let files = retained_package(&transaction.before, config, false)?;
    let (_, base) = split(cmd::text(config, "source_path")?)?;
    let record = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Invalid("retained record missing"))?,
    )?;
    let (profile, schemas, native_identity_snapshot, native_text_snapshot) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &record,
    )?;
    let subject = source_forms::metadata_subject(&record)?;
    let retained = verify_history(ctx, config, &files, &record)?;
    let before = Inspection {
        files,
        record,
        subject,
        history: retained,
        profile,
        schemas,
        native_identity_snapshot,
        native_text_snapshot,
    };
    let output = retained_package(&transaction.after, config, true)?;
    let retained_history = history(
        &output,
        &cmd::parse(
            output
                .get(base)
                .ok_or(SourceCommandError::Invalid("retained successor missing"))?,
        )?,
    )?;
    let receipt =
        cmd::array(&retained_history, "receipts")?
            .last()
            .ok_or(SourceCommandError::Invalid(
                "retained successor receipt missing",
            ))?;
    if cmd::array(&retained_history, "receipts")?.len()
        != cmd::array(&before.history, "receipts")?.len() + 1
    {
        return Err(SourceCommandError::Conflict(
            "retained successor must append exactly one receipt",
        ));
    }
    if !cmd::same(cmd::field(original, "expected_source")?, &before.subject)?
        || cmd::text(original, "expected_revision")? != revision(&before.files)?
        || cmd::text(original, "expected_dependencies")?
            != dependencies(ctx, config, family, &before)?
    {
        return Err(SourceCommandError::Conflict(
            "retained predecessor or dependencies differ",
        ));
    }
    let (after, reconstructed) = successor(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        config,
        family,
        &before,
        original,
        cmd::text(receipt, "recorded_at")?,
        scope_operation,
    )?;
    if output != after.files || !cmd::same(receipt, &reconstructed)? {
        return Err(SourceCommandError::Conflict(
            "retained bytes do not reconstruct exact delegated successor",
        ));
    }
    if read_archive(ctx, config, &reconstructed)?.0 != before.files {
        return Err(SourceCommandError::Conflict(
            "retained transaction predecessor archive differs",
        ));
    }
    Ok((original.clone(), before, after, reconstructed))
}

/// Prepare the exact maintained record handler selected by protected owner
/// bytes. `publication` is mandatory for selected handlers, including their
/// legacy token=None baseline. It contains observations, not authority.
pub fn prepare_record_revision(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    prepare_record_revision_inner(ctx, publication, None, worker, deadline, cancelled)
}

/// Complete native identity reservation uses the anchored manifest membership.
/// Every selected native packet must also be present byte-for-byte in `ctx`, so
/// the proposed plan retains its exact read dependencies.
pub fn prepare_record_revision_with_profile_cut(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    ctx.check()?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "profile cut and command revision differ",
        ));
    }
    for selected in &ctx.files {
        if !selected.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Unsupported(
                "profile cut input requires separately selected software capture",
            ));
        }
        let member =
            cut.current()
                .member(&selected.path)
                .ok_or(SourceCommandError::Unsupported(
                    "selected profile input outside anchored cut",
                ))?;
        if member.sha256 != Digest256::of_bytes(&selected.raw)
            || member.size_bytes != selected.raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "selected profile input differs from anchored cut member",
            ));
        }
    }
    prepare_record_revision_inner(ctx, publication, Some(cut), worker, deadline, cancelled)
}

/// Authenticate authored and implementation inputs through their independently
/// selected carriers. Software components cannot supply source membership or
/// native identity absence; the complete source cut still owns that inventory.
/// Protected configuration/account observations remain outside both captures.
pub fn prepare_record_revision_from_captures(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    prepare_record_revision_inner(ctx, publication, Some(cut), worker, deadline, cancelled)
}

fn prepare_record_revision_inner(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "schema worker and source command cut differ",
        ));
    }
    let (config, family) = configuration(ctx)?;
    let request_value = cmd::parse(&ctx.request_raw)?;
    let operation = request(&request_value, family)?;
    let configuration = cmd::record_digest(&config)?.to_prefixed();
    if family.selected() {
        let publication = publication.ok_or(SourceCommandError::Unsupported(
            "selected publication observation required",
        ))?;
        let ids = publication
            .transactions
            .iter()
            .map(|t| t.transaction_id.as_str())
            .collect::<Vec<_>>();
        if read_record_revision_publication(ctx, &ids)? != *publication {
            return Err(SourceCommandError::Conflict(
                "publication observation differs from exact selected owner carriers",
            ));
        }
        if let Some(token) = &publication.token {
            digest_text(&cmd::string(token))?;
        }
        let ids = publication
            .transactions
            .iter()
            .map(|t| &t.transaction_id)
            .collect::<BTreeSet<_>>();
        if ids.len() != publication.transactions.len() || publication.transactions.len() > 128 {
            return Err(SourceCommandError::Invalid(
                "publication transaction identity or budget",
            ));
        }
        let pending = publication
            .transactions
            .iter()
            .filter(|t| t.status == RevisionTransactionStatus::Pending)
            .collect::<Vec<_>>();
        if pending.len() > 1 {
            return Err(SourceCommandError::Conflict(
                "multiple pending selected transactions",
            ));
        }
        if let Some(transaction) = pending.first() {
            if operation != "record.revise" && operation != "record.recover" {
                return Err(SourceCommandError::Conflict(
                    "selected publication has pending transaction",
                ));
            }
            let recovery = operation == "record.recover";
            if recovery
                && (!has(&config, "allowed_operations", "record.recover")?
                    || cmd::text(&request_value, "expected_configuration")? != configuration
                    || cmd::text(&request_value, "transaction_id")? != transaction.transaction_id)
            {
                return Err(SourceCommandError::Denied(
                    "recovery exact transaction or current delegation",
                ));
            }
            let (original, before, after, receipt) = reconstruct_transaction(
                cut,
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                transaction,
                if recovery {
                    "record.recover"
                } else {
                    "record.revise"
                },
            )?;
            if !recovery && !cmd::same(&request_value, &original)? {
                return Err(SourceCommandError::Conflict(
                    "only exact original command may resume",
                ));
            }
            let rollback = recovery && cmd::text(&request_value, "decision")? == "rollback";
            let source_path = cmd::text(&config, "source_path")?;
            let (parent, _) = split(source_path)?;
            let mut changes = Vec::new();
            for name in names(source_path)? {
                let path = path(&format!("{parent}/{name}"))?;
                let current = ctx.file(&path)?;
                if current != before.files.get(&name).map(Vec::as_slice)
                    && current != after.files.get(&name).map(Vec::as_slice)
                {
                    return Err(SourceCommandError::Conflict(
                        "pending source member is neither retained before nor after",
                    ));
                }
                let target = if rollback {
                    &before.files
                } else {
                    &after.files
                };
                changes.push(SourceChange {
                    before: current.map(Digest256::of_bytes),
                    path,
                    after: target.get(&name).cloned(),
                });
            }
            let inspection = if rollback { &before } else { &after };
            let mut response = result(
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                inspection,
                Some(publication),
                if rollback { None } else { Some(&receipt) },
                false,
            )?;
            cmd::set(
                &mut response,
                "recovery",
                cmd::object(vec![
                    ("transaction_id", cmd::string(&transaction.transaction_id)),
                    (
                        "decision",
                        cmd::string(if rollback { "rollback" } else { "resume" }),
                    ),
                    ("status", cmd::string("prepared")),
                ]),
            )?;
            return ctx.plan(family.handler_id(), response, changes, false);
        }
        if operation == "record.recover" {
            return Err(SourceCommandError::Conflict(
                "no exact pending transaction selected for recovery",
            ));
        }
    }
    let inspection = inspect(cut, worker, deadline, cancelled, ctx, &config, family)?;
    let mut response = result(
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &inspection,
        publication,
        None,
        false,
    )?;
    if operation == "describe" {
        return ctx.plan(family.handler_id(), response, vec![], false);
    }
    if operation == "inspect-version" {
        let requested = cmd::field(&request_value, "source")?;
        for receipt in cmd::array(&inspection.history, "receipts")? {
            if cmd::same(cmd::field(receipt, "previous_source")?, requested)? {
                let (files, locations) = read_archive(ctx, &config, receipt)?;
                let (_, base) = split(cmd::text(&config, "source_path")?)?;
                cmd::set(
                    &mut response,
                    "record",
                    cmd::parse(
                        files
                            .get(base)
                            .ok_or(SourceCommandError::Conflict("archived record missing"))?,
                    )?,
                )?;
                cmd::set(&mut response, "inspected_source", requested.clone())?;
                cmd::set(&mut response, "files", locations)?;
                return ctx.plan(family.handler_id(), response, vec![], false);
            }
        }
        return Err(SourceCommandError::Conflict(
            "requested exact version is not retained in committed history",
        ));
    }
    scope(&config, &request_value, "record.revise")?; // Revocation precedes replay.
    if operation == "prepare-revise" {
        if !family.selected() && cmd::array(&inspection.history, "receipts")?.len() >= 128 {
            return Err(SourceCommandError::Invalid(
                "source revision history capacity reached",
            ));
        }
        let (_, subject, _, views, refs) = proposal(
            cut,
            worker,
            deadline,
            cancelled,
            ctx,
            &config,
            family,
            &inspection,
            &request_value,
            "record.revise",
        )?;
        cmd::set(&mut response, "prepared_source", subject)?;
        cmd::set(&mut response, "prepared_forms", JsonValue::Array(refs))?;
        cmd::set(
            &mut response,
            "prepared_materializations",
            JsonValue::Array(views),
        )?;
        cmd::set(
            &mut response,
            "expected_dependencies",
            cmd::string(&dependencies(ctx, &config, family, &inspection)?),
        )?;
        if family.selected() {
            cmd::set(
                &mut response,
                "expected_publication",
                publication
                    .and_then(|p| p.token.as_deref())
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            )?;
        }
        return ctx.plan(family.handler_id(), response, vec![], false);
    }
    for receipt in cmd::array(&inspection.history, "receipts")? {
        if cmd::field(receipt, "command_id")? == cmd::field(&request_value, "command_id")? {
            if cmd::text(receipt, "request_digest")?
                != cmd::record_digest(&request_value)?.to_prefixed()
            {
                return Err(SourceCommandError::Conflict(
                    "command identity reused for another request",
                ));
            }
            read_archive(ctx, &config, receipt)?;
            if family.selected() {
                if cmd::text(&request_value, "expected_configuration")? != configuration {
                    return Err(SourceCommandError::Conflict(
                        "selected replay requires current exact configuration",
                    ));
                }
                let id = cmd::text(cmd::field(receipt, "publication")?, "transaction_id")?;
                let retained = publication
                    .and_then(|p| p.transactions.iter().find(|t| t.transaction_id == id))
                    .ok_or(SourceCommandError::Unsupported(
                        "selected replay retained publication evidence absent",
                    ))?;
                if retained.status != RevisionTransactionStatus::Committed {
                    return Err(SourceCommandError::Conflict(
                        "selected history has no committed publication evidence",
                    ));
                }
                let (original, _, _, reconstructed) = reconstruct_transaction(
                    cut,
                    worker,
                    deadline,
                    cancelled,
                    ctx,
                    &config,
                    family,
                    retained,
                    "record.revise",
                )?;
                if !cmd::same(&original, &request_value)? || !cmd::same(&reconstructed, receipt)? {
                    return Err(SourceCommandError::Conflict(
                        "retained publication differs from correction receipt",
                    ));
                }
            }
            response = result(
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                &inspection,
                publication,
                Some(receipt),
                true,
            )?;
            return ctx.plan(family.handler_id(), response, vec![], true);
        }
    }
    if cmd::text(&request_value, "expected_configuration")? != configuration
        || !cmd::same(
            cmd::field(&request_value, "expected_source")?,
            &inspection.subject,
        )?
        || cmd::text(&request_value, "expected_revision")? != revision(&inspection.files)?
        || cmd::text(&request_value, "expected_dependencies")?
            != dependencies(ctx, &config, family, &inspection)?
    {
        return Err(SourceCommandError::Conflict(
            "record revision source or dependency snapshot stale",
        ));
    }
    if family.selected()
        && cmd::field(&request_value, "expected_publication")?
            != &publication
                .and_then(|p| p.token.as_deref())
                .map(cmd::string)
                .unwrap_or(JsonValue::Null)
    {
        return Err(SourceCommandError::Conflict(
            "selected publication snapshot stale",
        ));
    }
    let (after, receipt) = successor(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &inspection,
        &request_value,
        &ctx.recorded_at,
        "record.revise",
    )?;
    let mut changes = retain_archive(ctx, &config, family, &inspection)?;
    changes.extend(source_changes(ctx, &config, &after.files)?);
    response = result(
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &after,
        publication,
        Some(&receipt),
        false,
    )?;
    ctx.plan(family.handler_id(), response, changes, false)
}

const CONTROL_PATH: &str = "ToS/source-witnesses/.metadata-publication.json";
pub(crate) fn state(value: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        value,
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
    let generation = cmd::integer(value, "generation")?;
    let transition = cmd::text(value, "transition_id")?;
    let phase = cmd::text(value, "phase")?;
    if cmd::text(value, "schema_version")? != "tos_source_metadata_publication_v1"
        || !(1..=9_007_199_254_740_991).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !matches!(phase, "pending" | "ready")
        || phase == "pending"
            && (cmd::field(value, "outcome")? != &JsonValue::Null
                || cmd::field(value, "recovery_authorization")? != &JsonValue::Null)
        || phase == "ready" && !matches!(cmd::text(value, "outcome")?, "committed" | "rolled-back")
    {
        return Err(SourceCommandError::Invalid(
            "selected publication state grammar",
        ));
    }
    let recovery = cmd::field(value, "recovery_authorization")?;
    if recovery != &JsonValue::Null
        && (recovery.as_object().is_none() || cmd::canonical(recovery)?.len() > 4096)
    {
        return Err(SourceCommandError::Invalid("selected recovery binding"));
    }
    for field in ["transaction_id", "manifest_sha256", "token"] {
        digest_text(cmd::field(value, field)?)?;
    }
    let fields = value
        .as_object()
        .ok_or(SourceCommandError::Invalid("publication state object"))?
        .iter()
        .filter(|(key, _)| key.as_str() != Some("token"))
        .cloned()
        .collect();
    if cmd::text(value, "token")? != cmd::record_digest(&JsonValue::Object(fields))?.to_prefixed() {
        return Err(SourceCommandError::Conflict(
            "publication control token does not bind exact state",
        ));
    }
    Ok(())
}
fn control(ctx: &CommandContext) -> SourceCommandResult<Option<JsonValue>> {
    let Some(raw) = ctx.file(&path(CONTROL_PATH)?)? else {
        return Ok(None);
    };
    if raw.len() > 8192 {
        return Err(SourceCommandError::Invalid(
            "publication control byte budget",
        ));
    }
    let value = cmd::parse(raw)?;
    state(&value)?;
    Ok(Some(value))
}
/// Read publication observations from explicitly selected owner control,
/// manifest, completion and immutable blob bytes. IDs select retained carriers;
/// they grant no admission. An arbitrary status enum cannot pass the command
/// entrypoint: it is re-derived here and compared with the supplied observation.
pub fn read_record_revision_publication(
    ctx: &CommandContext,
    selected_ids: &[&str],
) -> SourceCommandResult<RevisionPublication> {
    ctx.check()?;
    if selected_ids.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "retained publication count budget",
        ));
    }
    let control = control(ctx)?;
    let mut ids = selected_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<BTreeSet<_>>();
    if ids.len() != selected_ids.len() {
        return Err(SourceCommandError::Invalid(
            "duplicate retained transaction selection",
        ));
    }
    if let Some(control) = &control {
        if cmd::text(control, "phase")? == "pending" {
            ids.insert(cmd::text(control, "transaction_id")?.into());
        }
    }
    let mut transactions = Vec::new();
    for id in ids {
        digest_text(&cmd::string(&id))?;
        let directory = format!("ToS/source-witnesses/.metadata-transactions/{}", &id[7..]);
        let manifest_raw = required(ctx, &format!("{directory}/manifest.json"))?;
        if manifest_raw.len() > 524_288 {
            return Err(SourceCommandError::Invalid(
                "transaction manifest byte budget",
            ));
        }
        let manifest = cmd::parse(manifest_raw)?;
        cmd::exact_keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
            ],
        )?;
        if cmd::text(&manifest, "schema_version")? != "tos_selected_metadata_transaction_v1"
            || cmd::text(&manifest, "transaction_id")? != id
        {
            return Err(SourceCommandError::Unsupported(
                "retained transaction manifest is not selected record revision grammar",
            ));
        }
        let base = cmd::field(&manifest, "base_publication")?;
        cmd::exact_keys(base, &["token", "generation"])?;
        let generation = cmd::integer(base, "generation")?;
        if generation > 9_007_199_254_740_989
            || (cmd::field(base, "token")? == &JsonValue::Null) != (generation == 0)
        {
            return Err(SourceCommandError::Invalid(
                "retained publication predecessor",
            ));
        }
        if generation > 0 {
            digest_text(cmd::field(base, "token")?)?;
        }
        let plan = cmd::field(&manifest, "plan")?;
        cmd::exact_keys(plan, &["authorization", "files", "new_directories"])?;
        if !cmd::array(plan, "new_directories")?.is_empty() || cmd::array(plan, "files")?.len() != 3
        {
            return Err(SourceCommandError::Denied(
                "record revision transaction selects other paths or directories",
            ));
        }
        let authorization = cmd::field(plan, "authorization")?;
        if authorization.as_object().is_none() || cmd::canonical(authorization)?.len() > 65_536 {
            return Err(SourceCommandError::Invalid(
                "retained transaction authorization budget",
            ));
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut paths = BTreeSet::new();
        let mut parents = BTreeSet::new();
        let mut sums = [0u64; 2];
        let mut changed = false;
        let mut last = "";
        for item in cmd::array(plan, "files")? {
            cmd::exact_keys(item, &["path", "before", "after"])?;
            let source_path = cmd::text(item, "path")?;
            path(source_path)?;
            if !source_path.starts_with("ToS/source-witnesses/")
                || source_path.split('/').any(|p| {
                    p.starts_with('.')
                        || matches!(
                            p,
                            "payload" | "private" | "local-content" | "owner-local" | "catalog"
                        )
                })
                || !source_path.ends_with(".json")
                || source_path.len() > 1024
                || source_path.split('/').count() > 24
                || source_path <= last
                || !paths.insert(source_path)
            {
                return Err(SourceCommandError::Denied(
                    "retained revision canonical selected path grammar",
                ));
            }
            last = source_path;
            let mut parent = split(source_path)?.0;
            loop {
                parents.insert(parent);
                if parent == "ToS/source-witnesses" {
                    break;
                }
                parent = split(parent)?.0;
            }
            changed |= !cmd::same(cmd::field(item, "before")?, cmd::field(item, "after")?)?;
            if cmd::field(item, "before")? == &JsonValue::Null
                && cmd::field(item, "after")? == &JsonValue::Null
            {
                return Err(SourceCommandError::Invalid(
                    "retained transaction absent-to-absent member",
                ));
            }
            for (index, side) in ["before", "after"].into_iter().enumerate() {
                let binding = cmd::field(item, side)?;
                if binding == &JsonValue::Null {
                    continue;
                }
                cmd::exact_keys(binding, &["sha256", "bytes"])?;
                let digest = digest_text(cmd::field(binding, "sha256")?)?;
                let length = cmd::integer(binding, "bytes")?;
                sums[index] = sums[index]
                    .checked_add(length)
                    .ok_or(SourceCommandError::Invalid("retained blob budget overflow"))?;
                if length > 8_388_608 || sums[index] > 8_388_608 {
                    return Err(SourceCommandError::Invalid(
                        "retained selected side byte budget",
                    ));
                }
                let raw = required(ctx, &format!("{directory}/{}.blob", &digest[7..]))?;
                if raw.len() as u64 != length || Digest256::of_bytes(raw).to_prefixed() != digest {
                    return Err(SourceCommandError::Conflict(
                        "retained transaction immutable blob differs",
                    ));
                }
                let file = SourceFile {
                    path: path(source_path)?,
                    raw: raw.to_vec(),
                };
                if side == "before" {
                    before.push(file)
                } else {
                    after.push(file)
                }
            }
        }
        if !changed {
            return Err(SourceCommandError::Invalid(
                "retained revision transaction has no byte change",
            ));
        }
        let bindings =
            cmd::field(&manifest, "parents")?
                .as_object()
                .ok_or(SourceCommandError::Invalid(
                    "retained source parent bindings",
                ))?;
        if bindings.len() != parents.len()
            || bindings
                .iter()
                .any(|(key, _)| !key.as_str().is_some_and(|key| parents.contains(key)))
        {
            return Err(SourceCommandError::Conflict(
                "retained source parent closure differs",
            ));
        }
        let current = control
            .as_ref()
            .filter(|state| cmd::text(state, "transaction_id").ok() == Some(id.as_str()));
        for (_, binding) in bindings {
            cmd::exact_keys(binding, &["device", "inode", "mode", "uid"])?;
            cmd::integer(binding, "device")?;
            cmd::integer(binding, "inode")?;
            let mode = cmd::integer(binding, "mode")?;
            let uid = cmd::integer(binding, "uid")?;
            if mode & 0o170000 != 0o040000
                || mode & 0o022 != 0
                || current.is_some_and(|c| cmd::text(c, "phase").ok() == Some("pending"))
                    && uid != 0
                    && uid != ctx.effective_uid
            {
                return Err(SourceCommandError::Denied(
                    "retained protected source parent binding",
                ));
            }
        }
        let digest = Digest256::of_bytes(manifest_raw).to_prefixed();
        let completion_raw = ctx.file(&path(&format!("{directory}/completion.json"))?)?;
        if completion_raw.is_some_and(|raw| raw.len() > 8192) {
            return Err(SourceCommandError::Invalid(
                "retained completion byte budget",
            ));
        }
        let completion = completion_raw.map(cmd::parse).transpose()?;
        let mut terminal = None;
        if let Some(completion) = &completion {
            cmd::exact_keys(completion, &["schema_version", "publication"])?;
            if cmd::text(completion, "schema_version")? != "tos_selected_metadata_completion_v1" {
                return Err(SourceCommandError::Invalid("retained completion schema"));
            }
            let completed = cmd::field(completion, "publication")?;
            state(completed)?;
            if cmd::text(completed, "phase")? != "ready"
                || cmd::text(completed, "transaction_id")? != id
                || cmd::text(completed, "manifest_sha256")? != digest
                || cmd::integer(completed, "generation")? != generation + 2
            {
                return Err(SourceCommandError::Conflict(
                    "retained completion does not bind exact manifest",
                ));
            }
            terminal = Some(completed);
        }
        let status = if let Some(current) = current {
            if cmd::text(current, "manifest_sha256")? != digest {
                return Err(SourceCommandError::Conflict(
                    "current publication manifest digest differs",
                ));
            }
            if cmd::text(current, "phase")? == "pending" {
                if cmd::integer(current, "generation")? != generation + 1 || terminal.is_some() {
                    return Err(SourceCommandError::Conflict(
                        "pending transaction predecessor or terminal state differs",
                    ));
                }
                RevisionTransactionStatus::Pending
            } else {
                if cmd::integer(current, "generation")? != generation + 2
                    || terminal.is_some_and(|t| t != current)
                {
                    return Err(SourceCommandError::Conflict(
                        "terminal current publication differs from completion",
                    ));
                }
                if cmd::text(current, "outcome")? == "committed" {
                    RevisionTransactionStatus::Committed
                } else {
                    RevisionTransactionStatus::RolledBack
                }
            }
        } else if let Some(terminal) = terminal {
            if cmd::text(terminal, "outcome")? == "committed" {
                RevisionTransactionStatus::Committed
            } else {
                RevisionTransactionStatus::RolledBack
            }
        } else {
            return Err(SourceCommandError::Unsupported(
                "unselected orphan transaction has no terminal publication evidence",
            ));
        };
        transactions.push(RetainedRevisionTransaction {
            transaction_id: id,
            status,
            authorization_raw: cmd::canonical(authorization)?,
            before,
            after,
        });
    }
    let token = control
        .as_ref()
        .map(|c| cmd::text(c, "token").map(String::from))
        .transpose()?;
    Ok(RevisionPublication {
        token,
        transactions,
    })
}
fn allowed_fields<'a>(
    family: RevisionFamily,
    config: &JsonValue,
) -> SourceCommandResult<Vec<&'a str>> {
    Ok(match family {
        RevisionFamily::Historical | RevisionFamily::PublicProfile => BASE_FIELDS.to_vec(),
        RevisionFamily::PublicProfileScope => BASE_FIELDS
            .iter()
            .copied()
            .chain(["semantic_scope"])
            .collect(),
        RevisionFamily::CorpusFlat
        | RevisionFamily::CorpusSelectedV2
        | RevisionFamily::CorpusSelectedV3 => CORPUS_FIELDS.to_vec(),
        RevisionFamily::NativeSelected => match cmd::text(config, "record_type")? {
            "artifact" => vec![
                "path_identity",
                "physical_description",
                "find_context",
                "bibliography",
            ],
            "composite" => vec!["preferred_label", "editorial_object"],
            "link" => vec![
                "preferred_label",
                "variant_labels",
                "notes",
                "source_refs",
                "provider_label",
            ],
            _ => return Err(SourceCommandError::Denied("native revision record type")),
        },
    })
}
fn request(v: &JsonValue, family: RevisionFamily) -> SourceCommandResult<&str> {
    let operation = cmd::text(v, "operation")?;
    let mut keys = vec!["schema_version", "operation"];
    match operation {
        "describe" => {}
        "inspect-version" => keys.push("source"),
        "prepare-revise" | "record.revise" => {
            keys.extend(["fields", "forms", "reason"]);
            if operation == "record.revise" {
                keys.extend([
                    "command_id",
                    "expected_configuration",
                    "expected_source",
                    "expected_revision",
                    "expected_dependencies",
                ]);
                if family.selected() {
                    keys.push("expected_publication");
                }
            }
        }
        "record.recover" if family.selected() => {
            keys.extend(["transaction_id", "decision", "expected_configuration"])
        }
        _ => return Err(SourceCommandError::Unsupported("record revision operation")),
    }
    cmd::exact_keys(v, &keys)?;
    if cmd::text(v, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid("command request schema"));
    }
    if operation == "record.revise" {
        let id = cmd::text(v, "command_id")?;
        if id.is_empty() || id.chars().count() > 256 {
            return Err(SourceCommandError::Invalid("revision command identity"));
        }
        for k in [
            "expected_configuration",
            "expected_revision",
            "expected_dependencies",
        ] {
            digest_text(cmd::field(v, k)?)?;
        }
        exact_ref(cmd::field(v, "expected_source")?)?;
        if family.selected() && cmd::field(v, "expected_publication")? != &JsonValue::Null {
            digest_text(cmd::field(v, "expected_publication")?)?;
        }
    }
    if operation == "inspect-version" {
        exact_ref(cmd::field(v, "source")?)?;
    }
    if operation == "record.recover" {
        digest_text(cmd::field(v, "transaction_id")?)?;
        digest_text(cmd::field(v, "expected_configuration")?)?;
        if !matches!(cmd::text(v, "decision")?, "resume" | "rollback") {
            return Err(SourceCommandError::Invalid("recovery decision"));
        }
    }
    Ok(operation)
}
fn exact_ref(v: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(v, &["id", "version", "digest"])?;
    if cmd::text(v, "id")?.is_empty() || cmd::integer(v, "version")? == 0 {
        return Err(SourceCommandError::Invalid("exact source reference"));
    }
    digest_text(cmd::field(v, "digest")?)?;
    Ok(())
}
fn scope(config: &JsonValue, request: &JsonValue, operation: &str) -> SourceCommandResult<()> {
    if !has(config, "allowed_operations", operation)? {
        return Err(SourceCommandError::Denied(
            "current record revision scope revoked",
        ));
    }
    let fields = cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields object"))?;
    if fields.is_empty() {
        return Err(SourceCommandError::Denied("empty correction"));
    }
    for (key, _) in fields {
        if !has(
            config,
            "allowed_fields",
            key.as_str()
                .ok_or(SourceCommandError::Invalid("field name"))?,
        )? {
            return Err(SourceCommandError::Denied(
                "field outside current delegated scope",
            ));
        }
    }
    let forms = cmd::array(request, "forms")?;
    if !(1..=32).contains(&forms.len()) {
        return Err(SourceCommandError::Invalid(
            "revision requires bounded source forms",
        ));
    }
    let mut seen = BTreeSet::new();
    for form in forms {
        cmd::exact_keys(form, &["form_id", "field_id"])?;
        let id = cmd::text(form, "form_id")?;
        cmd::text(form, "field_id")?;
        if !has(config, "allowed_form_ids", id)? {
            return Err(SourceCommandError::Denied(
                "form outside current delegated scope",
            ));
        }
        if !seen.insert(id) {
            return Err(SourceCommandError::Invalid("duplicate revision form"));
        }
    }
    if !(1..=4096).contains(&strip(cmd::text(request, "reason")?)?.chars().count()) {
        return Err(SourceCommandError::Invalid("authored correction reason"));
    }
    Ok(())
}
fn package(
    ctx: &CommandContext,
    source_path: &str,
    selected: bool,
) -> SourceCommandResult<Package> {
    let (parent, base) = split(source_path)?;
    let selected_names = names(source_path)?;
    let prefix = format!("{parent}/");
    let mut result = Package::new();
    for file in &ctx.files {
        if let Some(name) = file.path.as_str().strip_prefix(&prefix) {
            if !name.contains('/') && (!selected || selected_names.iter().any(|s| s == name)) {
                result.insert(name.into(), file.raw.clone());
            } else if !selected && name.contains('/') {
                return Err(SourceCommandError::Denied(
                    "flat revision package has descendants",
                ));
            }
        }
    }
    if !result.contains_key(base)
        || result.is_empty()
        || result.len() > 64
        || result.values().any(|b| b.len() > 2_097_152)
        || result.values().map(Vec::len).sum::<usize>() > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "record package presence or budget",
        ));
    }
    Ok(result)
}
pub(crate) fn file_refs(files: &Package, blobs: bool) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                let digest = Digest256::of_bytes(raw);
                let mut value = cmd::object(vec![
                    ("sha256", cmd::string(&digest.to_prefixed())),
                    ("bytes", cmd::number(raw.len() as u64)),
                ]);
                if blobs {
                    cmd::set(
                        &mut value,
                        "blob",
                        cmd::string(&format!("{}.blob", digest.to_hex())),
                    )
                    .expect("constructed object");
                }
                (JsonString::from_utf8(name), value)
            })
            .collect(),
    )
}
pub(crate) fn revision(files: &Package) -> SourceCommandResult<String> {
    Ok(cmd::record_digest(&file_refs(files, false))?.to_prefixed())
}
pub(crate) fn archive_path(config: &JsonValue, revision: &str) -> SourceCommandResult<String> {
    Ok(format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(cmd::text(config, "record_id")?.as_bytes()).to_hex(),
        revision
            .strip_prefix("sha256:")
            .ok_or(SourceCommandError::Invalid("revision digest"))?
    ))
}

pub(crate) fn schema(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
) -> SourceCommandResult<()> {
    schema_at(
        worker, deadline, cancelled, ctx, refs, root, instance, None, false,
    )
}

// Reuse is limited to an actual scalar result from this selected worker operation.
// The caller still reads and checks the current source/resource dependencies.
fn schema_at(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
    source_path: Option<&str>,
    reuse_scalar: bool,
) -> SourceCommandResult<()> {
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "schema worker and source command cut differ",
        ));
    }
    for name in refs {
        let raw = required(ctx, name)?;
        let source = cmd::parse(raw)?;
        let uri = cmd::text(&source, "$id")?;
        if uri != format!("https://tree-of-sophia.local/{name}")
            && uri != format!("https://treeofsophia.local/{name}")
        {
            return Err(SourceCommandError::Invalid(
                "source schema identity differs",
            ));
        }
        if worker.contract_digest(name) != Some(Digest256::of_bytes(raw)) {
            return Err(SourceCommandError::Conflict(
                "schema worker resources and selected source bytes differ",
            ));
        }
    }
    let fallback_path;
    let source_path = match source_path {
        Some(path) => path,
        None => {
            fallback_path =
                cmd::text(&cmd::parse(&ctx.configuration_raw)?, "source_path")?.to_owned();
            &fallback_path
        }
    };
    let raw = cmd::canonical(instance)?;
    let result = if reuse_scalar {
        worker.check_reusing_scalar(source_path, &raw, root, deadline, cancelled)
    } else {
        worker.check(source_path, &raw, root, deadline, cancelled)
    };
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid(
            "source violates selected schema",
        )),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: source_path.to_owned(),
            root: root.to_owned(),
            reason,
        }),
    }
}

fn profile(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    record: &JsonValue,
) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)> {
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    let (kind, id_field, schema_ref) = if family == RevisionFamily::NativeSelected {
        let kind = cmd::text(config, "record_type")?;
        let (subtree, basename, identity, source_schema) =
            match (kind, cmd::text(record, "schema_version")?) {
                ("artifact", "tos_artifact_source_witness_v1") => (
                    "artifacts",
                    "artifact-witness.json",
                    "artifact_id",
                    "ToS/contracts/artifact-source-witness.schema.json",
                ),
                ("artifact", "tos_artifact_source_witness_v2") => (
                    "artifacts",
                    "artifact-witness.json",
                    "artifact_id",
                    "ToS/contracts/artifact-source-witness-v2.schema.json",
                ),
                ("composite", "tos_scholarly_composite_witness_v1") => (
                    "scholarly-composites",
                    "composite-witness.json",
                    "composite_id",
                    "ToS/contracts/scholarly-composite-witness.schema.json",
                ),
                ("link", "tos_source_link_v1") => (
                    "links",
                    "link.json",
                    "record_id",
                    "ToS/contracts/source-link.schema.json",
                ),
                _ => return Err(SourceCommandError::Denied("exact native record schema")),
            };
        if base != basename
            || !source_path.starts_with(&format!("ToS/source-witnesses/{subtree}/"))
            || cmd::text(config, "record_schema_version")? != cmd::text(record, "schema_version")?
        {
            return Err(SourceCommandError::Denied(
                "exact native owner path or schema",
            ));
        }
        (kind, identity, source_schema)
    } else if family == RevisionFamily::Historical {
        if cmd::text(record, "schema_version")? != "tos_historical_record_v1" {
            return Err(SourceCommandError::Denied("historical schema required"));
        }
        (
            cmd::text(record, "record_type")?,
            "record_id",
            "ToS/contracts/historical-record.schema.json",
        )
    } else if family.profile() {
        return public_profile(cut, worker, deadline, cancelled, ctx, config, record);
    } else {
        let kind = cmd::text(config, "record_type")?;
        let mut allowed = vec!["agent", "place", "organization", "work"];
        if matches!(
            family,
            RevisionFamily::CorpusSelectedV2 | RevisionFamily::CorpusSelectedV3
        ) {
            allowed.push("expression");
        }
        if family == RevisionFamily::CorpusSelectedV3 {
            allowed.extend(["edition", "collection", "item"]);
        }
        if !allowed.contains(&kind)
            || cmd::text(record, "record_type")? != kind
            || cmd::text(record, "schema_version")? != "tos_corpus_record_v1"
            || base != format!("{kind}.json")
        {
            return Err(SourceCommandError::Denied(
                "native grant version or corpus type",
            ));
        }
        (kind, "record_id", CORPUS_SCHEMA)
    };
    if cmd::text(record, id_field)? != cmd::text(config, "record_id")?
        || !valid_id(
            cmd::text(config, "record_id")?,
            &format!("tos.{kind}."),
            false,
        )
    {
        return Err(SourceCommandError::Denied("exact record identity"));
    }
    let mut schemas = vec![schema_ref.into()];
    if family == RevisionFamily::Historical {
        schemas.push(CORPUS_SCHEMA.into());
    }
    schema(
        worker, deadline, cancelled, ctx, &schemas, schema_ref, record,
    )?;
    if family == RevisionFamily::Historical
        && !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
    {
        return Err(SourceCommandError::Denied("record visibility"));
    }
    let profile = cmd::object(vec![
        ("record_type", cmd::string(kind)),
        ("id_prefix", cmd::string(&format!("tos.{kind}."))),
        ("source_basename", cmd::string(base)),
        ("schema_ref", cmd::string(schema_ref)),
        (
            "schema_version",
            cmd::string(cmd::text(record, "schema_version")?),
        ),
        ("source_scope", cmd::string("public_metadata_only")),
    ]);
    Ok((profile, schemas, None, None))
}

fn ancestry<'a>(
    entities: &BTreeMap<&'a str, &'a JsonValue>,
    id: &'a str,
    visiting: &mut BTreeSet<&'a str>,
    out: &mut BTreeSet<&'a str>,
) -> SourceCommandResult<()> {
    if visiting.len() > 128 || !visiting.insert(id) {
        return Err(SourceCommandError::Invalid(
            "entity ancestry cycle or depth budget",
        ));
    }
    let entry = entities
        .get(id)
        .ok_or(SourceCommandError::Invalid("entity parent missing"))?;
    out.insert(id);
    for parent in cmd::array(entry, "parent_type_ids")? {
        ancestry(
            entities,
            parent
                .as_str()
                .ok_or(SourceCommandError::Invalid("entity parent identity"))?,
            visiting,
            out,
        )?;
    }
    visiting.remove(id);
    Ok(())
}
/// Match the maintained native inventory discovery against complete anchored
/// membership. A caller-selected list alone can never prove absence.
fn native_identity_inventory(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    record_id: &str,
) -> SourceCommandResult<(Option<String>, bool)> {
    if !["occurrence", "lexeme", "sense", "sign", "concept"]
        .iter()
        .any(|kind| record_id.starts_with(&format!("tos.{kind}.")))
    {
        return Ok((None, false));
    }
    let cut = cut.ok_or(SourceCommandError::Unsupported(
        "profile native identity reservation needs complete anchored source cut",
    ))?;
    let (_, snapshot, schema) =
        selected_native_identity_inventory(ctx, cut, worker, deadline, cancelled, Some(record_id))?;
    Ok((Some(snapshot), schema))
}

/// Exact native identity membership for the maintained creation catalog.
/// It reserves identities; it never projects private semantic packet contents.
pub(crate) fn native_identity_inventory_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(BTreeMap<String, Vec<String>>, String, bool)> {
    selected_native_identity_inventory(ctx, cut, worker, deadline, cancelled, None)
}

fn selected_native_identity_inventory(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    reserved_id: Option<&str>,
) -> SourceCommandResult<(BTreeMap<String, Vec<String>>, String, bool)> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "native inventory source cut differs",
        ));
    }
    let mut entries = BTreeMap::new();
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut remaining = 8_388_608usize;
    let contract = "ToS/contracts/semantic-annotation-packet-v2.schema.json";
    for member in cut.current().members() {
        let name = member.path.as_str();
        if name == "ToS/source-witnesses/owner-local"
            || name.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace cannot enter public native inventory",
            ));
        }
        let basename = name.rsplit('/').next().unwrap_or(name);
        if !name.starts_with("ToS/source-witnesses/")
            || !basename.starts_with("semantic-annotation")
            || !basename.ends_with(".json")
            || name
                .split('/')
                .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
        {
            continue;
        }
        if entries.len() >= 1024 || member.size_bytes > remaining.min(1_048_576) as u64 {
            return Err(SourceCommandError::Invalid(
                "native identity inventory metadata budget",
            ));
        }
        let observed = cut
            .read_member(
                ctx.base_revision,
                &member.path,
                remaining.min(1_048_576) as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported("exact native inventory member read unavailable")
            })?;
        let raw = required(ctx, name)?;
        if raw != observed.raw.as_slice() {
            return Err(SourceCommandError::Conflict(
                "selected native inventory bytes differ from anchored cut",
            ));
        }
        remaining = remaining
            .checked_sub(raw.len())
            .ok_or(SourceCommandError::Invalid("native inventory byte budget"))?;
        let packet = cmd::parse(raw)?;
        if cmd::text(&packet, "schema_version")? != "tos_semantic_annotation_packet_v2" {
            return Err(SourceCommandError::Unsupported(
                "native identity packet contract",
            ));
        }
        schema_at(
            worker,
            deadline,
            cancelled,
            ctx,
            &[contract.into()],
            contract,
            &packet,
            Some(name),
            true,
        )?;
        for entity in cmd::array(&packet, "entities")? {
            let entity_id = cmd::text(entity, "entity_id")?;
            if reserved_id == Some(entity_id) {
                return Err(SourceCommandError::Denied(
                    "subject identity belongs to native packet; explicit owner migration required",
                ));
            }
            identities
                .entry(entity_id.to_owned())
                .or_default()
                .push(name.to_owned());
        }
        entries.insert(
            name.to_string(),
            Digest256::of_bytes(raw).to_prefixed()[7..].to_string(),
        );
    }
    let used_schema = !entries.is_empty();
    let value = JsonValue::Object(
        entries
            .into_iter()
            .map(|(name, digest)| (JsonString::from_utf8(&name), cmd::string(&digest)))
            .collect(),
    );
    // Python inventory json.dumps uses ensure_ascii=True, like binding snapshots.
    Ok((identities, python_ascii_digest(&value)?, used_schema))
}

struct NativeBindingReader<'a> {
    ctx: &'a CommandContext,
    cut: &'a CorpusCutReader,
    worker: &'a mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    inputs: BTreeMap<String, String>,
    contracts: BTreeSet<String>,
    remaining: usize,
}
impl NativeBindingReader<'_> {
    fn route(name: &str, support: bool, contract: bool, content: bool) -> SourceCommandResult<()> {
        path(name)?;
        let home = if contract {
            "ToS/contracts/"
        } else if support {
            "ToS/"
        } else {
            "ToS/source-witnesses/"
        };
        if !name.starts_with(home)
            || name.starts_with("ToS/source-witnesses/owner-local/")
            || name.split('/').any(|part| {
                part == "catalog" || !content && matches!(part, "payload" | "local-content")
            })
        {
            return Err(SourceCommandError::Denied(
                "native binding reference leaves declared public owner home",
            ));
        }
        Ok(())
    }
    fn read(
        &mut self,
        name: &str,
        expected: Option<&str>,
        support: bool,
        contract: bool,
    ) -> SourceCommandResult<Vec<u8>> {
        Self::route(name, support, contract, false)?;
        let raw = required(self.ctx, name)?;
        let digest = Digest256::of_bytes(raw).to_hex();
        if expected.is_some_and(|expected| expected != digest) {
            return Err(SourceCommandError::Conflict(
                "native binding exact input digest differs",
            ));
        }
        if !self.inputs.contains_key(name) {
            if self.inputs.len() >= 128 || raw.len() > self.remaining.min(1_048_576) {
                return Err(SourceCommandError::Invalid(
                    "native binding metadata dependency budget",
                ));
            }
            let member = self
                .cut
                .read_member(
                    self.ctx.base_revision,
                    &path(name)?,
                    self.remaining.min(1_048_576) as u64,
                    self.deadline,
                    self.cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Unsupported(
                        "native binding exact source member unavailable",
                    )
                })?;
            if raw != member.raw.as_slice() {
                return Err(SourceCommandError::Conflict(
                    "native binding selected input differs from cut",
                ));
            }
            self.remaining -= raw.len();
            self.inputs.insert(name.into(), digest);
        }
        if contract {
            self.contracts.insert(name.into());
        }
        Ok(raw.to_vec())
    }
    fn record(
        &mut self,
        name: &str,
        expected: Option<&str>,
    ) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
        let raw = self.read(name, expected, false, false)?;
        let value = cmd::parse(&raw)?;
        if value.as_object().is_none() {
            return Err(SourceCommandError::Invalid(
                "native binding metadata object",
            ));
        }
        Ok((value, raw))
    }
    fn validate(
        &mut self,
        value: &JsonValue,
        basename: &str,
        locator: &str,
    ) -> SourceCommandResult<()> {
        let name = format!("ToS/contracts/{basename}");
        let grammar = cmd::parse(&self.read(&name, None, false, true)?)?;
        if cmd::text(&grammar, "$id")? != format!("https://tree-of-sophia.local/{name}") {
            return Err(SourceCommandError::Invalid(
                "native schema identity differs from owner path",
            ));
        }
        native_schema_refs(&grammar, 0)?;
        schema_at(
            self.worker,
            self.deadline,
            self.cancelled,
            self.ctx,
            &[name.clone()],
            &name,
            value,
            Some(locator),
            true,
        )
    }
    fn metadata(&self, raw: &[u8], name: &str, kind: &str) -> SourceCommandResult<()> {
        use tos_validation::text_metadata_rules::{
            self as rules, TextMetadataLimits, TextMetadataState,
        };
        let limits = TextMetadataLimits {
            max_packet_bytes: 1_048_576,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline: self.deadline,
        };
        let report = match kind {
            "unit" => {
                rules::inspect_source_text_unit_v1_metadata(raw, name, limits, self.cancelled)
            }
            "layer" => rules::inspect_source_text_layer_metadata(raw, name, limits, self.cancelled),
            "anchor" => rules::inspect_source_anchor_v2_metadata(raw, name, limits, self.cancelled),
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "native metadata predicate route",
                ));
            }
        }
        .map_err(|_| {
            SourceCommandError::Unsupported("native metadata owner predicate execution unavailable")
        })?;
        if report.packet_digest != Digest256::of_bytes(raw).to_hex()
            || report.scope != "owner-metadata-predicates-only"
        {
            return Err(SourceCommandError::Conflict(
                "native metadata predicate input binding differs",
            ));
        }
        match report.state {
            TextMetadataState::CheckedMetadata if report.issues.is_empty() => Ok(()),
            TextMetadataState::Unsupported => Err(SourceCommandError::Unsupported(
                "native metadata owner predicates need unsupported scalar representation",
            )),
            _ => Err(SourceCommandError::Invalid(
                "native metadata violates owner predicates",
            )),
        }
    }
    fn layer_dependencies(
        &mut self,
        layer: &JsonValue,
        raw: &[u8],
        name: &str,
        visiting: &mut BTreeSet<String>,
    ) -> SourceCommandResult<()> {
        let id = cmd::text(layer, "layer_id")?;
        if visiting.len() >= 16 || !visiting.insert(id.into()) {
            return Err(SourceCommandError::Invalid(
                "native text layer lineage cycle or depth",
            ));
        }
        self.metadata(raw, name, "layer")?;
        let policy = cmd::field(layer, "editorial_policy")?;
        self.read(
            cmd::text(policy, "policy_ref")?,
            Some(cmd::text(policy, "policy_sha256")?),
            true,
            false,
        )?;
        let derivation = cmd::field(layer, "derivation")?;
        let maker = cmd::field(derivation, "maker")?;
        if let Some(configuration) = maker
            .object_get("configuration_ref")
            .filter(|v| **v != JsonValue::Null)
        {
            self.read(
                configuration.as_str().ok_or(SourceCommandError::Invalid(
                    "native maker configuration path",
                ))?,
                Some(cmd::text(maker, "configuration_digest")?),
                true,
                false,
            )?;
        }
        for target in cmd::array(derivation, "input_layers")? {
            let name = cmd::text(target, "record_ref")?;
            let (previous, raw) = self.record(name, Some(cmd::text(target, "record_sha256")?))?;
            self.validate(&previous, "source-text-layer.schema.json", name)?;
            if cmd::field(&previous, "layer_id")? != cmd::field(target, "layer_id")?
                || cmd::field(cmd::field(&previous, "representation")?, "content_sha256")?
                    != cmd::field(target, "content_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "native predecessor layer identity or content differs",
                ));
            }
            self.layer_dependencies(&previous, &raw, name, visiting)?;
        }
        visiting.remove(id);
        Ok(())
    }
    fn source_scope(
        &mut self,
        binding: &JsonValue,
        packet: &JsonValue,
        layer: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        let scope = cmd::field(packet, "source_scope")?;
        let layer_scope = cmd::field(layer, "source_binding")?;
        let refs = cmd::field(binding, "source_record_refs")?;
        let mut records = BTreeMap::new();
        for kind in ["work", "expression", "edition", "item"] {
            let name = cmd::text(refs, kind)?;
            if name.rsplit('/').next() != Some(format!("{kind}.json").as_str()) {
                return Err(SourceCommandError::Denied(
                    "native source scope metadata kind locator",
                ));
            }
            let (record, _) = self.record(name, None)?;
            self.validate(&record, "corpus-record.schema.json", name)?;
            let key = format!("{kind}_ref");
            if cmd::text(&record, "record_type")? != kind
                || cmd::field(&record, "record_id")? != cmd::field(scope, &key)?
                || cmd::field(layer_scope, &key)? != cmd::field(scope, &key)?
            {
                return Err(SourceCommandError::Conflict(
                    "native source scope identity differs",
                ));
            }
            records.insert(kind, record);
        }
        if cmd::field(&records["expression"], "work_ref")? != cmd::field(scope, "work_ref")?
            || !cmd::array(&records["edition"], "embodies_expression_refs")?
                .contains(cmd::field(scope, "expression_ref")?)
        {
            return Err(SourceCommandError::Conflict(
                "native source scope bibliographic topology differs",
            ));
        }
        let name = cmd::text(&records["item"], "item_manifest_ref")?;
        if split(name)?.0 != split(cmd::text(refs, "item")?)?.0
            || split(name)?.1 != "item.manifest.json"
        {
            return Err(SourceCommandError::Denied(
                "native item manifest leaves exact item",
            ));
        }
        let (manifest, _) = self.record(name, None)?;
        self.validate(&manifest, "source-item-manifest.schema.json", name)?;
        if cmd::field(&manifest, "item_id")? != cmd::field(scope, "item_ref")?
            || cmd::field(&manifest, "embodiment_ref")? != cmd::field(scope, "edition_ref")?
            || cmd::field(layer_scope, "source_file_ref")? != cmd::field(scope, "file_ref")?
            || cmd::field(layer_scope, "source_file_sha256")? != cmd::field(scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native item or source file binding differs",
            ));
        }
        let matches = cmd::array(&manifest, "payload_files")?
            .iter()
            .filter(|row| row.object_get("file_id") == scope.object_get("file_ref"))
            .collect::<Vec<_>>();
        if matches.len() != 1
            || cmd::field(matches[0], "sha256")? != cmd::field(scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native source file not unique in exact item manifest",
            ));
        }
        Ok(manifest)
    }
    fn snapshot(self) -> SourceCommandResult<(String, Vec<String>)> {
        let value = JsonValue::Array(
            self.inputs
                .into_iter()
                .map(|(name, digest)| {
                    JsonValue::Array(vec![
                        cmd::string(&name),
                        cmd::string("metadata"),
                        cmd::string(&digest),
                    ])
                })
                .collect(),
        );
        Ok((
            python_ascii_digest(&value)?,
            self.contracts.into_iter().collect(),
        ))
    }
}

// The maintained native resolver has no external-resource retrieval route for
// these self-contained contracts. A worker's wider resource inventory must not
// silently make an undeclared native grammar dependency usable.
fn native_schema_refs(value: &JsonValue, depth: usize) -> SourceCommandResult<()> {
    if depth > 128 {
        return Err(SourceCommandError::Invalid("native schema depth budget"));
    }
    match value {
        JsonValue::Object(fields) => {
            for (name, child) in fields {
                if [Some("$ref"), Some("$dynamicRef"), Some("$recursiveRef")]
                    .contains(&name.as_str())
                    && !child
                        .as_str()
                        .is_some_and(|reference| reference.starts_with('#'))
                {
                    return Err(SourceCommandError::Unsupported(
                        "native binding schema selects undeclared resource",
                    ));
                }
                native_schema_refs(child, depth + 1)?;
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                native_schema_refs(child, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}

pub(crate) fn python_ascii_digest(value: &JsonValue) -> SourceCommandResult<String> {
    let compact = cmd::canonical(value)?;
    let compact = std::str::from_utf8(&compact)
        .map_err(|_| SourceCommandError::Invalid("native snapshot UTF-8"))?;
    let mut ascii = String::new();
    for ch in compact.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units).iter() {
                use std::fmt::Write;
                write!(&mut ascii, "\\u{unit:04x}")
                    .map_err(|_| SourceCommandError::Invalid("native snapshot emission"))?;
            }
        }
    }
    Ok(Digest256::of_bytes(ascii.as_bytes()).to_prefixed())
}

/// Exact maintained metadata-only adapter. Public gate means recorded posture,
/// not verified content, an authenticated producer, rights judgment or admission.
fn native_text_binding(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    binding: &JsonValue,
) -> SourceCommandResult<(String, Vec<String>)> {
    let cut = cut.ok_or(SourceCommandError::Unsupported(
        "native text binding requires anchored source cut",
    ))?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict("native binding cut differs"));
    }
    let mut reader = NativeBindingReader {
        ctx,
        cut,
        worker,
        deadline,
        cancelled,
        inputs: BTreeMap::new(),
        contracts: BTreeSet::new(),
        remaining: 8_388_608,
    };
    let binding_locator =
        cmd::text(&cmd::parse(&ctx.configuration_raw)?, "source_path")?.to_owned();
    reader.validate(
        binding,
        "native-text-unit-binding.schema.json",
        &binding_locator,
    )?;
    let packet_path = cmd::text(binding, "packet_ref")?;
    let (packet, packet_raw) =
        reader.record(packet_path, Some(cmd::text(binding, "packet_sha256")?))?;
    reader.validate(
        &packet,
        "source-text-unit-packet-v1.schema.json",
        packet_path,
    )?;
    if cmd::text(&packet, "content_posture")? != "source_bound"
        || cmd::field(&packet, "packet_id")? != cmd::field(binding, "packet_id")?
        || cmd::field(&packet, "packet_version")? != cmd::field(binding, "packet_version")?
    {
        return Err(SourceCommandError::Conflict(
            "native packet identity version or posture differs",
        ));
    }
    let layer_binding = cmd::field(binding, "text_layer")?;
    let packet_layer = cmd::field(&packet, "source_layer")?;
    let layer_path = cmd::text(layer_binding, "record_ref")?;
    if cmd::text(packet_layer, "text_layer_ref")? != layer_path {
        return Err(SourceCommandError::Conflict(
            "native packet addresses another text layer",
        ));
    }
    let (layer, layer_raw) =
        reader.record(layer_path, Some(cmd::text(layer_binding, "record_sha256")?))?;
    reader.validate(&layer, "source-text-layer.schema.json", layer_path)?;
    if cmd::field(&layer, "layer_id")? != cmd::field(layer_binding, "layer_id")?
        || cmd::field(&layer, "layer_version")? != cmd::field(layer_binding, "layer_version")?
    {
        return Err(SourceCommandError::Conflict(
            "native text layer identity or version differs",
        ));
    }
    reader.metadata(&packet_raw, packet_path, "unit")?;
    reader.layer_dependencies(&layer, &layer_raw, layer_path, &mut BTreeSet::new())?;
    let manifest = reader.source_scope(binding, &packet, &layer)?;
    let rep = cmd::field(&layer, "representation")?;
    let normalization = cmd::text(rep, "character_normalization")?;
    let unicode_form = if normalization == "none" {
        "source_preserved"
    } else {
        normalization
    };
    if cmd::field(packet_layer, "text_layer_sha256")? != cmd::field(rep, "content_sha256")?
        || cmd::field(packet_layer, "language")? != cmd::field(rep, "language")?
        || cmd::text(packet_layer, "unicode_form")? != unicode_form
        || cmd::field(packet_layer, "visibility")? != cmd::field(rep, "content_visibility")?
        || cmd::field(packet_layer, "publication_authorized")?
            != cmd::field(rep, "publication_authorized")?
        || !["text/plain", "text/plain; charset=utf-8"].contains(&cmd::text(rep, "media_type")?)
    {
        return Err(SourceCommandError::Conflict(
            "native packet and UTF-8 layer declarations differ",
        ));
    }
    let scope = cmd::field(rep, "text_scope")?;
    for anchor in cmd::array(&packet, "anchors")? {
        let selector = cmd::field(anchor, "selector")?;
        if cmd::field(cmd::field(anchor, "source_return")?, "locator_ref")?
            != cmd::field(rep, "content_ref")?
            || !(cmd::integer(scope, "start")? <= cmd::integer(selector, "start")?
                && cmd::integer(selector, "start")? <= cmd::integer(selector, "end")?
                && cmd::integer(selector, "end")? <= cmd::integer(scope, "end")?)
        {
            return Err(SourceCommandError::Conflict(
                "native unit anchor leaves exact representation scope",
            ));
        }
    }
    let source_scope = cmd::field(&packet, "source_scope")?;
    for target in cmd::array(cmd::field(&layer, "source_binding")?, "anchors")? {
        let name = cmd::text(target, "anchor_record_ref")?;
        let (anchor, raw) =
            reader.record(name, Some(cmd::text(target, "anchor_record_sha256")?))?;
        reader.validate(&anchor, "source-anchor-v2.schema.json", name)?;
        reader.metadata(&raw, name, "anchor")?;
        let anchor_target = cmd::field(&anchor, "target")?;
        if cmd::field(&anchor, "anchor_id")? != cmd::field(target, "anchor_id")?
            || cmd::field(anchor_target, "item_id")? != cmd::field(source_scope, "item_ref")?
            || cmd::field(anchor_target, "file_id")? != cmd::field(source_scope, "file_ref")?
            || cmd::field(anchor_target, "file_sha256")? != cmd::field(source_scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native text layer source anchor binding differs",
            ));
        }
    }
    let rights = cmd::field(&packet, "rights_and_visibility")?;
    let rights_refs = texts(rights, "rights_record_refs", 128)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let declared = cmd::array(rep, "rights_record_refs")?
        .iter()
        .map(|row| cmd::text(row, "ref").map(String::from))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if declared.is_empty()
        || rights_refs != declared
        || !declared.contains(cmd::text(&manifest, "rights_ref")?)
    {
        return Err(SourceCommandError::Conflict(
            "native packet layer and item rights closure differs",
        ));
    }
    let exact_scope = [
        cmd::text(&layer, "layer_id")?,
        cmd::text(rep, "content_file_id")?,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut relevant = exact_scope.clone();
    for (key, value) in source_scope
        .as_object()
        .ok_or(SourceCommandError::Invalid("native source scope"))?
    {
        if key.as_str().is_some_and(|name| name.ends_with("_ref")) {
            relevant.insert(
                value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("native source scope reference"))?,
            );
        }
    }
    let mut rights_records = Vec::new();
    for target in cmd::array(rep, "rights_record_refs")? {
        let name = cmd::text(target, "ref")?;
        let (record, _) = reader.record(name, Some(cmd::text(target, "sha256")?))?;
        reader.validate(&record, "rights-record.schema.json", name)?;
        let scope = texts(&record, "scope_refs", 4096)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if !relevant.iter().any(|id| scope.contains(*id))
            || name == cmd::text(&manifest, "rights_ref")?
                && ![
                    cmd::text(source_scope, "item_ref")?,
                    cmd::text(source_scope, "file_ref")?,
                ]
                .iter()
                .all(|id| scope.contains(*id))
        {
            return Err(SourceCommandError::Denied(
                "native rights record lacks exact relevant source scope",
            ));
        }
        rights_records.push((record, scope));
    }
    for target in cmd::array(rep, "publication_authority_refs")? {
        reader.read(
            cmd::text(target, "ref")?,
            Some(cmd::text(target, "sha256")?),
            true,
            false,
        )?;
    }
    let units = cmd::array(&packet, "units")?
        .iter()
        .filter(|row| row.object_get("unit_id") == binding.object_get("unit_id"))
        .collect::<Vec<_>>();
    let segments = cmd::array(&packet, "segmentations")?
        .iter()
        .filter(|row| row.object_get("segmentation_id") == binding.object_get("segmentation_id"))
        .collect::<Vec<_>>();
    if units.len() != 1 || segments.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "native unit or segmentation is not unique",
        ));
    }
    let unit = units[0];
    let segment = segments[0];
    if cmd::field(unit, "unit_version")? != cmd::field(binding, "unit_version")?
        || cmd::field(unit, "ordered_anchor_refs")? != cmd::field(binding, "ordered_anchor_refs")?
        || cmd::text(unit, "surface_posture")? != "source_bearing"
        || cmd::field(segment, "segmentation_version")?
            != cmd::field(binding, "segmentation_version")?
        || !cmd::array(segment, "ordered_unit_refs")?.contains(cmd::field(unit, "unit_id")?)
    {
        return Err(SourceCommandError::Conflict(
            "native unit membership version or ordered anchors differs",
        ));
    }
    if cmd::text(rep, "content_visibility")? != "public"
        || cmd::field(rep, "publication_authorized")? != &JsonValue::Bool(true)
        || cmd::text(rights, "packet_visibility")? != "public"
        || cmd::text(rights, "effective_visibility")? != "public"
        || cmd::field(rights, "publication_authorized")? != &JsonValue::Bool(true)
        || cmd::field(rights, "private_source_used")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Denied(
            "public profile native binding has no declared public content gate",
        ));
    }
    let narrower = rights_records
        .iter()
        .any(|(_, scope)| exact_scope.iter().any(|id| scope.contains(*id)));
    for (record, scope) in &rights_records {
        if narrower && !exact_scope.iter().any(|id| scope.contains(*id)) {
            continue;
        }
        if !["public_domain_reviewed", "licensed", "permission_granted"]
            .contains(&cmd::text(record, "assessment_status")?)
            || cmd::text(record, "visibility")? != "public_payload"
            || !["authorized", "authorized_with_conditions"]
                .contains(&cmd::text(record, "redistribution_posture")?)
            || !["allowed", "allowed_with_conditions"]
                .contains(&cmd::text(record, "derivative_posture")?)
            || ["legal_review_requested", "superseded"]
                .contains(&cmd::text(record, "review_status")?)
        {
            return Err(SourceCommandError::Denied(
                "native public content declaration conflicts with recorded rights gate",
            ));
        }
    }
    // Validate navigation without opening, executing, hashing or projecting text.
    NativeBindingReader::route(cmd::text(rep, "content_ref")?, false, false, true)?;
    reader.snapshot()
}

/// Reuse the maintained registry constructor law without loading unused profile
/// schemas or confusing a profile declaration with writer authority.
pub(crate) fn validate_source_profile_registry(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
) -> SourceCommandResult<JsonValue> {
    let contract = "ToS/contracts/semantic-entity-type-registry.schema.json";
    let registry = cmd::parse(required(ctx, REGISTRY)?)?;
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &[contract.into()],
        contract,
        &registry,
        Some(REGISTRY),
        true,
    )?;
    let entries = cmd::array(&registry, "types")?;
    let mut entities = BTreeMap::new();
    for entry in entries {
        if entities
            .insert(cmd::text(entry, "type_id")?, entry)
            .is_some()
        {
            return Err(SourceCommandError::Invalid("duplicate entity type"));
        }
    }
    let reserved = [
        "agent",
        "place",
        "organization",
        "work",
        "expression",
        "edition",
        "collection",
        "item",
        "link",
        "artifact",
        "composite",
    ];
    let mut unique: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for entry in entries {
        let Some(profile) = entry.object_get("source_record_profile") else {
            continue;
        };
        let kind = cmd::text(profile, "record_type")?;
        let type_id = cmd::text(entry, "type_id")?;
        let reader = cmd::text(profile, "reader")?;
        let (role, family) = match reader {
            "corpus-metadata-v1" => ("identity", "tos.entity.identity"),
            "semantic-metadata-v1" => ("semantic", "tos.entity.semantic-object"),
            _ => return Err(SourceCommandError::Unsupported("profile reader")),
        };
        let mut parents = BTreeSet::new();
        ancestry(&entities, type_id, &mut BTreeSet::new(), &mut parents)?;
        let retained = kind == "composite"
            && type_id == "tos.entity.composite"
            && cmd::text(profile, "retained_native_adapter").ok() == Some("scholarly-composite-v1")
            && reader == "corpus-metadata-v1"
            && cmd::text(profile, "catalog_filename")? == "composites.jsonl";
        let sign =
            kind == "sign" && type_id == "tos.entity.sign" && reader == "semantic-metadata-v1";
        if cmd::field(entry, "abstract")? != &JsonValue::Bool(false)
            || cmd::text(entry, "object_role")? != role
            || !parents.contains(family)
            || type_id == family
            || !retained
                && (reserved.contains(&kind)
                    || reserved.iter().any(|k| {
                        cmd::text(profile, "source_basename").ok()
                            == Some(format!("{k}.json").as_str())
                    })
                    || reserved.iter().any(|k| {
                        cmd::text(profile, "catalog_filename").ok()
                            == Some(format!("{k}s.jsonl").as_str())
                    })
                    || cmd::text(profile, "catalog_filename")? == "claims.jsonl")
            || cmd::text(profile, "id_prefix")? != format!("tos.{kind}.")
            || cmd::text(profile, "source_basename")? != format!("{kind}.json")
            || profile.object_get("retained_native_adapter").is_some() && !retained
            || profile.object_get("creation_gate").is_some() && !sign
            || sign && cmd::text(profile, "creation_gate").ok() != Some("sign-promotion-v1")
            || (kind == "sign" || type_id == "tos.entity.sign") && !sign
            || profile.object_get("native_binding_adapter").is_some()
                && (reader != "semantic-metadata-v1"
                    || cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1")
            || profile.object_get("identity_proposal_adapter").is_some()
                && (role != "semantic"
                    || cmd::text(profile, "identity_proposal_adapter")?
                        != "exact-semantic-metadata-v1"
                    || cmd::text(profile, "graph_layer")? != "source-profile"
                    || ["claim", "literal", "temporal-assertion"].contains(&kind))
        {
            return Err(SourceCommandError::Denied(
                "source record profile role, ancestry or reserved adapter collision",
            ));
        }
        for graph in ["source-claims", "source-navigation"] {
            let mappings = cmd::array(entry, "source_mappings")?
                .iter()
                .filter(|mapping| {
                    cmd::text(mapping, "source_graph").ok() == Some(graph)
                        && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                })
                .count();
            if mappings != 1
                || entries
                    .iter()
                    .filter(|other| cmd::text(other, "type_id").ok() != Some(type_id))
                    .any(|other| {
                        cmd::array(other, "source_mappings").is_ok_and(|mappings| {
                            mappings.iter().any(|mapping| {
                                cmd::text(mapping, "source_graph").ok() == Some(graph)
                                    && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                            })
                        })
                    })
            {
                return Err(SourceCommandError::Denied(
                    "source profile mapping has another type owner",
                ));
            }
        }
        for field in [
            "record_type",
            "id_prefix",
            "source_basename",
            "catalog_filename",
        ] {
            if !unique
                .entry(field)
                .or_default()
                .insert(cmd::text(profile, field)?)
            {
                return Err(SourceCommandError::Invalid(
                    "duplicate source record profile identity",
                ));
            }
        }
        let mut routes = BTreeSet::new();
        for route in cmd::array(profile, "schemas")? {
            if !routes.insert(cmd::text(route, "schema_version")?) {
                return Err(SourceCommandError::Invalid(
                    "duplicate source profile schema route",
                ));
            }
        }
    }
    Ok(registry)
}

pub(crate) fn public_profile(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    record: &JsonValue,
) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)> {
    let contract = "ToS/contracts/semantic-entity-type-registry.schema.json";
    let registry = validate_source_profile_registry(worker, deadline, cancelled, ctx)?;
    let entities = cmd::array(&registry, "types")?
        .iter()
        .map(|entry| Ok((cmd::text(entry, "type_id")?, entry)))
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    let entry = entities
        .get(cmd::text(config, "profile_type_id")?)
        .ok_or(SourceCommandError::Denied("profile type absent"))?;
    let profile = cmd::field(entry, "source_record_profile")?;
    let kind = cmd::text(profile, "record_type")?;
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    if base != cmd::text(profile, "source_basename")?
        || cmd::text(record, "record_type")? != kind
        || cmd::text(record, "record_id")? != cmd::text(config, "record_id")?
        || !valid_id(
            cmd::text(record, "record_id")?,
            cmd::text(profile, "id_prefix")?,
            false,
        )
        || !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
        || !matches!(
            cmd::text(record, "identity_status")?,
            "provisional" | "verified" | "disputed" | "superseded"
        )
        || strip(cmd::text(record, "preferred_label")?)?.is_empty()
        || cmd::integer(record, "record_version")? == 0
    {
        return Err(SourceCommandError::Denied(
            "public profile exact identity, visibility or metadata",
        ));
    }
    if kind == "composite"
        && (!source_path.starts_with("ToS/source-witnesses/scholarly-composites/")
            || source_path.split('/').count() < 7)
    {
        return Err(SourceCommandError::Denied(
            "retained composite profile path",
        ));
    }
    let (native_identity_snapshot, inventory_schema) = native_identity_inventory(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        cmd::text(record, "record_id")?,
    )?;
    let native_text = if profile.object_get("native_binding_adapter").is_some() {
        Some(native_text_binding(
            cut,
            worker,
            deadline,
            cancelled,
            ctx,
            cmd::field(record, "native_text_binding")?,
        )?)
    } else {
        None
    };
    if profile.object_get("native_binding_adapter").is_none()
        && record.object_get("native_text_binding").is_some()
    {
        return Err(SourceCommandError::Denied(
            "native text binding has no explicit profile adapter",
        ));
    }
    let routes = cmd::array(profile, "schemas")?
        .iter()
        .filter(|route| {
            cmd::text(route, "schema_version").ok() == cmd::text(record, "schema_version").ok()
        })
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "profile exact schema version unavailable",
        ));
    }
    let root = cmd::text(routes[0], "schema_ref")?;
    let mut resources = vec![CORPUS_SCHEMA.to_string()];
    resources.extend(texts(routes[0], "schema_dependencies", 128)?);
    resources.push(root.into());
    let mut seen = BTreeSet::new();
    resources.retain(|r| seen.insert(r.clone()));
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &resources,
        root,
        record,
        Some(source_path),
        true,
    )?;
    // This exact source contract supplies the shared Corpus property checks
    // through the same disposable worker, without a synthetic schema issuer.
    let shared = "ToS/contracts/source-metadata-record.schema.json";
    if !resources.iter().any(|path| path == shared) {
        return Err(SourceCommandError::Unsupported(
            "profile shared metadata worker contract absent from exact declared route",
        ));
    }
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &resources,
        shared,
        record,
        Some(source_path),
        true,
    )?;
    if inventory_schema {
        resources.push("ToS/contracts/semantic-annotation-packet-v2.schema.json".into());
    }
    let native_text_snapshot = native_text.map(|(snapshot, contracts)| {
        for name in contracts {
            if !resources.contains(&name) {
                resources.push(name);
            }
        }
        snapshot
    });
    resources.insert(0, contract.into());
    resources.insert(0, REGISTRY.into());
    if resources.len() > 128
        || resources
            .iter()
            .map(|name| required(ctx, name).map(<[u8]>::len))
            .collect::<SourceCommandResult<Vec<_>>>()?
            .into_iter()
            .sum::<usize>()
            > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "source profile contract snapshot budget",
        ));
    }
    Ok((
        profile.clone(),
        resources,
        native_identity_snapshot,
        native_text_snapshot,
    ))
}

/// Resolve an exact metadata version through its selected current source home
/// and complete retained record-revision predecessor closure. This is a read
/// seam for typed Claim owners, with the same concrete source-cut worker.
/// Compound parent receipts require their operation-specific verifier and
/// currently refuse explicitly; transport evidence cannot substitute for it.
pub fn resolve_record_version(
    ctx: &CommandContext,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String)> {
    resolve_record_version_selected(ctx, None, exact, worker, deadline, cancelled)
}

pub(crate) fn resolve_record_version_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String)> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "metadata resolver selected cut differs",
        ));
    }
    resolve_record_version_selected(ctx, Some(cut), exact, worker, deadline, cancelled)
}

fn resolve_record_version_selected(
    ctx: &CommandContext,
    cut: Option<&CorpusCutReader>,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String)> {
    ctx.check()?;
    exact_ref(exact)?;
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "metadata resolver worker and source cut differ",
        ));
    }
    let mut matches = Vec::new();
    for file in &ctx.files {
        let location = file.path.as_str();
        if !location.starts_with("ToS/source-witnesses/")
            || !location.ends_with(".json")
            || location.ends_with(".human-forms.json")
            || location.ends_with(HISTORY)
            || location.split('/').any(|part| {
                part.starts_with('.')
                    || matches!(
                        part,
                        "owner-local" | "catalog" | "payload" | "local-content" | "private"
                    )
            })
        {
            continue;
        }
        let record = cmd::parse(&file.raw)?;
        if let Ok(subject) = source_forms::metadata_subject(&record) {
            if cmd::field(&subject, "id")? == cmd::field(exact, "id")? {
                matches.push((location, record, subject));
            }
        }
    }
    if matches.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "exact metadata identity has no unique selected current owner",
        ));
    }
    let (location, record, subject) = matches
        .pop()
        .ok_or(SourceCommandError::Conflict("selected metadata missing"))?;
    let schema_version = cmd::text(&record, "schema_version")?;
    let mut descriptor = cmd::object(vec![
        ("source_path", cmd::string(location)),
        ("record_id", cmd::field(&subject, "id")?.clone()),
    ]);
    let family = match schema_version {
        "tos_historical_record_v1" => RevisionFamily::Historical,
        "tos_corpus_record_v1" => {
            cmd::set(
                &mut descriptor,
                "record_type",
                cmd::field(&record, "record_type")?.clone(),
            )?;
            RevisionFamily::CorpusSelectedV3
        }
        "tos_artifact_source_witness_v1"
        | "tos_artifact_source_witness_v2"
        | "tos_scholarly_composite_witness_v1"
        | "tos_source_link_v1" => {
            let kind = if schema_version.starts_with("tos_artifact_") {
                "artifact"
            } else if schema_version == "tos_scholarly_composite_witness_v1" {
                "composite"
            } else {
                "link"
            };
            cmd::set(&mut descriptor, "record_type", cmd::string(kind))?;
            cmd::set(
                &mut descriptor,
                "record_schema_version",
                cmd::string(schema_version),
            )?;
            RevisionFamily::NativeSelected
        }
        _ => {
            let registry = cmd::parse(required(ctx, REGISTRY)?)?;
            let entries = cmd::array(&registry, "types")?
                .iter()
                .filter(|entry| {
                    entry
                        .object_get("source_record_profile")
                        .and_then(|p| cmd::text(p, "record_type").ok())
                        == cmd::text(&record, "record_type").ok()
                })
                .collect::<Vec<_>>();
            if entries.len() != 1 {
                return Err(SourceCommandError::Unsupported(
                    "exact metadata schema has no unique declared profile",
                ));
            }
            cmd::set(
                &mut descriptor,
                "profile_type_id",
                cmd::field(entries[0], "type_id")?.clone(),
            )?;
            RevisionFamily::PublicProfile
        }
    };
    // This descriptor routes a read. It is never evaluated as a protected
    // configuration or used to manufacture a grant or preparation plan.
    let mut routed = ctx.clone();
    routed.configuration_raw = cmd::canonical(&descriptor)?;
    profile(
        cut,
        worker,
        deadline,
        cancelled,
        &routed,
        &descriptor,
        family,
        &record,
    )?;
    let files = package(&routed, location, true)?;
    let retained = verify_history(&routed, &descriptor, &files, &record)?;
    if cmd::same(&subject, exact)? {
        return Ok((record, location.into()));
    }
    for receipt in cmd::array(&retained, "receipts")? {
        if cmd::same(cmd::field(receipt, "previous_source")?, exact)? {
            let (archived, _) = read_archive(&routed, &descriptor, receipt)?;
            let (_, base) = split(location)?;
            let record = cmd::parse(archived.get(base).ok_or(SourceCommandError::Conflict(
                "retained metadata record absent",
            ))?)?;
            return Ok((record, location.into()));
        }
    }
    Err(SourceCommandError::Conflict(
        "requested metadata version is not retained in exact owner history",
    ))
}
