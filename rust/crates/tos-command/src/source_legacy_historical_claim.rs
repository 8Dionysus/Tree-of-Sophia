//! Native preparation for the maintained captured historical Claim owner.
//!
//! Historical Claims keep their authored schema and predicate vocabulary. The
//! adapter narrows mutation to explicitly delegated descriptive qualifiers;
//! retained packages and receipt-bound creation bytes stay under the existing
//! Claim and HumanForm kernels.
use crate::source_command::{self as cmd, *};
use crate::source_creation_store::{LegacyArchiveReader, LegacyPackage};
use crate::{source_claims, source_forms, source_revisions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath, SourceRevision};
use tos_source_store::CorpusCutReader;
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const STREAM: &str = "historical-claims.jsonl";
const HISTORY: &str = "claim-revision-history.json";
const REVISION_OWNER: &str = "tos_local_historical_claim_revision_owner_v1";
const FORM_OWNER: &str = "tos_local_historical_claim_form_owner_v1";
const CLAIM_SCHEMA: &str = "ToS/contracts/historical-claim.schema.json";
const CAPTURE: &[&str] = &[
    "source-create-request.json",
    "source-create-receipt.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
];
const CLAIM_SCHEMA_CONTRACTS: &[&str] = &[
    CLAIM_SCHEMA,
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/historical-record.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/claim-display-fields.schema.json",
];
const CLAIM_REGISTRY_REFS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
const HISTORICAL_GROUND_IMPLEMENTATIONS: &[&str] = &[
    "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
    "rust/crates/tos-command/src/source_claims.rs",
    "rust/crates/tos-command/src/source_corpus_index_projection.rs",
    "rust/crates/tos-compiler/src/source_bibliographic.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-command/src/source_native_cli.rs",
];
const REVISION_IMPLEMENTATIONS: &[&str] = &[
    "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
const SOURCE_HISTORY: &str = "source-revision-history.json";

pub(crate) fn pg_required_contract_paths() -> Vec<String> {
    CLAIM_SCHEMA_CONTRACTS
        .iter()
        .chain(CLAIM_REGISTRY_REFS)
        .chain(
            REVISION_IMPLEMENTATIONS
                .iter()
                .filter(|name| name.starts_with("ToS/contracts/")),
        )
        .chain(
            [
                "ToS/contracts/semantic-entity-type-registry.schema.json",
                "ToS/contracts/semantic-relation-type-registry.schema.json",
                "ToS/contracts/provenance-event-v2.schema.json",
            ]
            .iter(),
        )
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) struct LegacyClaimPlan {
    pub(crate) files: Option<LegacyPackage>,
    pub(crate) response: JsonValue,
    pub(crate) archive: Option<(String, LegacyPackage)>,
    pub(crate) reads: Vec<SourceFile>,
    pub(crate) replayed: bool,
}

struct Owner {
    config: JsonValue,
    request: JsonValue,
    path: RelativePath,
    config_digest: String,
    contract_digests: JsonValue,
    claim_id: String,
    reads: Vec<SourceFile>,
}

struct History {
    payload: JsonValue,
    initial_stream: Vec<u8>,
}

/// Exact point lookups from the maintained PG identity index. Absence is a
/// first-class result; this is a selected dependency closure, never a claim
/// that all authored identities were materialized.
#[derive(Clone, Debug, Default)]
pub(crate) struct HistoricalIdentityIndex {
    pub(crate) entries: BTreeMap<String, Option<(String, RelativePath)>>,
}

impl HistoricalIdentityIndex {
    fn resolution(&self, token: &str) -> SourceCommandResult<Option<&(String, RelativePath)>> {
        self.entries
            .get(token)
            .map(Option::as_ref)
            .ok_or(SourceCommandError::Conflict(
                "historical Claim PG identity input was not selected",
            ))
    }

    fn selected_digest(&self) -> SourceCommandResult<JsonValue> {
        self.entries
            .iter()
            .map(|(token, resolution)| {
                let value = match resolution {
                    Some((kind, path)) => cmd::object(vec![
                        ("kind", cmd::string(kind)),
                        ("path", cmd::string(path.as_str())),
                    ]),
                    None => JsonValue::Null,
                };
                Ok((JsonString::from_utf8(token), value))
            })
            .collect::<SourceCommandResult<Vec<_>>>()
            .map(JsonValue::Object)
    }
}

trait HistoricalCurrentSource {
    fn revision(&self) -> SourceRevision;
    fn identity_index(&self) -> Option<&HistoricalIdentityIndex> {
        None
    }
    fn member_metadata(&self, path: &RelativePath)
    -> SourceCommandResult<Option<(Digest256, u64)>>;
    fn read_member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>>;
}

struct CorpusHistoricalCurrentSource<'a>(&'a CorpusCutReader);

impl HistoricalCurrentSource for CorpusHistoricalCurrentSource<'_> {
    fn revision(&self) -> SourceRevision {
        self.0.current().revision()
    }

    fn member_metadata(
        &self,
        path: &RelativePath,
    ) -> SourceCommandResult<Option<(Digest256, u64)>> {
        Ok(self
            .0
            .current()
            .member(path)
            .map(|member| (member.sha256, member.size_bytes)))
    }

    fn read_member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.0
            .read_member(revision, path, max_bytes as u64, deadline, cancelled)
            .map(|member| member.raw)
            .map_err(|_| {
                SourceCommandError::Conflict("historical Claim secure source read refused")
            })
    }
}

struct MembersHistoricalCurrentSource<'a> {
    revision: SourceRevision,
    members: &'a [SourceFile],
    identity_index: Option<&'a HistoricalIdentityIndex>,
}

impl HistoricalCurrentSource for MembersHistoricalCurrentSource<'_> {
    fn revision(&self) -> SourceRevision {
        self.revision
    }

    fn identity_index(&self) -> Option<&HistoricalIdentityIndex> {
        self.identity_index
    }

    fn member_metadata(
        &self,
        path: &RelativePath,
    ) -> SourceCommandResult<Option<(Digest256, u64)>> {
        let mut found = None;
        for member in self.members {
            if member.path != *path {
                continue;
            }
            if found.is_some() {
                return Err(SourceCommandError::Invalid(
                    "duplicate historical Claim current member",
                ));
            }
            found = Some((Digest256::of_bytes(&member.raw), member.raw.len() as u64));
        }
        Ok(found)
    }

    fn read_member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if revision != self.revision {
            return Err(SourceCommandError::Conflict(
                "historical Claim source revision differs",
            ));
        }
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(SourceCommandError::Denied(
                "historical Claim source read cancelled or expired",
            ));
        }
        let mut found = None;
        for member in self.members {
            if member.path != *path {
                continue;
            }
            if found.is_some() {
                return Err(SourceCommandError::Invalid(
                    "duplicate historical Claim current member",
                ));
            }
            found = Some(member.raw.as_slice());
        }
        let raw = found.ok_or(SourceCommandError::Unsupported(
            "historical Claim source member absent",
        ))?;
        if raw.len() > max_bytes {
            return Err(SourceCommandError::Invalid(
                "historical Claim source member byte budget",
            ));
        }
        Ok(raw.to_vec())
    }
}

fn invalid() -> SourceCommandError {
    SourceCommandError::Invalid("captured historical Claim owner input")
}

fn rel(value: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(value).map_err(|_| invalid())
}

fn object_map(value: &JsonValue) -> SourceCommandResult<&[(JsonString, JsonValue)]> {
    value.as_object().ok_or(invalid())
}

fn optional_text<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value.object_get(key).and_then(JsonValue::as_str)
}

fn claim_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.claim.") else {
        return false;
    };
    !rest.is_empty()
        && rest.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn historical_record_id(value: &str) -> bool {
    [
        "tos.historical-event.",
        "tos.historical-process.",
        "tos.historical-state.",
    ]
    .into_iter()
    .any(|prefix| {
        value.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty()
                && rest.split(['.', '-']).all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                })
        })
    })
}

fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn historical_path(value: &str) -> bool {
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() >= 5
        && parts[..3] == ["ToS", "source-witnesses", "history"]
        && parts.last() == Some(&STREAM)
        && parts[3..parts.len() - 1].iter().all(|part| slug(part))
}

fn texts(config: &JsonValue, key: &str, maximum: usize) -> SourceCommandResult<Vec<String>> {
    let values = cmd::array(config, key)?;
    if values.len() > maximum {
        return Err(invalid());
    }
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let item = value.as_str().ok_or(invalid())?;
        if item.is_empty() || !seen.insert(item.to_owned()) {
            return Err(invalid());
        }
        result.push(item.to_owned());
    }
    Ok(result)
}

fn form_field_ids(config: &JsonValue) -> SourceCommandResult<Vec<String>> {
    let values = texts(config, "allowed_form_field_ids", 4)?;
    if values.is_empty()
        || values.iter().any(|value| {
            !matches!(
                value.as_str(),
                "claim.statement" | "claim.name" | "claim.caption" | "claim.hover"
            )
        })
    {
        return Err(SourceCommandError::Denied(
            "historical Claim form field scope",
        ));
    }
    Ok(values)
}

fn form_id(value: &str) -> bool {
    value.strip_prefix("tos.form.").is_some_and(|rest| {
        rest.as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            && rest.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
    })
}

fn owner(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    authored_members: &[SourceFile],
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Owner> {
    ctx.check()?;
    let config = cmd::parse(&ctx.configuration_raw)?;
    let request = cmd::parse(&ctx.request_raw)?;
    let version = cmd::text(&config, "schema_version")?;
    if !matches!(version, REVISION_OWNER | FORM_OWNER) {
        return Err(SourceCommandError::Unsupported(
            "historical Claim owner schema",
        ));
    }
    let is_revision = version == REVISION_OWNER;
    let expected = if is_revision {
        &[
            "schema_version",
            "uid",
            "principal_id",
            "source_root",
            "source_path",
            "authority_ref",
            "expires_at",
            "claim_id",
            "allowed_operations",
            "allowed_fields",
            "allowed_evidence_refs",
            "allowed_form_ids",
            "historical_record_id",
            "creation_receipt_sha256",
            "allowed_qualifier_fields",
            "allowed_form_field_ids",
        ][..]
    } else {
        &[
            "schema_version",
            "uid",
            "principal_id",
            "source_root",
            "source_path",
            "authority_ref",
            "expires_at",
            "claim_id",
            "allowed_operations",
            "allowed_form_ids",
            "allowed_form_field_ids",
            "historical_record_id",
            "creation_receipt_sha256",
        ][..]
    };
    cmd::exact_keys(&config, expected)?;
    if cmd::integer(&config, "uid")? != ctx.effective_uid
        || !cmd::nonblank(cmd::text(&config, "principal_id")?)
        || !cmd::nonblank(cmd::text(&config, "authority_ref")?)
        || !cmd::text(&config, "source_root")?.starts_with('/')
    {
        return Err(SourceCommandError::Denied(
            "historical Claim owner identity",
        ));
    }
    cmd::validate_expiry(cmd::text(&config, "expires_at")?, &ctx.recorded_at)?;
    let path_text = cmd::text(&config, "source_path")?;
    if !historical_path(path_text) {
        return Err(SourceCommandError::Denied(
            "exact public historical Claim path",
        ));
    }
    let path = rel(path_text)?;
    let id = cmd::text(&config, "claim_id")?;
    let historical_id = cmd::text(&config, "historical_record_id")?;
    let receipt_hash = cmd::text(&config, "creation_receipt_sha256")?;
    if !claim_id(id)
        || !historical_record_id(historical_id)
        || !receipt_hash.starts_with("sha256:")
        || receipt_hash.len() != 71
        || !receipt_hash[7..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(SourceCommandError::Denied(
            "historical Claim captured identity",
        ));
    }
    let form_ids = texts(&config, "allowed_form_ids", 32)?;
    if form_ids.iter().any(|value| !form_id(value)) {
        return Err(SourceCommandError::Denied(
            "historical Claim form identity scope",
        ));
    }
    let fields = form_field_ids(&config)?;
    let (allowed_operations, allowed_fields) = if is_revision {
        let operations = texts(&config, "allowed_operations", 1)?;
        let delegated = texts(&config, "allowed_fields", 1)?;
        let evidence = texts(&config, "allowed_evidence_refs", 128)?;
        let qualifiers = texts(&config, "allowed_qualifier_fields", 4)?;
        if operations.as_slice() != ["claim.revise"]
            || delegated.as_slice() != ["qualifiers"]
            || !evidence.is_empty()
            || qualifiers.is_empty()
            || qualifiers.iter().any(|name| {
                !matches!(
                    name.as_str(),
                    "statement" | "statement_language" | "statement_script" | "display_fields"
                )
            })
        {
            return Err(SourceCommandError::Denied(
                "historical corrections delegate descriptive qualifiers only",
            ));
        }
        (operations, Some(qualifiers))
    } else {
        let operations = texts(&config, "allowed_operations", 2)?;
        if operations
            .iter()
            .any(|name| !matches!(name.as_str(), "form.create" | "form.revise"))
        {
            return Err(SourceCommandError::Denied(
                "historical Claim form operations",
            ));
        }
        (operations, None)
    };
    if is_revision && fields.is_empty() {
        return Err(invalid());
    }
    let contracts = contract_digests(ctx)?;
    if !is_revision {
        if let Some(index) = cut.identity_index() {
            selected_form_identity_inputs(ctx, index, &config, &path, id)?;
        } else {
            let inventory = source_claims::maintained_inventory_from_members(
                ctx,
                authored_members,
                false,
                worker,
                deadline,
                cancelled,
            )?;
            form_identity_inputs(
                ctx, cut, &inventory, &config, &path, id, worker, deadline, cancelled,
            )?;
        }
    }
    let config_digest = if is_revision {
        cmd::record_digest(&config)?.to_prefixed()
    } else {
        cmd::record_digest(&cmd::object(vec![
            ("configuration", config.clone()),
            ("source_contracts", contracts.clone()),
        ]))?
        .to_prefixed()
    };
    validate_request_shape(&request, is_revision)?;
    // Keep these schema dependencies bound to the actual selected worker; the
    // historical-claim schema remains the root and is never rewritten as a
    // public source-Claim profile.
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "historical Claim worker and source cut differ",
        ));
    }
    let _ = (allowed_operations, allowed_fields);
    let claim_id = id.to_owned();
    Ok(Owner {
        config,
        request,
        path,
        config_digest,
        contract_digests: contracts,
        claim_id,
        reads: ctx.files.clone(),
    })
}

/// Bind the complete current form-ID inventory from the authored cut. The
/// selected owner's own adjacent set is checked separately by its retained
/// form lineage; every sibling set is an identity collision boundary.
fn form_identity_inputs(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    inventory: &source_claims::MaintainedInventory,
    config: &JsonValue,
    target: &RelativePath,
    claim_id: &str,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let allocated = texts(config, "allowed_form_ids", 32)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let target_form = package_form_name(target.as_str(), claim_id)?;
    let target_form_path = format!(
        "{}/{}",
        target.as_str().rsplit_once('/').ok_or(invalid())?.0,
        target_form
    );
    let mut paths = BTreeSet::new();
    for object in inventory.objects.values() {
        let source = cmd::text(object, "source_record_ref")?;
        let source_path = Path::new(source);
        let stem = source_path
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or(invalid())?;
        let parent = source_path.parent().ok_or(invalid())?;
        let forms = parent.join(format!("{stem}.human-forms.json"));
        let forms = forms.to_str().ok_or(invalid())?.to_owned();
        if forms != target_form_path {
            paths.insert(forms);
        }
    }
    for claim in inventory.claims.values() {
        let source = cmd::text(claim, "source_claim_file_ref")?;
        let source_path = Path::new(source);
        let stream = source_path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(invalid())?;
        if stream != "source-claims.jsonl" && !historical_path(source) {
            continue;
        }
        let parent = source_path.parent().ok_or(invalid())?;
        let stem = source_path
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or(invalid())?;
        let id = cmd::text(claim, "claim_id")?;
        let forms = parent.join(format!(
            "{stem}.{}.human-forms.json",
            Digest256::of_bytes(id.as_bytes()).to_hex()
        ));
        let forms = forms.to_str().ok_or(invalid())?.to_owned();
        if forms != target_form_path {
            paths.insert(forms);
        }
    }
    if paths.len() > 4096 {
        return Err(SourceCommandError::Invalid(
            "historical Claim form inventory path budget",
        ));
    }
    let mut total = 0usize;
    let mut inputs = Vec::new();
    for name in paths {
        let path = rel(&name)?;
        if cut.member_metadata(&path)?.is_none() {
            continue;
        }
        let raw = ctx.file(&path)?.ok_or(SourceCommandError::Unsupported(
            "complete historical Claim form inventory input absent",
        ))?;
        let secure = cut.read_member(ctx.base_revision, &path, 2_097_152, deadline, cancelled)?;
        let (digest, size) = cut
            .member_metadata(&path)?
            .ok_or(SourceCommandError::Conflict(
                "historical Claim form inventory member disappeared",
            ))?;
        if secure != raw || digest != Digest256::of_bytes(raw) || size != raw.len() as u64 {
            return Err(SourceCommandError::Conflict(
                "historical Claim form inventory differs from authored cut",
            ));
        }
        total = total
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "historical Claim form inventory byte overflow",
            ))?;
        if total > 33_554_432 || raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid(
                "historical Claim form inventory byte budget",
            ));
        }
        let payload = cmd::parse(raw)?;
        schema(
            path.as_str(),
            ctx,
            worker,
            &payload,
            "ToS/contracts/human-form-set.schema.json",
            &["ToS/contracts/human-form-set.schema.json"],
            deadline,
            cancelled,
        )?;
        tos_validation::source_forms::inspect_lineage_raw(raw)
            .map_err(|_| SourceCommandError::Invalid("historical Claim sibling form lineage"))?;
        for form in cmd::array(&payload, "forms")?
            .iter()
            .chain(cmd::array(&payload, "prior_forms")?)
        {
            let id = cmd::text(form, "form_id")?.to_owned();
            if !form_id(&id) {
                return Err(SourceCommandError::Invalid(
                    "historical Claim sibling form identity",
                ));
            }
            if allocated.contains(&id) {
                return Err(SourceCommandError::Conflict(
                    "form identity already exists on another subject",
                ));
            }
        }
        inputs.push((
            JsonString::from_utf8(&name),
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ));
    }
    Ok(JsonValue::Object(inputs))
}

fn selected_form_identity_inputs(
    ctx: &CommandContext,
    index: &HistoricalIdentityIndex,
    config: &JsonValue,
    target: &RelativePath,
    claim_id: &str,
) -> SourceCommandResult<JsonValue> {
    let allocated = texts(config, "allowed_form_ids", 32)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let target_form = package_form_name(target.as_str(), claim_id)?;
    let target_form_path = format!(
        "{}/{}",
        target.as_str().rsplit_once('/').ok_or(invalid())?.0,
        target_form
    );
    let mut selected_form_ids = BTreeMap::new();
    for member in &ctx.files {
        if !member.path.as_str().ends_with(".human-forms.json")
            || member.path.as_str().contains("/.record-revisions/")
        {
            continue;
        }
        let set = cmd::parse(&member.raw)?;
        let mut ids = BTreeSet::new();
        for section in ["forms", "prior_forms"] {
            for form in cmd::array(&set, section)? {
                ids.insert(cmd::text(form, "form_id")?.to_owned());
            }
        }
        selected_form_ids.insert(member.path.as_str().to_owned(), ids);
    }
    let mut inputs = Vec::new();
    for id in allocated {
        let value = match index.resolution(&id)? {
            None if selected_form_ids.values().any(|ids| ids.contains(&id)) => {
                return Err(SourceCommandError::Conflict(
                    "historical Claim selected form is missing from PG identity index",
                ));
            }
            None => JsonValue::Null,
            Some((kind, path)) if kind == "form" && path.as_str() == target_form_path => {
                let raw = ctx.file(path)?.ok_or(SourceCommandError::Conflict(
                    "historical Claim indexed form set is absent",
                ))?;
                let set = cmd::parse(raw)?;
                let present = ["forms", "prior_forms"].into_iter().any(|key| {
                    set.object_get(key)
                        .and_then(JsonValue::as_array)
                        .is_some_and(|forms| {
                            forms.iter().any(|form| {
                                form.object_get("form_id").and_then(JsonValue::as_str)
                                    == Some(id.as_str())
                            })
                        })
                });
                if !present {
                    return Err(SourceCommandError::Conflict(
                        "historical Claim indexed form identity differs from its selected set",
                    ));
                }
                cmd::object(vec![
                    ("kind", cmd::string(kind)),
                    ("path", cmd::string(path.as_str())),
                ])
            }
            Some(_) => {
                return Err(SourceCommandError::Conflict(
                    "form identity already exists on another subject",
                ));
            }
        };
        inputs.push((JsonString::from_utf8(&id), value));
    }
    Ok(JsonValue::Object(inputs))
}

/// Return the source identity tokens and direct ToS path evidence needed by a
/// historical Claim revision or form operation. The caller resolves identity
/// tokens through the admitted PG index, reads direct ToS paths exactly, and
/// binds all results into the prepared readset.
pub(crate) fn required_identity_tokens(
    config_raw: &[u8],
    current: &LegacyPackage,
    operation: &str,
) -> SourceCommandResult<BTreeSet<String>> {
    let config = cmd::parse(config_raw)?;
    let owner_schema = cmd::text(&config, "schema_version")?;
    let needs_form_collisions = owner_schema == FORM_OWNER;
    let needs_grounding =
        owner_schema == REVISION_OWNER && matches!(operation, "prepare-revise" | "claim.revise");
    if !needs_form_collisions && !needs_grounding {
        return Ok(BTreeSet::new());
    }
    let claim_id = cmd::text(&config, "claim_id")?;
    let stream = current.get(STREAM).ok_or(invalid())?;
    let mut rows = claim_rows(stream)?;
    let claim = rows.remove(claim_id).ok_or(SourceCommandError::Conflict(
        "delegated historical Claim is absent",
    ))?;
    let mut tokens = BTreeSet::new();
    if needs_grounding {
        tokens.insert(cmd::text(&claim, "subject_ref")?.to_owned());
        let object = cmd::field(&claim, "object")?;
        if let Some(id) = object.as_str() {
            tokens.insert(id.to_owned());
        } else if let Some(id) = object
            .object_get("relative")
            .and_then(|value| value.object_get("anchor_ref"))
            .and_then(JsonValue::as_str)
        {
            tokens.insert(id.to_owned());
        }
        tokens.insert(cmd::text(&claim, "provenance_event_ref")?.to_owned());
        for key in ["evidence_refs", "counterevidence_refs"] {
            if let Some(values) = claim.object_get(key) {
                for value in values.as_array().ok_or(invalid())? {
                    let reference = value.as_str().ok_or(invalid())?;
                    tokens.insert(reference.to_owned());
                }
            }
        }
    }
    if needs_form_collisions || needs_grounding {
        for id in texts(&config, "allowed_form_ids", 32)? {
            if !form_id(&id) {
                return Err(SourceCommandError::Denied(
                    "historical Claim form identity scope",
                ));
            }
            tokens.insert(id);
        }
    }
    let token_bytes = tokens.iter().try_fold(0usize, |sum, token| {
        if token.len() > 8_192 {
            return None;
        }
        sum.checked_add(token.len())
    });
    if tokens.iter().any(String::is_empty)
        || tokens.len() > 256
        || token_bytes.is_none_or(|bytes| bytes > 131_072)
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim identity lookup budget",
        ));
    }
    Ok(tokens)
}

fn validate_request_shape(request: &JsonValue, revision: bool) -> SourceCommandResult<()> {
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid(
            "historical Claim request schema",
        ));
    }
    let operation = cmd::text(request, "operation")?;
    let keys: &[&str] = match (revision, operation) {
        (true, "describe") => &["operation"],
        (true, "inspect-version") => &["operation", "source"],
        (true, "prepare-revise") => &["operation", "fields", "forms", "reason"],
        (true, "claim.revise") => &[
            "operation",
            "fields",
            "forms",
            "reason",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_inputs",
        ],
        (false, "describe") => &["operation"],
        (false, "prepare") => &["operation", "form_id", "field_id"],
        (false, "apply") => &[
            "operation",
            "changes",
            "command_id",
            "expected_source",
            "expected_revision",
            "expected_configuration",
        ],
        _ => {
            return Err(SourceCommandError::Unsupported(
                "historical Claim operation",
            ));
        }
    };
    let mut keys = keys.to_vec();
    keys.push("schema_version");
    cmd::exact_keys(request, &keys)
}

fn contract_digests(ctx: &CommandContext) -> SourceCommandResult<JsonValue> {
    Ok(JsonValue::Object(
        CLAIM_SCHEMA_CONTRACTS
            .iter()
            .chain(CLAIM_REGISTRY_REFS)
            .map(|name| {
                let bytes = ctx
                    .file(&rel(name)?)?
                    .ok_or(SourceCommandError::Unsupported(
                        "historical Claim contract absent from selected source",
                    ))?;
                Ok((
                    JsonString::from_utf8(name),
                    cmd::string(&Digest256::of_bytes(bytes).to_prefixed()),
                ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    ))
}

fn schema(
    source_path: &str,
    ctx: &CommandContext,
    worker: &mut impl source_revisions::RecordSchemaWorker<Identity = SourceRevision>,
    instance: &JsonValue,
    root: &str,
    refs: &[&str],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    // Whole-corpus assessment has no single command-owner source_path.
    // Every historical schema call supplies the source it is checking.
    source_revisions::schema_for_source_path_worker(
        worker,
        deadline,
        cancelled,
        ctx,
        &refs
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
        root,
        source_path,
        instance,
    )
}

fn claim_rows(raw: &[u8]) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "historical Claim stream byte budget",
        ));
    }
    let mut rows = BTreeMap::new();
    for line in raw.split(|byte| matches!(*byte, b'\r' | b'\n')) {
        if line
            .iter()
            .all(|byte| matches!(*byte, b' ' | b'\t' | 0x0b | 0x0c))
        {
            continue;
        }
        let row = cmd::parse(line)?;
        let id = cmd::text(&row, "claim_id")?.to_owned();
        if !claim_id(&id) || rows.insert(id, row).is_some() {
            return Err(SourceCommandError::Conflict(
                "historical Claim identity repeats",
            ));
        }
    }
    Ok(rows)
}

pub(crate) fn pg_selected_claim(raw: &[u8], claim_id: &str) -> SourceCommandResult<JsonValue> {
    claim_rows(raw)?
        .remove(claim_id)
        .ok_or(SourceCommandError::Conflict(
            "historical Claim selected form source is absent",
        ))
}

fn subject(record: &JsonValue) -> SourceCommandResult<JsonValue> {
    source_forms::metadata_subject(record)
}

fn same(left: &JsonValue, right: &JsonValue) -> SourceCommandResult<bool> {
    cmd::same(left, right)
}

fn replace_row(raw: &[u8], revised: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    source_claims::replace_claim_row(raw, revised)
}

fn advance(record: &JsonValue, fields: &JsonValue) -> SourceCommandResult<JsonValue> {
    cmd::exact_keys(fields, &["qualifiers"])?;
    let patch = cmd::field(fields, "qualifiers")?;
    let qualifiers = object_map(patch)?;
    if qualifiers.is_empty() {
        return Err(SourceCommandError::Denied(
            "historical Claim qualifier patch is empty",
        ));
    }
    for (key, value) in qualifiers {
        let key = key.as_str().ok_or(invalid())?;
        if !matches!(
            key,
            "statement" | "statement_language" | "statement_script" | "display_fields"
        ) {
            return Err(SourceCommandError::Denied(
                "historical Claim structural qualifier",
            ));
        }
    }
    source_claims::advance_claim(record, fields, None)
}

fn archive_reference(id: &str, revision: &str) -> SourceCommandResult<String> {
    let hash = revision.strip_prefix("sha256:").ok_or(invalid())?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid());
    }
    Ok(format!(
        "ToS/source-witnesses/.record-revisions/{}-{hash}",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}

fn required_archive_refs(
    current: &LegacyPackage,
    config: &JsonValue,
) -> SourceCommandResult<Vec<String>> {
    let Some(raw) = current.get(HISTORY) else {
        return Ok(Vec::new());
    };
    let history = cmd::parse(raw)?;
    cmd::exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    if cmd::text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || cmd::text(&history, "source_path")? != cmd::text(config, "source_path")?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim history identity differs",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "Claim revision history capacity",
        ));
    }
    let mut refs = Vec::with_capacity(receipts.len());
    let mut seen = BTreeSet::new();
    for receipt in receipts {
        let id = cmd::text(cmd::field(receipt, "previous_source")?, "id")?;
        let revision = cmd::text(receipt, "previous_revision")?;
        let expected = archive_reference(id, revision)?;
        if cmd::text(receipt, "archive_path")? != expected || !seen.insert(expected.clone()) {
            return Err(SourceCommandError::Conflict(
                "Claim archive locator is not receipt-bound",
            ));
        }
        refs.push(expected);
    }
    Ok(refs)
}

fn required_source_archive_refs(
    current: &LegacyPackage,
    config: &JsonValue,
) -> SourceCommandResult<Vec<String>> {
    let Some(raw) = current.get(SOURCE_HISTORY) else {
        return Ok(Vec::new());
    };
    let history = cmd::parse(raw)?;
    cmd::exact_keys(&history, &["schema_version", "record_id", "receipts"])?;
    let record_id = cmd::text(config, "historical_record_id")?;
    if !matches!(
        cmd::text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || cmd::text(&history, "record_id")? != record_id
    {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record history identity differs",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.is_empty() || receipts.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "captured Historical Record history capacity",
        ));
    }
    let mut refs = Vec::with_capacity(receipts.len());
    let mut seen = BTreeSet::new();
    for receipt in receipts {
        let previous = cmd::field(receipt, "previous_source")?;
        let source = cmd::field(receipt, "source")?;
        let id = cmd::text(previous, "id")?;
        let revision = cmd::text(receipt, "previous_revision")?;
        let expected = archive_reference(id, revision)?;
        if id != record_id
            || cmd::text(source, "id")? != record_id
            || cmd::text(receipt, "archive_path")? != expected
            || !seen.insert(expected.clone())
        {
            return Err(SourceCommandError::Conflict(
                "Historical Record archive locator is not receipt-bound",
            ));
        }
        refs.push(expected);
    }
    Ok(refs)
}

pub(crate) fn required_archives(
    current: &LegacyPackage,
    config: &JsonValue,
) -> SourceCommandResult<Vec<String>> {
    let mut refs = required_archive_refs(current, config)?;
    refs.extend(required_source_archive_refs(current, config)?);
    refs.sort();
    if refs.len() > 256 || refs.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SourceCommandError::Conflict(
            "retained Claim/Record archive reference overlap or capacity",
        ));
    }
    Ok(refs)
}

fn decode_archive(
    raw: LegacyPackage,
    config: &JsonValue,
    receipt: &JsonValue,
) -> SourceCommandResult<(LegacyPackage, JsonValue)> {
    let path = cmd::text(receipt, "archive_path")?;
    let id = cmd::text(cmd::field(receipt, "previous_source")?, "id")?;
    let revision = cmd::text(receipt, "previous_revision")?;
    if path != archive_reference(id, revision)? || raw.len() < 2 || raw.len() > 65 {
        return Err(SourceCommandError::Conflict(
            "historical Claim archive route differs",
        ));
    }
    let manifest_raw = raw.get("manifest.json").ok_or(invalid())?;
    let manifest = cmd::parse(manifest_raw)?;
    let selected = cmd::text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
    if selected != receipt.object_get("publication").is_some() {
        return Err(SourceCommandError::Conflict(
            "historical source archive protocol differs from its retained receipt",
        ));
    }
    if selected {
        cmd::exact_keys(
            &manifest,
            &[
                "schema_version",
                "source_path",
                "source",
                "revision",
                "files",
                "publication_protocol",
            ],
        )?;
    } else {
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
    }
    if (!selected && cmd::text(&manifest, "schema_version")? != "tos_source_package_archive_v1")
        || selected
            && cmd::text(&manifest, "publication_protocol")? != "tos_selected_source_metadata_v1"
        || cmd::text(&manifest, "source_path")? != cmd::text(config, "source_path")?
        || cmd::text(&manifest, "revision")? != revision
        || !same(
            cmd::field(&manifest, "source")?,
            cmd::field(receipt, "previous_source")?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim archive manifest binding differs",
        ));
    }
    let refs = object_map(cmd::field(&manifest, "files")?)?;
    if refs.is_empty() || refs.len() > 64 {
        return Err(invalid());
    }
    let mut restored = LegacyPackage::new();
    let mut locations = Vec::new();
    let mut blobs = BTreeSet::new();
    for (name, binding) in refs {
        let name = name.as_str().ok_or(invalid())?;
        if name.is_empty() || name.contains('/') || matches!(name, "." | "..") {
            return Err(invalid());
        }
        cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let digest = cmd::text(binding, "sha256")?;
        if digest.len() != 71
            || !digest.starts_with("sha256:")
            || !digest[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(invalid());
        }
        let blob = format!("{}.blob", &digest[7..]);
        if cmd::text(binding, "blob")? != blob || !blobs.insert(blob.clone()) {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive blob path differs",
            ));
        }
        let bytes = raw.get(&blob).ok_or(invalid())?;
        if Digest256::of_bytes(bytes).to_prefixed() != digest
            || bytes.len() as u64 != cmd::integer(binding, "bytes")?
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive blob digest differs",
            ));
        }
        restored.insert(name.to_owned(), bytes.clone());
        locations.push((
            JsonString::from_utf8(name),
            cmd::object(vec![
                ("archive_path", cmd::string(&format!("{path}/{blob}"))),
                ("sha256", cmd::string(digest)),
                ("bytes", cmd::number(bytes.len() as u64)),
            ]),
        ));
    }
    if raw
        .keys()
        .any(|name| name != "manifest.json" && !blobs.contains(name))
        || blobs.len() + 1 != raw.len()
        || source_revisions::revision(&restored)? != revision
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim archive package digest differs",
        ));
    }
    Ok((restored, JsonValue::Object(locations)))
}

fn archive_package(
    current: &LegacyPackage,
    source_path: &str,
    previous: &JsonValue,
    revision: &str,
) -> SourceCommandResult<(String, LegacyPackage)> {
    let reference = archive_reference(cmd::text(previous, "id")?, revision)?;
    let mut refs = Vec::new();
    let mut archive = LegacyPackage::new();
    for (name, raw) in current {
        let digest = Digest256::of_bytes(raw);
        let blob = format!("{}.blob", digest.to_hex());
        refs.push((
            JsonString::from_utf8(name),
            cmd::object(vec![
                ("blob", cmd::string(&blob)),
                ("sha256", cmd::string(&digest.to_prefixed())),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        ));
        if archive.insert(blob, raw.clone()).is_some() {
            return Err(invalid());
        }
    }
    let manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_source_package_archive_v1"),
        ),
        ("source_path", cmd::string(source_path)),
        ("source", previous.clone()),
        ("revision", cmd::string(revision)),
        ("files", JsonValue::Object(refs)),
    ]);
    archive.insert("manifest.json".into(), cmd::published(&manifest)?);
    Ok((reference, archive))
}

fn form_name(source_path: &str, id: &str) -> SourceCommandResult<String> {
    let stem = source_path.strip_suffix(".jsonl").ok_or(invalid())?;
    Ok(format!(
        "{}.{}.human-forms.json",
        stem.rsplit('/').next().ok_or(invalid())?,
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}

fn package_form_name(source_path: &str, id: &str) -> SourceCommandResult<String> {
    form_name(source_path, id)
}

fn form_refs(set: Option<&JsonValue>) -> SourceCommandResult<Vec<JsonValue>> {
    set.map(|set| {
        cmd::array(set, "forms")?
            .iter()
            .map(source_forms::form_reference)
            .collect()
    })
    .unwrap_or_else(|| Ok(Vec::new()))
}

fn materializations(
    claim: &JsonValue,
    set: Option<&JsonValue>,
) -> SourceCommandResult<Vec<JsonValue>> {
    set.map(|value| source_forms::materialize_source_forms(claim, value))
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn package_revision(current: &LegacyPackage) -> SourceCommandResult<String> {
    source_revisions::revision(current)
}

fn validate_captured_request_bytes(raw: &[u8], request: &JsonValue) -> SourceCommandResult<()> {
    // historical.create stores canonical command JSON followed by one LF.
    // Its receipt binds those exact bytes; pretty source-form serialization
    // belongs to another output contract and cannot reinterpret this capture.
    let mut expected = cmd::canonical(request)?;
    expected.push(b'\n');
    if raw != expected {
        return Err(SourceCommandError::Conflict(
            "captured historical request bytes differ",
        ));
    }
    Ok(())
}

fn validate_capture(
    current: &LegacyPackage,
    owner: &Owner,
    worker: &mut CutWorkerSchemaExecutor,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue, Vec<JsonValue>)> {
    let receipt_raw =
        current
            .get("source-create-receipt.json")
            .ok_or(SourceCommandError::Denied(
                "historical Claim requires captured historical.create v2",
            ))?;
    if Digest256::of_bytes(receipt_raw).to_prefixed()
        != cmd::text(&owner.config, "creation_receipt_sha256")?
    {
        return Err(SourceCommandError::Denied(
            "historical Claim receipt digest differs",
        ));
    }
    if CAPTURE.iter().any(|name| !current.contains_key(*name)) {
        return Err(SourceCommandError::Denied(
            "historical Claim capture is incomplete",
        ));
    }
    let receipt = cmd::parse(receipt_raw)?;
    let request = cmd::parse(current.get("source-create-request.json").ok_or(invalid())?)?;
    let record = cmd::field(&request, "record")?.clone();
    let original_claims = cmd::array(&request, "claims")?.to_vec();
    let record_id = cmd::text(&record, "record_id")?;
    let record_type = cmd::text(&record, "record_type")?;
    cmd::exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "record",
            "claims",
            "forms",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_source",
            "expected_revision",
        ],
    )?;
    cmd::exact_keys(
        &receipt,
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
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !matches!(
            record_type,
            "historical-event" | "historical-process" | "historical-state"
        )
        || record_id != cmd::text(&owner.config, "historical_record_id")?
        || cmd::text(&request, "operation")? != "historical.create"
        || cmd::field(&request, "expected_source")? != &JsonValue::Null
        || cmd::field(&request, "expected_revision")? != &JsonValue::Null
        || cmd::text(&receipt, "schema_version")? != "tos_local_historical_create_receipt_v1"
        || cmd::text(&receipt, "command_id")? != cmd::text(&request, "command_id")?
        || cmd::text(&receipt, "request_digest")? != cmd::record_digest(&request)?.to_prefixed()
        || cmd::field(&receipt, "owner_configuration")?
            != cmd::field(&request, "expected_configuration")?
        || cmd::field(&receipt, "dependencies")? != cmd::field(&request, "expected_dependencies")?
        || !cmd::same(
            cmd::field(&receipt, "source")?,
            &source_forms::metadata_subject(&record)?,
        )?
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Denied(
            "captured historical.create receipt binding differs",
        ));
    }
    let record_path = cmd::text(&receipt, "source_path")?;
    let stream_path = cmd::text(&owner.config, "source_path")?;
    let parent = stream_path.rsplit_once('/').ok_or(invalid())?.0;
    if record_path.rsplit_once('/').map(|part| part.0) != Some(parent)
        || record_path.rsplit('/').next() != Some(&format!("{record_type}.json"))
    {
        return Err(SourceCommandError::Denied(
            "historical creation record path differs",
        ));
    }
    let mut originals = BTreeMap::new();
    for claim in &original_claims {
        let id = cmd::text(claim, "claim_id")?.to_owned();
        if !claim_id(&id) || originals.insert(id, claim.clone()).is_some() {
            return Err(SourceCommandError::Conflict(
                "historical creation repeats Claim identity",
            ));
        }
    }
    if !originals.contains_key(&owner.claim_id) {
        return Err(SourceCommandError::Denied(
            "delegated Claim absent from historical.create request",
        ));
    }
    let record_raw = current
        .get(&format!("{record_type}.json"))
        .ok_or(invalid())?;
    let current_record = cmd::parse(record_raw)?;
    schema(
        record_path,
        ctx,
        worker,
        &current_record,
        "ToS/contracts/historical-record.schema.json",
        &[
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/historical-record.schema.json",
        ],
        deadline,
        cancelled,
    )?;
    let _ = cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    let receipt_files = object_map(cmd::field(&receipt, "files")?)?;
    let record_form = format!("{record_type}.human-forms.json");
    let expected_files = BTreeSet::from([
        format!("{record_type}.json"),
        record_form,
        "historical-claims.jsonl".to_owned(),
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
    ]);
    if receipt_files
        .iter()
        .filter_map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>()
        != expected_files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "captured historical creation output closure differs",
        ));
    }
    let claim_stream = current.get(STREAM).ok_or(invalid())?;
    let rows = claim_rows(claim_stream)?;
    if rows.keys().cloned().collect::<BTreeSet<_>>()
        != originals.keys().cloned().collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "historical current Claim identities differ from captured request",
        ));
    }
    for (name, binding) in receipt_files {
        let name = name.as_str().ok_or(invalid())?;
        cmd::exact_keys(binding, &["sha256", "bytes"])?;
        if CAPTURE.contains(&name) {
            let raw = current.get(name).ok_or_else(invalid)?;
            if cmd::text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
                || cmd::integer(binding, "bytes")? != raw.len() as u64
            {
                return Err(SourceCommandError::Conflict(
                    "captured historical evidence fixity differs",
                ));
            }
        }
    }
    let request_raw = current.get("source-create-request.json").ok_or(invalid())?;
    validate_captured_request_bytes(request_raw, &request)?;
    for claim in &original_claims {
        if cmd::text(claim, "schema_version")? != "tos_historical_claim_v1"
            || cmd::text(claim, "subject_ref")? != record_id
            || cmd::integer(claim, "claim_version")? != 1
            || cmd::text(cmd::field(claim, "maker")?, "agent_ref")?
                != cmd::text(&receipt, "principal_id")?
        {
            return Err(SourceCommandError::Conflict(
                "captured historical Claim identity differs",
            ));
        }
    }
    let _ = (record_path, originals, rows);
    Ok((request, receipt, original_claims))
}

fn validate_config_claim(
    ctx: &CommandContext,
    owner: &Owner,
    claim: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    schema(
        cmd::text(&owner.config, "source_path")?,
        ctx,
        worker,
        claim,
        CLAIM_SCHEMA,
        CLAIM_SCHEMA_CONTRACTS,
        deadline,
        cancelled,
    )?;
    if cmd::text(claim, "schema_version")? != "tos_historical_claim_v1"
        || cmd::text(claim, "subject_ref")? != cmd::text(&owner.config, "historical_record_id")?
        || ![
            "historical_participant",
            "historical_place",
            "historical_work",
            "historical_dating",
        ]
        .contains(&cmd::text(claim, "predicate")?)
        || !["public", "public_metadata_only"].contains(&cmd::text(claim, "visibility")?)
    {
        return Err(SourceCommandError::Denied(
            "historical Claim schema or structural family differs",
        ));
    }
    let fields = source_forms::metadata_fields(claim)?;
    for field in fields {
        let change = source_forms::prepare_form_change(
            claim,
            None,
            "historical-claim-schema-check",
            "tos.form.historical-claim-schema-check",
            &field.id,
        )?;
        schema(
            cmd::text(&owner.config, "source_path")?,
            ctx,
            worker,
            cmd::field(&change, "form")?,
            "ToS/contracts/human-form.schema.json",
            &["ToS/contracts/human-form.schema.json"],
            deadline,
            cancelled,
        )?;
    }
    if cmd::field(claim, "qualifiers")?
        .object_get("display_fields")
        .and_then(|v| v.object_get("schema_version"))
        .and_then(JsonValue::as_str)
        == Some("tos_claim_display_fields_v1")
    {
        schema(
            cmd::text(&owner.config, "source_path")?,
            ctx,
            worker,
            cmd::field(claim, "qualifiers")?,
            "ToS/contracts/claim-display-fields.schema.json",
            &[
                "ToS/contracts/corpus-record.schema.json",
                "ToS/contracts/claim-display-fields.schema.json",
            ],
            deadline,
            cancelled,
        )?;
    }
    Ok(())
}

/// Validate the original historical.create event and its Claim-owned output
/// bindings. The neighboring Historical Record and its forms keep their own
/// current and retained-byte validation.
fn validate_historical_creation_outputs(
    ctx: &CommandContext,
    current: &LegacyPackage,
    source_path: &str,
    record_path: &str,
    request: &JsonValue,
    receipt: &JsonValue,
    original_claims: &[JsonValue],
    initial_stream: &[u8],
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let record = cmd::field(request, "record")?;
    let record_type = cmd::text(record, "record_type")?;
    if !matches!(
        record_type,
        "historical-event" | "historical-process" | "historical-state"
    ) {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record type differs",
        ));
    }
    let record_name = format!("{record_type}.json");
    let forms_name = format!("{record_type}.human-forms.json");
    let base = source_path.rsplit_once('/').ok_or(invalid())?.0;
    if record_path.rsplit_once('/').map(|parts| parts.0) != Some(base)
        || record_path.rsplit('/').next() != Some(record_name.as_str())
    {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record output path differs",
        ));
    }

    let event_raw = current
        .get("source-create-provenance.jsonl")
        .ok_or(invalid())?;
    let mut event_lines = event_raw.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if event_lines.last().is_some_and(|line| line.is_empty()) {
        event_lines.pop();
    }
    if event_lines.len() != 1 || event_lines[0].iter().all(u8::is_ascii_whitespace) {
        return Err(SourceCommandError::Conflict(
            "captured historical creation requires one provenance event",
        ));
    }
    let event_bytes = event_lines[0].strip_suffix(b"\r").unwrap_or(event_lines[0]);
    let event = cmd::parse(event_bytes)?;
    schema(
        &format!("{base}/source-create-provenance.jsonl"),
        ctx,
        worker,
        &event,
        "ToS/contracts/provenance-event-v2.schema.json",
        &["ToS/contracts/provenance-event-v2.schema.json"],
        deadline,
        cancelled,
    )?;
    let event_json: serde_json::Value = serde_json::from_slice(event_bytes)
        .map_err(|_| SourceCommandError::Invalid("captured historical provenance JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&event_json, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("captured historical provenance semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "captured historical provenance semantics",
        ));
    }
    let event_id = cmd::text(&event, "event_id")?;
    for claim in original_claims {
        if cmd::text(claim, "provenance_event_ref")? != event_id {
            return Err(SourceCommandError::Conflict(
                "captured historical Claim event identity differs",
            ));
        }
    }

    let expected_outputs = BTreeSet::from([
        record_path.to_owned(),
        source_path.to_owned(),
        format!("{base}/{forms_name}"),
    ]);
    let output_entities = cmd::array(cmd::field(&event, "entities")?, "outputs")?;
    let observed_outputs = output_entities
        .iter()
        .map(|entity| cmd::text(entity, "entity_ref").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if observed_outputs != expected_outputs
        || output_entities.len() != expected_outputs.len()
        || cmd::text(
            cmd::field(cmd::field(&event, "method")?, "procedure")?,
            "name",
        )? != "historical-source-metadata-serialization"
        || cmd::text(cmd::field(&event, "record_binding")?, "manifest_ref")?
            != format!("{base}/source-create-receipt.json")
        || cmd::field(
            cmd::field(&event, "rights_and_visibility")?,
            "publication_authorized",
        )? != &JsonValue::Bool(false)
        || !cmd::array(cmd::field(&event, "review_and_authority")?, "accepted_uses")?.is_empty()
        || cmd::field(
            cmd::field(&event, "review_and_authority")?,
            "promotion_authorized",
        )? != &JsonValue::Bool(false)
        || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Conflict(
            "captured historical event is not the exact non-admitting serializer",
        ));
    }

    let files = object_map(cmd::field(receipt, "files")?)?;
    let expected_names = BTreeSet::from([
        record_name.as_str(),
        forms_name.as_str(),
        STREAM,
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
    ]);
    if files
        .iter()
        .filter_map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>()
        != expected_names
    {
        return Err(SourceCommandError::Conflict(
            "captured historical receipt file closure differs",
        ));
    }
    for (_, binding) in files {
        cmd::exact_keys(binding, &["sha256", "bytes"])?;
    }
    for entity in output_entities {
        let entity_ref = cmd::text(entity, "entity_ref")?;
        let name = entity_ref.rsplit('/').next().ok_or(invalid())?;
        let binding = cmd::field(cmd::field(receipt, "files")?, name)?;
        if cmd::text(binding, "sha256")? != format!("sha256:{}", cmd::text(entity, "sha256")?)
            || cmd::integer(binding, "bytes")? != cmd::integer(entity, "size_bytes")?
        {
            return Err(SourceCommandError::Conflict(
                "historical provenance output fixity differs",
            ));
        }
    }
    for (name, raw) in [
        (STREAM, initial_stream),
        (
            "source-create-request.json",
            current
                .get("source-create-request.json")
                .ok_or(invalid())?
                .as_slice(),
        ),
        (
            "source-create-environment.json",
            current
                .get("source-create-environment.json")
                .ok_or(invalid())?
                .as_slice(),
        ),
        ("source-create-provenance.jsonl", event_raw.as_slice()),
    ] {
        let binding = cmd::field(cmd::field(receipt, "files")?, name)?;
        if cmd::text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "captured historical Claim origin output bytes differ",
            ));
        }
    }
    Ok(())
}

/// Replay the captured historical.create origin and its independent record
/// revision chain. Claim history and HistoricalRecord history are separate
/// ledgers even though the creation receipt originally covered both outputs.
fn verify_record_origin(
    ctx: &CommandContext,
    current: &LegacyPackage,
    owner: &Owner,
    history: &History,
    original_claims: &[JsonValue],
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let request = cmd::parse(current.get("source-create-request.json").ok_or(invalid())?)?;
    let receipt = cmd::parse(current.get("source-create-receipt.json").ok_or(invalid())?)?;
    let record = cmd::field(&request, "record")?.clone();
    let record_type = cmd::text(&record, "record_type")?;
    let record_id = cmd::text(&record, "record_id")?;
    let record_path = cmd::text(&receipt, "source_path")?;
    let record_name = format!("{record_type}.json");
    let forms_name = format!("{record_type}.human-forms.json");
    let record_raw = current.get(&record_name).ok_or(invalid())?;
    let current_record = cmd::parse(record_raw)?;
    let original_subject = subject(&record)?;
    if record_id != cmd::text(&owner.config, "historical_record_id")?
        || record_path.rsplit_once('/').map(|pair| pair.1) != Some(record_name.as_str())
        || record_path.rsplit_once('/').map(|pair| pair.0)
            != cmd::text(&owner.config, "source_path")?
                .rsplit_once('/')
                .map(|pair| pair.0)
        || cmd::text(&receipt, "schema_version")? != "tos_local_historical_create_receipt_v1"
        || !same(cmd::field(&receipt, "source")?, &original_subject)?
        || cmd::field(&receipt, "owner_configuration")?
            != cmd::field(&request, "expected_configuration")?
        || cmd::field(&receipt, "dependencies")? != cmd::field(&request, "expected_dependencies")?
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record origin identity differs",
        ));
    }
    schema(
        record_path,
        ctx,
        worker,
        &record,
        "ToS/contracts/historical-record.schema.json",
        &[
            "ToS/contracts/corpus-record.schema.json",
            "ToS/contracts/historical-record.schema.json",
        ],
        deadline,
        cancelled,
    )?;

    let raw_history = current.get(SOURCE_HISTORY);
    let record_history = if let Some(raw) = raw_history {
        cmd::parse(raw)?
    } else {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_revision_history_v1"),
            ),
            ("record_id", cmd::string(record_id)),
            ("receipts", JsonValue::Array(vec![])),
        ])
    };
    cmd::exact_keys(
        &record_history,
        &["schema_version", "record_id", "receipts"],
    )?;
    let history_schema = cmd::text(&record_history, "schema_version")?;
    let record_receipts = cmd::array(&record_history, "receipts")?;
    if !matches!(
        history_schema,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || cmd::text(&record_history, "record_id")? != record_id
        || record_receipts.len() > 128
        || raw_history.is_some() && record_receipts.is_empty()
    {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record revision history differs",
        ));
    }

    let mut record_config = owner.config.clone();
    cmd::set(&mut record_config, "source_path", cmd::string(record_path))?;
    let mut previous = original_subject.clone();
    let mut initial_record_raw = record_raw.clone();
    let mut seen_commands = BTreeSet::new();
    for (index, retained) in record_receipts.iter().enumerate() {
        let selected = retained.object_get("publication").is_some();
        let mut receipt_keys = vec![
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
            receipt_keys.push("publication");
        }
        cmd::exact_keys(retained, &receipt_keys)?;
        if selected && history_schema != "tos_source_revision_history_v2" {
            return Err(SourceCommandError::Conflict(
                "selected Historical Record revision requires v2 history",
            ));
        }
        let revision_request = cmd::field(retained, "request")?;
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "fields",
            "forms",
            "reason",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ];
        if selected {
            request_keys.push("expected_publication");
        }
        cmd::exact_keys(revision_request, &request_keys)?;
        let fields = cmd::field(revision_request, "fields")?;
        let mut changed = object_map(fields)?
            .iter()
            .map(|(key, _)| key.as_str().map(str::to_owned).ok_or(invalid()))
            .collect::<SourceCommandResult<Vec<_>>>()?;
        changed.sort();
        let changed = JsonValue::Array(changed.iter().map(|key| cmd::string(key)).collect());
        let command_id = cmd::text(retained, "command_id")?;
        let previous_source = cmd::field(retained, "previous_source")?;
        let next_source = cmd::field(retained, "source")?;
        let previous_revision = cmd::text(retained, "previous_revision")?;
        let expected_archive = archive_reference(record_id, previous_revision)?;
        if !(1..=256).contains(&command_id.chars().count())
            || !seen_commands.insert(command_id.to_owned())
            || cmd::text(revision_request, "schema_version")? != "tos_local_source_command_v1"
            || cmd::text(revision_request, "operation")? != "record.revise"
            || cmd::text(retained, "request_digest")?
                != cmd::record_digest(revision_request)?.to_prefixed()
            || cmd::field(retained, "command_id")? != cmd::field(revision_request, "command_id")?
            || !same(previous_source, &previous)?
            || !same(
                previous_source,
                cmd::field(revision_request, "expected_source")?,
            )?
            || cmd::field(retained, "previous_revision")?
                != cmd::field(revision_request, "expected_revision")?
            || cmd::field(retained, "owner_configuration")?
                != cmd::field(revision_request, "expected_configuration")?
            || cmd::field(retained, "dependencies")?
                != cmd::field(revision_request, "expected_dependencies")?
            || cmd::field(retained, "reason")? != cmd::field(revision_request, "reason")?
            || cmd::field(retained, "changed_fields")? != &changed
            || cmd::field(retained, "grants_admission")? != &JsonValue::Bool(false)
            || cmd::text(previous_source, "id")? != record_id
            || cmd::text(next_source, "id")? != record_id
            || cmd::integer(previous_source, "version")?.checked_add(1)
                != Some(cmd::integer(next_source, "version")?)
            || cmd::text(retained, "archive_path")? != expected_archive
        {
            return Err(SourceCommandError::Conflict(
                "broken Historical Record revision receipt",
            ));
        }
        cmd::validate_instant(cmd::text(retained, "recorded_at")?)?;
        if selected {
            let publication = cmd::field(retained, "publication")?;
            cmd::exact_keys(
                publication,
                &["protocol", "transaction_id", "selected_files"],
            )?;
            let names = cmd::array(publication, "selected_files")?
                .iter()
                .map(|value| value.as_str().ok_or(invalid()).map(str::to_owned))
                .collect::<SourceCommandResult<Vec<_>>>()?;
            let mut expected_names = vec![
                record_name.clone(),
                forms_name.clone(),
                SOURCE_HISTORY.to_owned(),
            ];
            expected_names.sort();
            if cmd::text(publication, "protocol")? != "tos_selected_source_metadata_v1"
                || cmd::text(publication, "transaction_id")?.is_empty()
                || names != expected_names
                || cmd::field(retained, "publication")?
                    .object_get("selected_files")
                    .and_then(JsonValue::as_array)
                    .is_none_or(|values| values.len() != 3)
            {
                return Err(SourceCommandError::Conflict(
                    "selected Historical Record scope differs",
                ));
            }
        }
        let mut archive_raw = reader.read_archive(&expected_archive, deadline, cancelled)?;
        let (archived, _) =
            decode_archive(std::mem::take(&mut archive_raw), &record_config, retained)?;
        if index == 0 {
            initial_record_raw = archived.get(&record_name).ok_or(invalid())?.clone();
        }
        let prior_record = cmd::parse(archived.get(&record_name).ok_or(invalid())?)?;
        if !same(&subject(&prior_record)?, previous_source)? {
            return Err(SourceCommandError::Conflict(
                "Historical Record archive subject differs",
            ));
        }
        let prior_history = archived
            .get(SOURCE_HISTORY)
            .map(|raw| cmd::parse(raw))
            .transpose()?;
        let prior_receipts = if let Some(prior_history) = prior_history {
            cmd::exact_keys(&prior_history, &["schema_version", "record_id", "receipts"])?;
            if cmd::text(&prior_history, "record_id")? != record_id {
                return Err(SourceCommandError::Conflict(
                    "Historical Record archived history identity differs",
                ));
            }
            cmd::array(&prior_history, "receipts")?.to_vec()
        } else {
            Vec::new()
        };
        let prefix_matches = prior_receipts.len() == index
            && prior_receipts
                .iter()
                .zip(&record_receipts[..index])
                .map(|(actual, expected)| same(actual, expected))
                .collect::<SourceCommandResult<Vec<_>>>()?
                .into_iter()
                .all(|value| value);
        if !prefix_matches {
            return Err(SourceCommandError::Conflict(
                "Historical Record archived history prefix differs",
            ));
        }
        let mut successor = prior_record.clone();
        for (key, value) in object_map(fields)? {
            cmd::set(
                &mut successor,
                key.as_str().ok_or(invalid())?,
                value.clone(),
            )?;
        }
        cmd::set(
            &mut successor,
            "record_version",
            cmd::number(
                cmd::integer(&prior_record, "record_version")?
                    .checked_add(1)
                    .ok_or(invalid())?,
            ),
        )?;
        schema(
            record_path,
            ctx,
            worker,
            &successor,
            "ToS/contracts/historical-record.schema.json",
            &[
                "ToS/contracts/corpus-record.schema.json",
                "ToS/contracts/historical-record.schema.json",
            ],
            deadline,
            cancelled,
        )?;
        if !same(&subject(&successor)?, next_source)? {
            return Err(SourceCommandError::Conflict(
                "Historical Record receipt successor differs",
            ));
        }
        previous = next_source.clone();
        if !selected {
            for capture in CAPTURE {
                if archived.get(*capture) != current.get(*capture) {
                    return Err(SourceCommandError::Conflict(
                        "historical creation capture changed across record history",
                    ));
                }
            }
        }
    }
    if !same(&subject(&current_record)?, &previous)? {
        return Err(SourceCommandError::Conflict(
            "current Historical Record is not retained history head",
        ));
    }

    let forms_raw = current.get(&forms_name).ok_or(invalid())?;
    let forms = cmd::parse(forms_raw)?;
    schema(
        record_path,
        ctx,
        worker,
        &forms,
        "ToS/contracts/human-form-set.schema.json",
        &["ToS/contracts/human-form-set.schema.json"],
        deadline,
        cancelled,
    )?;
    let form_lineage = tos_validation::source_forms::inspect_lineage_raw(forms_raw)
        .map_err(|_| SourceCommandError::Invalid("Historical Record form lineage"))?;
    if form_lineage.subject_id != record_id {
        return Err(SourceCommandError::Conflict(
            "Historical Record forms belong to another subject",
        ));
    }
    let form_lock = format!(".{forms_name}.writer.lock");
    if current.get(&form_lock).is_some_and(|raw| !raw.is_empty()) {
        return Err(SourceCommandError::Conflict(
            "Historical Record form writer lock contains data",
        ));
    }
    let selections = cmd::array(&request, "forms")?;
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record form selections are invalid",
        ));
    }
    let mut changes = Vec::new();
    let mut selected_ids = BTreeSet::new();
    for selection in selections {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let id = cmd::text(selection, "form_id")?;
        if !selected_ids.insert(id.to_owned()) {
            return Err(SourceCommandError::Conflict(
                "captured Historical Record repeats form identity",
            ));
        }
        changes.push(source_forms::prepare_form_change(
            &record,
            None,
            cmd::text(&receipt, "principal_id")?,
            id,
            cmd::text(selection, "field_id")?,
        )?);
    }
    let initial_forms = source_forms::apply_form_changes(None, &original_subject, &changes)?;
    let initial_views = source_forms::materialize_source_forms(&record, &initial_forms)?;
    if initial_views
        .iter()
        .any(|view| optional_text(view, "state") != Some("ready"))
        || !initial_views
            .iter()
            .any(|view| optional_text(view, "role") == Some("name"))
    {
        return Err(SourceCommandError::Conflict(
            "captured Historical Record initial forms are not source-ready",
        ));
    }
    let retained_initial = cmd::array(&initial_forms, "forms")?;
    let retained_all = cmd::array(&forms, "forms")?
        .iter()
        .chain(cmd::array(&forms, "prior_forms")?)
        .collect::<Vec<_>>();
    for form in retained_initial {
        let mut found = false;
        for retained in &retained_all {
            found |= same(retained, form)?;
        }
        if !found {
            return Err(SourceCommandError::Conflict(
                "initial Historical Record form is not retained",
            ));
        }
    }
    let original_form_bytes = if !cmd::array(&forms, "prior_forms")?.is_empty()
        || forms
            .object_get("growth_history")
            .and_then(JsonValue::as_array)
            .is_some_and(|values| !values.is_empty())
    {
        cmd::published(&initial_forms)?
    } else {
        forms_raw.clone()
    };
    let original_record = cmd::parse(&initial_record_raw)?;
    if !same(&subject(&original_record)?, &original_subject)? {
        return Err(SourceCommandError::Conflict(
            "Historical Record history does not start at its captured subject",
        ));
    }

    for claim in original_claims {
        validate_config_claim(ctx, owner, claim, worker, deadline, cancelled)?;
    }
    validate_historical_creation_outputs(
        ctx,
        current,
        cmd::text(&owner.config, "source_path")?,
        record_path,
        &request,
        &receipt,
        original_claims,
        &history.initial_stream,
        worker,
        deadline,
        cancelled,
    )?;
    for (name, raw) in [
        (record_name.as_str(), initial_record_raw.as_slice()),
        (forms_name.as_str(), original_form_bytes.as_slice()),
    ] {
        let binding = cmd::field(cmd::field(&receipt, "files")?, name)?;
        if cmd::text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "captured historical origin output bytes differ",
            ));
        }
    }
    Ok(())
}

fn validate_forms_scope(
    config: &JsonValue,
    fields: &[String],
    changes: &[JsonValue],
    set: Option<&JsonValue>,
) -> SourceCommandResult<()> {
    let allowed_ids = texts(config, "allowed_form_ids", 32)?;
    let allowed_fields = fields.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for change in changes {
        let form = cmd::field(change, "form")?;
        let form_id = cmd::text(form, "form_id")?;
        let predecessor = cmd::field(change, "expected_form")?;
        if !allowed_ids.iter().any(|id| id == form_id)
            || !ids.insert(form_id.to_owned())
            || !same(predecessor, cmd::field(form, "revises")?)?
        {
            return Err(SourceCommandError::Denied(
                "historical Claim form identity or predecessor scope",
            ));
        }
        let operation = cmd::text(change, "operation")?;
        if (operation == "form.create") != predecessor.is_null()
            || operation == "form.create" && cmd::integer(form, "form_version")? != 1
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim form operation differs from lineage",
            ));
        }
        if cmd::text(cmd::field(form, "content")?, "kind")? != "source-copy" {
            return Err(SourceCommandError::Denied(
                "historical Claim delegation permits exact source copies",
            ));
        }
        let slot = cmd::text(cmd::field(form, "content")?, "slot")?;
        let binding = cmd::field(cmd::field(form, "bindings")?, slot)?;
        let role = cmd::text(form, "role")?;
        let pointer = cmd::text(binding, "pointer")?;
        if !allowed_fields.iter().any(|field| match *field {
            "claim.statement" => role == "statement" && pointer == "/qualifiers/statement",
            "claim.name" => role == "name" && pointer == "/qualifiers/display_fields/name/text",
            "claim.caption" => {
                role == "caption" && pointer == "/qualifiers/display_fields/caption/text"
            }
            "claim.hover" => role == "hover" && pointer == "/qualifiers/display_fields/hover/text",
            _ => false,
        }) {
            return Err(SourceCommandError::Denied(
                "historical Claim source-copy field outside delegation",
            ));
        }
    }
    if let Some(set) = set {
        for form in cmd::array(set, "forms")?
            .iter()
            .chain(cmd::array(set, "prior_forms")?)
        {
            if ids.contains(cmd::text(form, "form_id")?) {
                // Only current forms selected for this exact rebind and exact retained predecessors
                // can be included; their individual lineage was checked by the HumanForm kernel.
                let _ = source_forms::form_reference(form)?;
            }
        }
    }
    Ok(())
}

fn validate_history<'a>(
    ctx: &CommandContext,
    current: &LegacyPackage,
    config: &JsonValue,
    originals: &[JsonValue],
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<History> {
    let path = cmd::text(config, "source_path")?;
    let history = if let Some(raw) = current.get(HISTORY) {
        cmd::parse(raw)?
    } else {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_revision_history_v1"),
            ),
            ("source_path", cmd::string(path)),
            ("receipts", JsonValue::Array(Vec::new())),
        ])
    };
    cmd::exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    if cmd::text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || cmd::text(&history, "source_path")? != path
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim history schema or path differs",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "historical Claim revision capacity",
        ));
    }
    let current_raw = current.get(STREAM).ok_or(invalid())?;
    let current_rows = claim_rows(current_raw)?;
    if receipts.is_empty() {
        for claim in current_rows.values() {
            if cmd::integer(claim, "claim_version")? != 1 {
                return Err(SourceCommandError::Conflict(
                    "historical Claim stream lacks retained revision history",
                ));
            }
        }
    }
    let mut expected: Option<Vec<u8>> = None;
    let mut commands = BTreeSet::new();
    let mut initial_stream = current_raw.clone();
    for (index, receipt) in receipts.iter().enumerate() {
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
                "source_bindings",
                "changed_fields",
                "forms",
                "grants_admission",
                "request",
            ],
        )?;
        let request = cmd::field(receipt, "request")?;
        validate_request_shape(request, true)?;
        let command_id = cmd::text(receipt, "command_id")?;
        cmd::validate_instant(cmd::text(receipt, "recorded_at")?)?;
        if !commands.insert(command_id.to_owned())
            || cmd::text(request, "operation")? != "claim.revise"
            || cmd::text(receipt, "request_digest")? != cmd::record_digest(request)?.to_prefixed()
            || cmd::field(receipt, "command_id")? != cmd::field(request, "command_id")?
            || !same(
                cmd::field(receipt, "previous_source")?,
                cmd::field(request, "expected_source")?,
            )?
            || cmd::field(receipt, "previous_revision")?
                != cmd::field(request, "expected_revision")?
            || cmd::field(receipt, "owner_configuration")?
                != cmd::field(request, "expected_configuration")?
            || cmd::field(receipt, "dependencies")? != cmd::field(request, "expected_dependencies")?
            || !same(
                cmd::field(receipt, "source_bindings")?,
                cmd::field(request, "expected_inputs")?,
            )?
            || cmd::field(receipt, "reason")? != cmd::field(request, "reason")?
            || cmd::field(receipt, "changed_fields")?
                != &JsonValue::Array(vec![cmd::string("qualifiers")])
            || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim revision receipt is broken",
            ));
        }
        let previous = cmd::field(receipt, "previous_source")?;
        let id = cmd::text(previous, "id")?;
        let archive_ref = cmd::text(receipt, "archive_path")?;
        if archive_ref != archive_reference(id, cmd::text(receipt, "previous_revision")?)? {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive locator differs",
            ));
        }
        let archive_raw = reader.read_archive(archive_ref, deadline, cancelled)?;
        let (archived, _) = decode_archive(archive_raw, config, receipt)?;
        verify_archived_form_result(
            ctx,
            &archived,
            config,
            receipt,
            cmd::text(receipt, "principal_id")?,
            worker,
            deadline,
            cancelled,
        )?;
        for name in CAPTURE {
            if archived.get(*name) != current.get(*name) {
                return Err(SourceCommandError::Conflict(
                    "historical creation capture changed across Claim history",
                ));
            }
        }
        let before_raw = archived.get(STREAM).ok_or(invalid())?;
        if index == 0 {
            initial_stream = before_raw.clone();
        }
        if expected.as_ref().is_some_and(|prior| prior != before_raw) {
            return Err(SourceCommandError::Conflict(
                "historical Claim stream changed outside retained sequence",
            ));
        }
        let before_rows = claim_rows(before_raw)?;
        if expected.is_none() {
            for claim in before_rows.values() {
                if cmd::integer(claim, "claim_version")? != 1 {
                    return Err(SourceCommandError::Conflict(
                        "historical Claim history does not start from initial claims",
                    ));
                }
            }
        }
        let prior = before_rows.get(id).ok_or(invalid())?;
        if !same(&subject(prior)?, previous)? {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive subject differs",
            ));
        }
        let fields = cmd::field(request, "fields")?;
        let revised = advance(prior, fields)?;
        if !same(&subject(&revised)?, cmd::field(receipt, "source")?)?
            || cmd::text(cmd::field(receipt, "source")?, "id")? != id
            || cmd::integer(previous, "version")?.checked_add(1)
                != Some(cmd::integer(cmd::field(receipt, "source")?, "version")?)
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim receipt successor differs",
            ));
        }
        let next = replace_row(before_raw, &revised)?;
        if index + 1 == receipts.len() && current_raw != &next {
            return Err(SourceCommandError::Conflict(
                "current historical Claim stream is not retained head",
            ));
        }
        expected = Some(next);
        let form_target = package_form_name(path, id)?;
        let current_form = current
            .get(&form_target)
            .ok_or(SourceCommandError::Conflict(
                "historical Claim revision form result is not retained",
            ))?;
        let payload = cmd::parse(current_form)?;
        let lineage = tos_validation::source_forms::inspect_lineage_raw(current_form)
            .map_err(|_| SourceCommandError::Invalid("historical Claim form result lineage"))?;
        if lineage.subject_id != id {
            return Err(SourceCommandError::Conflict(
                "historical Claim forms belong to another subject",
            ));
        }
        let retained = cmd::array(&payload, "forms")?
            .iter()
            .chain(cmd::array(&payload, "prior_forms")?)
            .map(source_forms::form_reference)
            .collect::<SourceCommandResult<Vec<_>>>()?;
        for result in cmd::array(receipt, "forms")? {
            let mut found = false;
            for candidate in &retained {
                found |= same(candidate, result)?;
            }
            if !found {
                return Err(SourceCommandError::Conflict(
                    "Claim correction form result is not retained",
                ));
            }
        }
        let mut initial_identity = false;
        for claim in originals {
            initial_identity |= cmd::text(claim, "claim_id")? == id;
        }
        if !initial_identity {
            return Err(SourceCommandError::Conflict(
                "retained revision is outside historical creation package",
            ));
        }
    }
    if let Some(expected) = expected {
        if current_raw != &expected {
            return Err(SourceCommandError::Conflict(
                "historical Claim stream differs from retained revision head",
            ));
        }
    }
    Ok(History {
        payload: history,
        initial_stream,
    })
}

fn verify_archived_form_result(
    ctx: &CommandContext,
    archived: &LegacyPackage,
    config: &JsonValue,
    receipt: &JsonValue,
    principal_id: &str,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let path = cmd::text(config, "source_path")?;
    let previous = cmd::field(receipt, "previous_source")?;
    let id = cmd::text(previous, "id")?;
    let before_rows = claim_rows(archived.get(STREAM).ok_or(invalid())?)?;
    let before = before_rows.get(id).ok_or(invalid())?;
    let revised = advance(
        before,
        cmd::field(cmd::field(receipt, "request")?, "fields")?,
    )?;
    let form_name = package_form_name(path, id)?;
    let prior_raw = archived.get(&form_name);
    let prior = prior_raw.map(|raw| cmd::parse(raw)).transpose()?;
    if let (Some(raw), Some(set)) = (prior_raw, prior.as_ref()) {
        schema(
            path,
            ctx,
            worker,
            set,
            "ToS/contracts/human-form-set.schema.json",
            &["ToS/contracts/human-form-set.schema.json"],
            deadline,
            cancelled,
        )?;
        let lineage = tos_validation::source_forms::inspect_lineage_raw(raw)
            .map_err(|_| SourceCommandError::Invalid("archived historical Claim form history"))?;
        if lineage.subject_id != id {
            return Err(SourceCommandError::Conflict(
                "archived Claim forms belong to another subject",
            ));
        }
        let _ = source_forms::materialize_source_forms(before, set)?;
    }
    let request = cmd::field(receipt, "request")?;
    let selectors = cmd::array(request, "forms")?;
    if selectors.is_empty() || selectors.len() > 32 {
        return Err(SourceCommandError::Conflict(
            "retained Claim revision form selections are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut selected_statement = false;
    let mut changes = Vec::new();
    let all_fields = vec![
        "claim.statement".to_owned(),
        "claim.name".to_owned(),
        "claim.caption".to_owned(),
        "claim.hover".to_owned(),
    ];
    for selection in selectors {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let form_id = cmd::text(selection, "form_id")?;
        let field_id = cmd::text(selection, "field_id")?;
        if !ids.insert(form_id.to_owned()) || !all_fields.iter().any(|field| field == field_id) {
            return Err(SourceCommandError::Conflict(
                "retained Claim revision form selector is invalid",
            ));
        }
        selected_statement |= field_id == "claim.statement";
        changes.push(source_forms::prepare_form_change(
            &revised,
            prior.as_ref(),
            principal_id,
            form_id,
            field_id,
        )?);
    }
    if !selected_statement {
        return Err(SourceCommandError::Conflict(
            "retained Claim revision omitted the complete statement form",
        ));
    }
    if let Some(prior) = &prior {
        for form in cmd::array(prior, "forms")? {
            if !ids.contains(cmd::text(form, "form_id")?) {
                return Err(SourceCommandError::Conflict(
                    "retained Claim revision omitted a current form rebind",
                ));
            }
        }
    }
    let mut history_config = cmd::object(vec![]);
    cmd::set(
        &mut history_config,
        "allowed_form_ids",
        JsonValue::Array(ids.iter().map(|id| cmd::string(id)).collect()),
    )?;
    validate_forms_scope(&history_config, &all_fields, &changes, prior.as_ref())?;
    let expected = changes
        .iter()
        .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let actual = cmd::array(receipt, "forms")?;
    let mut references_match = actual.len() == expected.len();
    for (actual, expected) in actual.iter().zip(&expected) {
        references_match &= same(actual, expected)?;
    }
    if !references_match {
        return Err(SourceCommandError::Conflict(
            "retained Claim form references differ from request replay",
        ));
    }
    let _ = source_forms::apply_form_changes(prior.as_ref(), &subject(&revised)?, &changes)?;
    Ok(())
}

fn creation_lineage(
    current: &LegacyPackage,
    owner: &Owner,
    original_claims: &[JsonValue],
    history: &History,
) -> SourceCommandResult<()> {
    let mut original = BTreeMap::new();
    for claim in original_claims {
        let id = cmd::text(claim, "claim_id")?.to_owned();
        if original.insert(id, claim).is_some() {
            return Err(SourceCommandError::Conflict(
                "historical creation repeats Claim identity",
            ));
        }
    }
    let expected_stream = original_claims
        .iter()
        .try_fold(Vec::new(), |mut bytes, claim| {
            bytes.extend(cmd::canonical(claim)?);
            bytes.push(b'\n');
            Ok::<_, SourceCommandError>(bytes)
        })?;
    if history.initial_stream != expected_stream
        || claim_rows(current.get(STREAM).ok_or(invalid())?)?
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            != original.keys().cloned().collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim lineage differs from its original source stream",
        ));
    }
    let creation_receipt = cmd::parse(current.get("source-create-receipt.json").ok_or(invalid())?)?;
    let receipt_files = object_map(cmd::field(&creation_receipt, "files")?)?;
    let record_request = cmd::parse(current.get("source-create-request.json").ok_or(invalid())?)?;
    let record_type = cmd::text(cmd::field(&record_request, "record")?, "record_type")?;
    let record_form = format!("{record_type}.human-forms.json");
    let expected_original_files = BTreeSet::from([
        STREAM.to_owned(),
        format!("{record_type}.json"),
        record_form.clone(),
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
    ]);
    if receipt_files
        .iter()
        .filter_map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>()
        != expected_original_files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "captured historical creation output closure differs",
        ));
    }
    for (name, binding) in receipt_files {
        let name = name.as_str().ok_or(invalid())?;
        let raw = if name == STREAM {
            history.initial_stream.as_slice()
        } else if name == "source-create-request.json"
            || name == "source-create-environment.json"
            || name == "source-create-provenance.jsonl"
        {
            current.get(name).ok_or(invalid())?.as_slice()
        } else {
            // The Historical Record and its forms keep their own current
            // revision/form lineages and are outside this Claim owner.
            continue;
        };
        if cmd::text(binding, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(binding, "bytes")? != raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "captured historical creation fixity differs",
            ));
        }
    }
    let mut allowed = CAPTURE
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    allowed.insert(STREAM.into());
    allowed.insert(format!("{record_type}.json"));
    allowed.insert(record_form.clone());
    allowed.insert(format!(".{record_form}.writer.lock"));
    allowed.insert("source-revision-history.json".into());
    for id in original.keys() {
        let name = package_form_name(cmd::text(&owner.config, "source_path")?, id)?;
        allowed.insert(name.clone());
        allowed.insert(format!(".{name}.writer.lock"));
        if let Some(raw) = current.get(&name) {
            let payload = cmd::parse(raw)?;
            let lineage = tos_validation::source_forms::inspect_lineage_raw(raw)
                .map_err(|_| SourceCommandError::Invalid("historical Claim form lineage"))?;
            if lineage.subject_id != *id {
                return Err(SourceCommandError::Conflict(
                    "historical Claim form subject differs",
                ));
            }
            let _ = payload;
        }
        if current
            .get(&format!(".{name}.writer.lock"))
            .is_some_and(|bytes| !bytes.is_empty())
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim writer lock contains bytes",
            ));
        }
    }
    allowed.insert(HISTORY.into());
    if current.keys().any(|name| !allowed.contains(name)) {
        return Err(SourceCommandError::Conflict(
            "historical Claim package contains an unbound member",
        ));
    }
    Ok(())
}

fn validate_cut(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    path: &RelativePath,
    current: &LegacyPackage,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    ctx.check()?;
    if cut.revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "historical Claim source cut changed",
        ));
    }
    let bytes = ctx.file(path)?.ok_or(SourceCommandError::Unsupported(
        "historical Claim stream absent from source cut",
    ))?;
    if current.get(STREAM).is_none_or(|actual| actual != bytes) {
        return Err(SourceCommandError::Conflict(
            "historical Claim package differs from source cut",
        ));
    }
    let (digest, size) = cut
        .member_metadata(path)?
        .ok_or(SourceCommandError::Conflict(
            "historical Claim stream absent from anchored source cut",
        ))?;
    if digest != Digest256::of_bytes(bytes) || size != bytes.len() as u64 {
        return Err(SourceCommandError::Conflict(
            "historical Claim source cut descriptor differs",
        ));
    }
    let member = cut.read_member(ctx.base_revision, path, 1_048_576, deadline, cancelled)?;
    if member != bytes {
        return Err(SourceCommandError::Conflict(
            "historical Claim secure source read differs",
        ));
    }
    Ok(())
}

fn read_dependencies(owner: &Owner) -> Vec<SourceFile> {
    owner.reads.clone()
}

fn result_base(
    owner: &Owner,
    current: &LegacyPackage,
    claim: &JsonValue,
    set: Option<&JsonValue>,
    receipt: Option<JsonValue>,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let operations = if cmd::text(&owner.config, "schema_version")? == REVISION_OWNER {
        vec![
            "describe",
            "prepare-revise",
            "claim.revise",
            "inspect-version",
        ]
    } else {
        vec!["describe", "prepare", "apply"]
    };
    let mut fields = vec![
        (
            "schema_version",
            cmd::string("tos_local_claim_revision_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&owner.config_digest)),
        (
            "source_path",
            cmd::field(&owner.config, "source_path")?.clone(),
        ),
        ("source", subject(claim)?),
        ("revision", cmd::string(&package_revision(current)?)),
        (
            "command_operations",
            JsonValue::Array(operations.iter().map(|value| cmd::string(value)).collect()),
        ),
        (
            "supported_operations",
            if cmd::text(&owner.config, "schema_version")? == REVISION_OWNER {
                JsonValue::Array(vec![cmd::string("claim.revise")])
            } else {
                JsonValue::Array(vec![cmd::string("form.create"), cmd::string("form.revise")])
            },
        ),
        (
            "allowed_operations",
            cmd::field(&owner.config, "allowed_operations")?.clone(),
        ),
        (
            "allowed_form_ids",
            cmd::field(&owner.config, "allowed_form_ids")?.clone(),
        ),
        (
            "allowed_form_field_ids",
            cmd::field(&owner.config, "allowed_form_field_ids")?.clone(),
        ),
        ("receipt", receipt.unwrap_or(JsonValue::Null)),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
        (
            "materializations",
            JsonValue::Array(materializations(claim, set)?),
        ),
    ];
    if let Some(allowed_fields) = owner.config.object_get("allowed_fields") {
        fields.push(("allowed_fields", allowed_fields.clone()));
    }
    let _ = current;
    Ok(cmd::object(fields))
}

fn claim_scope(owner: &Owner, request: &JsonValue, record: &JsonValue) -> SourceCommandResult<()> {
    if cmd::text(&owner.config, "schema_version")? != REVISION_OWNER
        || !cmd::array(&owner.config, "allowed_operations")?
            .iter()
            .any(|v| v.as_str() == Some("claim.revise"))
        || cmd::text(record, "schema_version")? != "tos_historical_claim_v1"
        || cmd::text(record, "subject_ref")? != cmd::text(&owner.config, "historical_record_id")?
    {
        return Err(SourceCommandError::Denied(
            "historical Claim revision scope",
        ));
    }
    let fields = cmd::field(request, "fields")?;
    cmd::exact_keys(fields, &["qualifiers"])?;
    let qualifiers = object_map(cmd::field(fields, "qualifiers")?)?;
    let allowed = texts(&owner.config, "allowed_qualifier_fields", 4)?;
    if qualifiers.is_empty()
        || qualifiers.iter().any(|(key, _)| {
            key.as_str()
                .is_none_or(|name| !allowed.iter().any(|value| value == name))
        })
    {
        return Err(SourceCommandError::Denied(
            "historical Claim qualifier exceeds delegated fields",
        ));
    }
    let reason = cmd::text(request, "reason")?;
    let trimmed = tos_foundation::python_strip_unicode16_v1(reason, reason.chars().count())
        .map_err(|_| invalid())?;
    if trimmed.is_empty() || trimmed.chars().count() > 4096 {
        return Err(SourceCommandError::Invalid(
            "historical Claim correction reason",
        ));
    }
    let selections = cmd::array(request, "forms")?;
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "historical Claim form selection capacity",
        ));
    }
    let allowed_ids = texts(&owner.config, "allowed_form_ids", 32)?;
    let allowed_fields = form_field_ids(&owner.config)?;
    let mut ids = BTreeSet::new();
    let mut statement = false;
    for selection in selections {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let id = cmd::text(selection, "form_id")?;
        let field = cmd::text(selection, "field_id")?;
        if !allowed_ids.iter().any(|value| value == id)
            || !ids.insert(id.to_owned())
            || !allowed_fields.iter().any(|value| value == field)
        {
            return Err(SourceCommandError::Denied(
                "historical Claim selected form outside delegation",
            ));
        }
        statement |= field == "claim.statement";
    }
    if !statement {
        return Err(SourceCommandError::Denied(
            "historical Claim revision must rebind statement",
        ));
    }
    Ok(())
}

fn historical_grounding(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    authored_members: &[SourceFile],
    config: &JsonValue,
    claim: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, JsonValue)> {
    let (
        objects,
        events,
        anchors,
        record_inputs,
        native_identity_snapshot,
        native_text_snapshot,
        form_inputs,
        identity_index_digest,
    ) = if let Some(index) = cut.identity_index() {
        let selected = source_claims::maintained_selected_identity_closure(
            ctx,
            authored_members,
            worker,
            deadline,
            cancelled,
        )?;
        for (token, resolution) in &index.entries {
            let Some((kind, path)) = resolution else {
                if selected.objects.contains_key(token)
                    || selected.events.object_get(token).is_some()
                    || selected.anchors.object_get(token).is_some()
                    || selected.claims.contains_key(token)
                {
                    return Err(SourceCommandError::Conflict(
                        "historical Claim PG identity absence differs from selected bytes",
                    ));
                }
                continue;
            };
            let indexed_path = path.as_str();
            let selected_path = match kind.as_str() {
                "metadata" => selected
                    .objects
                    .get(token)
                    .and_then(|entry| entry.object_get("source_record_ref"))
                    .and_then(JsonValue::as_str),
                "event" => selected
                    .events
                    .object_get(token)
                    .and_then(|entry| entry.object_get("source_ref"))
                    .and_then(JsonValue::as_str),
                "anchor" => selected
                    .anchors
                    .object_get(token)
                    .and_then(|entry| entry.object_get("source_ref"))
                    .and_then(JsonValue::as_str),
                "claim" | "form" => continue,
                _ => {
                    return Err(SourceCommandError::Conflict(
                        "historical Claim identity index kind is unsupported",
                    ));
                }
            };
            if selected_path != Some(indexed_path) {
                return Err(SourceCommandError::Conflict(
                    "historical Claim selected identity differs from PG index",
                ));
            }
        }
        let form_inputs = selected_form_identity_inputs(
            ctx,
            index,
            config,
            &rel(cmd::text(config, "source_path")?)?,
            cmd::text(config, "claim_id")?,
        )?;
        (
            selected.objects,
            selected.events,
            selected.anchors,
            selected.record_inputs,
            None,
            None,
            form_inputs,
            Some(index.selected_digest()?),
        )
    } else {
        let inventory = source_claims::maintained_inventory_from_members(
            ctx,
            authored_members,
            false,
            worker,
            deadline,
            cancelled,
        )?;
        let form_inputs = form_identity_inputs(
            ctx,
            cut,
            &inventory,
            config,
            &rel(cmd::text(config, "source_path")?)?,
            cmd::text(config, "claim_id")?,
            worker,
            deadline,
            cancelled,
        )?;
        (
            inventory.objects,
            inventory.events,
            inventory.anchors,
            inventory.record_inputs,
            inventory.native_identity_snapshot,
            inventory.native_text_snapshot,
            form_inputs,
            None,
        )
    };
    let entities = ctx
        .file(&rel(
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        )?)?
        .ok_or(SourceCommandError::Unsupported(
            "historical entity registry absent",
        ))?;
    let relations = ctx
        .file(&rel(
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        )?)?
        .ok_or(SourceCommandError::Unsupported(
            "historical relation registry absent",
        ))?;
    let entity_registry = cmd::parse(entities)?;
    let relation_registry = cmd::parse(relations)?;
    let types = cmd::array(&entity_registry, "types")?;
    let predicate = cmd::text(claim, "predicate")?;
    let mapped = cmd::array(&relation_registry, "relations")?
        .iter()
        .filter(|relation| {
            cmd::array(relation, "source_mappings").is_ok_and(|mappings| {
                mappings.iter().any(|mapping| {
                    cmd::text(mapping, "source_graph").ok() == Some("source-claims")
                        && cmd::text(mapping, "scope").ok() == Some("claim-predicate")
                        && cmd::text(mapping, "source_predicate_id").ok() == Some(predicate)
                })
            })
        })
        .collect::<Vec<_>>();
    if mapped.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "historical predicate registry mapping",
        ));
    }
    let objects = &objects;
    let subject_id = cmd::text(claim, "subject_ref")?;
    let object_value = cmd::field(claim, "object")?;
    let mut identities = BTreeSet::from([subject_id.to_owned()]);
    if let Some(id) = object_value.as_str() {
        identities.insert(id.to_owned());
    } else if let Some(anchor) = object_value
        .object_get("relative")
        .and_then(|v| v.object_get("anchor_ref"))
        .and_then(JsonValue::as_str)
    {
        identities.insert(anchor.to_owned());
    }
    for (field, scope) in [
        ("subject_ref", "domain_type_ids"),
        ("object", "range_type_ids"),
    ] {
        let type_id = if field == "object" && predicate == "historical_dating" {
            if let Some(anchor) = object_value
                .object_get("relative")
                .and_then(|v| v.object_get("anchor_ref"))
                .and_then(JsonValue::as_str)
            {
                if !objects.get(anchor).is_some_and(|entry| {
                    matches!(
                        cmd::text(entry, "record_type").ok(),
                        Some("historical-event" | "historical-process" | "historical-state")
                    )
                }) {
                    return Err(SourceCommandError::Invalid(
                        "historical dating anchor unresolved",
                    ));
                }
            }
            "tos.entity.temporal-assertion"
        } else {
            let id = if field == "subject_ref" {
                subject_id
            } else {
                cmd::text(claim, "object")?
            };
            let entry = objects.get(id).ok_or(SourceCommandError::Invalid(
                "historical endpoint unresolved",
            ))?;
            let kind = cmd::text(entry, "record_type")?;
            let mapped = types
                .iter()
                .filter(|item| {
                    cmd::array(item, "source_mappings").is_ok_and(|mappings| {
                        mappings.iter().any(|mapping| {
                            cmd::text(mapping, "source_graph").ok() == Some("source-claims")
                                && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                        })
                    })
                })
                .collect::<Vec<_>>();
            if mapped.len() != 1 {
                return Err(SourceCommandError::Invalid(
                    "historical endpoint type mapping",
                ));
            }
            cmd::text(mapped[0], "type_id")?
        };
        let ancestry = source_claims::ancestry(types, type_id)?;
        if !cmd::array(mapped[0], scope)?
            .iter()
            .any(|value| value.as_str().is_some_and(|id| ancestry.contains(id)))
        {
            return Err(SourceCommandError::Denied(
                "historical registry domain/range",
            ));
        }
    }
    if !events
        .object_get(cmd::text(claim, "provenance_event_ref")?)
        .is_some()
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim provenance event unresolved",
        ));
    }
    let mut evidence_bindings = Vec::new();
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = claim.object_get(key) {
            for reference in values.as_array().ok_or(invalid())? {
                let reference = reference.as_str().ok_or(invalid())?;
                if reference.starts_with("ToS/") {
                    let parsed = rel(reference)?;
                    if parsed.as_str() != reference
                        || Path::new(reference).components().any(|component| {
                            matches!(
                                component,
                                std::path::Component::ParentDir | std::path::Component::CurDir
                            )
                        })
                        || reference.split('/').any(|part| {
                            matches!(
                                part,
                                "payload" | "local-content" | "owner-local" | "private"
                            )
                        })
                    {
                        return Err(SourceCommandError::Denied(
                            "historical Claim evidence addresses private or noncanonical metadata",
                        ));
                    }
                }
                evidence_bindings.push(source_claims::maintained_evidence(
                    ctx, reference, objects, &anchors, &events,
                )?);
            }
        }
    }
    let mut object_bindings = Vec::new();
    for id in identities {
        let entry = objects.get(&id).ok_or(SourceCommandError::Invalid(
            "historical endpoint unresolved",
        ))?;
        let path = cmd::text(entry, "source_record_ref")?;
        let raw = ctx
            .file(&rel(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "historical endpoint source absent",
            ))?;
        let parsed = cmd::parse(raw)?;
        object_bindings.push((
            JsonString::from_utf8(&id),
            cmd::object(vec![
                ("source_ref", cmd::string(path)),
                (
                    "source_sha256",
                    cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                ),
                (
                    "canonical_record_sha256",
                    cmd::string(&format!("sha256:{}", cmd::text(entry, "record_sha256")?)),
                ),
                (
                    "schema_version",
                    cmd::field(&parsed, "schema_version")?.clone(),
                ),
                (
                    "record_version",
                    cmd::field(&parsed, "record_version")?.clone(),
                ),
            ]),
        ));
    }
    let mut bound_evidence = Vec::new();
    for node in evidence_bindings {
        let properties = cmd::field(&node, "properties")?;
        let reference = cmd::text(properties, "evidence_ref")?;
        let digest = cmd::text(&node, "source_sha256")?;
        let digest = if digest.starts_with("sha256:") {
            digest.to_owned()
        } else {
            format!("sha256:{digest}")
        };
        bound_evidence.push((
            JsonString::from_utf8(reference),
            cmd::object(vec![
                ("source_ref", cmd::field(&node, "source_ref")?.clone()),
                ("source_sha256", cmd::string(&digest)),
                (
                    "source_line",
                    node.object_get("source_line")
                        .cloned()
                        .unwrap_or(JsonValue::Null),
                ),
                (
                    "evidence_kind",
                    cmd::field(properties, "evidence_kind")?.clone(),
                ),
            ]),
        ));
    }
    let mut bindings = cmd::object(vec![
        ("objects", JsonValue::Object(object_bindings)),
        ("evidence", JsonValue::Object(bound_evidence)),
    ]);
    if let Some(index) = &identity_index_digest {
        cmd::set(&mut bindings, "identity_index", index.clone())?;
    }
    let contracts = contract_digests(ctx)?;
    let mut implementation = Vec::new();
    for name in HISTORICAL_GROUND_IMPLEMENTATIONS {
        let bytes = ctx
            .file(&rel(name)?)?
            .ok_or(SourceCommandError::Unsupported(
                "historical Claim implementation input absent",
            ))?;
        implementation.push((
            JsonString::from_utf8(name),
            cmd::string(&Digest256::of_bytes(bytes).to_prefixed()),
        ));
    }
    let source_contracts = JsonValue::Object(
        record_inputs
            .as_object()
            .ok_or(invalid())?
            .iter()
            .map(|(name, digest)| {
                let digest = digest.as_str().ok_or(invalid())?;
                Ok((name.clone(), cmd::string(&format!("sha256:{digest}"))))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let mut profile_inputs = vec![("source_contracts", source_contracts)];
    if let Some(snapshot) = &native_identity_snapshot {
        profile_inputs.push(("native_semantic_identity_snapshot", cmd::string(snapshot)));
    }
    if let Some(snapshot) = &native_text_snapshot {
        let mut native_implementation = Vec::new();
        for name in [
            "rust/crates/tos-command/src/source_sign_native.rs",
            "rust/crates/tos-command/src/source_text_owner.rs",
        ] {
            let raw = ctx
                .file(&rel(name)?)?
                .ok_or(SourceCommandError::Unsupported(
                    "historical native text implementation absent",
                ))?;
            native_implementation.push((
                JsonString::from_utf8(name),
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ));
        }
        profile_inputs.push((
            "native_binding_implementation",
            JsonValue::Object(native_implementation),
        ));
        profile_inputs.push(("native_text_binding_snapshot", cmd::string(snapshot)));
    }
    let mut ground = cmd::object(vec![
        ("contracts", contracts),
        (
            "objects",
            JsonValue::Object(
                objects
                    .iter()
                    .map(|(key, value)| (JsonString::from_utf8(key), value.clone()))
                    .collect(),
            ),
        ),
        ("profiles", cmd::object(profile_inputs)),
        ("events", events),
        ("anchors", anchors),
        ("bindings", bindings.clone()),
        ("form_identity_inputs", form_inputs),
        ("implementation", JsonValue::Object(implementation.clone())),
    ]);
    if let Some(index) = identity_index_digest {
        cmd::set(&mut ground, "identity_index", index)?;
    }
    let grounding = cmd::record_digest(&ground)?.to_prefixed();
    let mut revision_implementation = Vec::new();
    for name in REVISION_IMPLEMENTATIONS {
        let bytes = ctx
            .file(&rel(name)?)?
            .ok_or(SourceCommandError::Unsupported(
                "historical Claim revision implementation input absent",
            ))?;
        revision_implementation.push((
            JsonString::from_utf8(name),
            cmd::string(&Digest256::of_bytes(bytes).to_prefixed()),
        ));
    }
    let dependencies = cmd::record_digest(&cmd::object(vec![
        ("grounding", cmd::string(&grounding)),
        ("implementation", JsonValue::Object(revision_implementation)),
    ]))?
    .to_prefixed();
    let _ = config;
    Ok((dependencies, bindings))
}

fn revision_proposal(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    authored_members: &[SourceFile],
    owner: &Owner,
    current: &LegacyPackage,
    claim: &JsonValue,
    existing_forms: Option<&JsonValue>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    JsonValue,
    LegacyPackage,
    Vec<JsonValue>,
    Vec<JsonValue>,
    String,
    JsonValue,
)> {
    claim_scope(owner, &owner.request, claim)?;
    let revised = advance(claim, cmd::field(&owner.request, "fields")?)?;
    validate_config_claim(ctx, owner, &revised, worker, deadline, cancelled)?;
    let (dependencies, bindings) = historical_grounding(
        ctx,
        cut,
        authored_members,
        &owner.config,
        &revised,
        worker,
        deadline,
        cancelled,
    )?;
    let selectors = cmd::array(&owner.request, "forms")?;
    let requested = selectors
        .iter()
        .map(|item| cmd::text(item, "form_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if let Some(set) = existing_forms {
        for form in cmd::array(set, "forms")? {
            if !requested.contains(cmd::text(form, "form_id")?) {
                return Err(SourceCommandError::Denied(
                    "historical Claim correction must rebind every current form",
                ));
            }
        }
    }
    let fields = form_field_ids(&owner.config)?;
    let mut changes = Vec::new();
    for selection in selectors {
        changes.push(source_forms::prepare_form_change(
            &revised,
            existing_forms,
            cmd::text(&owner.config, "principal_id")?,
            cmd::text(selection, "form_id")?,
            cmd::text(selection, "field_id")?,
        )?);
    }
    validate_forms_scope(&owner.config, &fields, &changes, existing_forms)?;
    let next_subject = subject(&revised)?;
    let next_forms = source_forms::apply_form_changes(existing_forms, &next_subject, &changes)?;
    let views = source_forms::materialize_source_forms(&revised, &next_forms)?;
    if views
        .iter()
        .any(|view| optional_text(view, "state") != Some("ready"))
        || !views
            .iter()
            .any(|view| optional_text(view, "role") == Some("statement"))
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim forms must be ready source copies including statement",
        ));
    }
    let refs = changes
        .iter()
        .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let stream = current.get(STREAM).ok_or(invalid())?;
    let mut output = current.clone();
    output.insert(STREAM.into(), replace_row(stream, &revised)?);
    output.insert(
        package_form_name(cmd::text(&owner.config, "source_path")?, &owner.claim_id)?,
        cmd::published(&next_forms)?,
    );
    Ok((revised, output, views, refs, dependencies, bindings))
}

fn revision_result(
    owner: &Owner,
    current: &LegacyPackage,
    claim: &JsonValue,
    forms: Option<&JsonValue>,
    receipt: Option<JsonValue>,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let mut result = result_base(owner, current, claim, forms, receipt, replayed)?;
    cmd::set(
        &mut result,
        "allowed_fields",
        cmd::field(&owner.config, "allowed_fields")?.clone(),
    )?;
    Ok(result)
}

fn validate_claim_head(
    ctx: &CommandContext,
    owner: &Owner,
    current: &LegacyPackage,
    history: &History,
    original_claims: &[JsonValue],
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Option<JsonValue>)> {
    verify_record_origin(
        ctx,
        current,
        owner,
        history,
        original_claims,
        reader,
        worker,
        deadline,
        cancelled,
    )?;
    creation_lineage(current, owner, original_claims, history)?;
    let rows = claim_rows(current.get(STREAM).ok_or(invalid())?)?;
    let claim = rows
        .get(&owner.claim_id)
        .ok_or(SourceCommandError::Conflict(
            "delegated historical Claim is absent",
        ))?
        .clone();
    validate_config_claim(ctx, owner, &claim, worker, deadline, cancelled)?;
    // The captured request is exact source authority for identity continuity;
    // source schemas and the existing HumanForm kernel validate its descendants.
    let form_name = package_form_name(cmd::text(&owner.config, "source_path")?, &owner.claim_id)?;
    let set = current
        .get(&form_name)
        .map(|raw| cmd::parse(raw))
        .transpose()?;
    if let Some(set) = &set {
        schema(
            cmd::text(&owner.config, "source_path")?,
            ctx,
            worker,
            set,
            "ToS/contracts/human-form-set.schema.json",
            &["ToS/contracts/human-form-set.schema.json"],
            deadline,
            cancelled,
        )?;
        let _ = source_forms::materialize_source_forms(&claim, set)?;
    }
    let _ = (reader, worker, deadline, cancelled);
    Ok((claim, set))
}

fn prepare_revision(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    authored_members: &[SourceFile],
    owner: &Owner,
    current: &LegacyPackage,
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyClaimPlan> {
    validate_cut(ctx, cut, &owner.path, current, deadline, cancelled)?;
    let (request, _creation_receipt, original_claims) =
        validate_capture(current, owner, worker, ctx, deadline, cancelled)?;
    let history = validate_history(
        ctx,
        current,
        &owner.config,
        &original_claims,
        reader,
        worker,
        deadline,
        cancelled,
    )?;
    let (claim, forms) = validate_claim_head(
        ctx,
        owner,
        current,
        &history,
        &original_claims,
        reader,
        worker,
        deadline,
        cancelled,
    )?;
    let operation = cmd::text(&owner.request, "operation")?;
    if operation == "describe" {
        return Ok(LegacyClaimPlan {
            files: None,
            response: revision_result(owner, current, &claim, forms.as_ref(), None, false)?,
            archive: None,
            reads: read_dependencies(owner),
            replayed: false,
        });
    }
    if operation == "inspect-version" {
        let selected = cmd::field(&owner.request, "source")?;
        let mut found = None;
        for receipt in cmd::array(&history.payload, "receipts")? {
            let previous = cmd::field(receipt, "previous_source")?;
            if same(previous, selected)? && cmd::text(previous, "id")? == owner.claim_id {
                found = Some(receipt);
                break;
            }
        }
        let found = found.ok_or(SourceCommandError::Conflict(
            "exact historical Claim version is not retained",
        ))?;
        let archive_raw =
            reader.read_archive(cmd::text(found, "archive_path")?, deadline, cancelled)?;
        let (archived, locations) = decode_archive(archive_raw, &owner.config, found)?;
        let rows = claim_rows(archived.get(STREAM).ok_or(invalid())?)?;
        let inspected = rows.get(&owner.claim_id).ok_or(invalid())?;
        let mut response = revision_result(owner, current, &claim, forms.as_ref(), None, false)?;
        cmd::set(&mut response, "record", inspected.clone())?;
        cmd::set(&mut response, "inspected_source", selected.clone())?;
        cmd::set(&mut response, "files", locations)?;
        return Ok(LegacyClaimPlan {
            files: None,
            response,
            archive: None,
            reads: read_dependencies(owner),
            replayed: false,
        });
    }
    if operation == "prepare-revise" {
        if cmd::array(&history.payload, "receipts")?.len() >= 128 {
            return Err(SourceCommandError::Invalid(
                "Claim correction history capacity reached",
            ));
        }
        let (revised, _, views, refs, dependencies, bindings) = revision_proposal(
            ctx,
            cut,
            authored_members,
            owner,
            current,
            &claim,
            forms.as_ref(),
            worker,
            deadline,
            cancelled,
        )?;
        let mut response = revision_result(owner, current, &claim, forms.as_ref(), None, false)?;
        cmd::set(&mut response, "prepared_source", subject(&revised)?)?;
        cmd::set(&mut response, "prepared_forms", JsonValue::Array(refs))?;
        cmd::set(
            &mut response,
            "prepared_materializations",
            JsonValue::Array(views),
        )?;
        cmd::set(
            &mut response,
            "expected_dependencies",
            cmd::string(&dependencies),
        )?;
        cmd::set(&mut response, "source_bindings", bindings)?;
        return Ok(LegacyClaimPlan {
            files: None,
            response,
            archive: None,
            reads: read_dependencies(owner),
            replayed: false,
        });
    }
    claim_scope(owner, &owner.request, &claim)?;
    let command_id = cmd::text(&owner.request, "command_id")?;
    if !(1..=256).contains(&command_id.chars().count()) {
        return Err(SourceCommandError::Invalid(
            "historical Claim command identity",
        ));
    }
    let request_digest = cmd::record_digest(&owner.request)?.to_prefixed();
    for receipt in cmd::array(&history.payload, "receipts")? {
        if cmd::text(receipt, "command_id")? == command_id {
            if cmd::text(receipt, "request_digest")? != request_digest
                || cmd::text(cmd::field(receipt, "source")?, "id")? != owner.claim_id
            {
                return Err(SourceCommandError::Conflict(
                    "historical Claim command identity was reused",
                ));
            }
            let mut response = revision_result(
                owner,
                current,
                &claim,
                forms.as_ref(),
                Some(receipt.clone()),
                true,
            )?;
            cmd::set(
                &mut response,
                "materializations",
                JsonValue::Array(materializations(&claim, forms.as_ref())?),
            )?;
            return Ok(LegacyClaimPlan {
                files: None,
                response,
                archive: None,
                reads: read_dependencies(owner),
                replayed: true,
            });
        }
    }
    if cmd::text(&owner.request, "expected_configuration")? != owner.config_digest
        || !same(
            cmd::field(&owner.request, "expected_source")?,
            &subject(&claim)?,
        )?
        || cmd::text(&owner.request, "expected_revision")? != package_revision(current)?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim compare-and-swap snapshot is stale",
        ));
    }
    if cmd::array(&history.payload, "receipts")?.len() >= 128 {
        return Err(SourceCommandError::Invalid(
            "Claim correction history capacity reached",
        ));
    }
    let (revised, proposed, _, refs, dependencies, bindings) = revision_proposal(
        ctx,
        cut,
        authored_members,
        owner,
        current,
        &claim,
        forms.as_ref(),
        worker,
        deadline,
        cancelled,
    )?;
    if cmd::text(&owner.request, "expected_dependencies")? != dependencies
        || !same(cmd::field(&owner.request, "expected_inputs")?, &bindings)?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim grounding inputs are stale",
        ));
    }
    let revision = package_revision(current)?;
    let before = subject(&claim)?;
    let archive = archive_package(
        current,
        cmd::text(&owner.config, "source_path")?,
        &before,
        &revision,
    )?;
    let receipt = cmd::object(vec![
        (
            "command_id",
            cmd::field(&owner.request, "command_id")?.clone(),
        ),
        ("request_digest", cmd::string(&request_digest)),
        (
            "principal_id",
            cmd::field(&owner.config, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&owner.config, "authority_ref")?.clone(),
        ),
        ("owner_configuration", cmd::string(&owner.config_digest)),
        ("recorded_at", cmd::string(&ctx.recorded_at)),
        ("reason", cmd::field(&owner.request, "reason")?.clone()),
        ("previous_source", before),
        ("source", subject(&revised)?),
        ("previous_revision", cmd::string(&revision)),
        ("archive_path", cmd::string(&archive.0)),
        ("dependencies", cmd::string(&dependencies)),
        ("source_bindings", bindings),
        (
            "changed_fields",
            JsonValue::Array(vec![cmd::string("qualifiers")]),
        ),
        ("forms", JsonValue::Array(refs)),
        ("grants_admission", JsonValue::Bool(false)),
        ("request", owner.request.clone()),
    ]);
    let mut output = proposed;
    let mut receipts = cmd::array(&history.payload, "receipts")?.to_vec();
    receipts.push(receipt.clone());
    let next_history = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_claim_revision_history_v1"),
        ),
        (
            "source_path",
            cmd::field(&owner.config, "source_path")?.clone(),
        ),
        ("receipts", JsonValue::Array(receipts)),
    ]);
    output.insert(HISTORY.into(), cmd::published(&next_history)?);
    if output.len() > 64
        || output.values().any(|raw| raw.len() > 8_388_608)
        || output
            .values()
            .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
            .is_none_or(|sum| sum > 8_388_608)
        || output.get(STREAM).is_none_or(|raw| raw.len() > 1_048_576)
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim successor package budget",
        ));
    }
    let response = revision_result(
        owner,
        &output,
        &revised,
        Some(&cmd::parse(
            output
                .get(&package_form_name(
                    cmd::text(&owner.config, "source_path")?,
                    &owner.claim_id,
                )?)
                .ok_or(invalid())?,
        )?),
        Some(receipt),
        false,
    )?;
    let _ = request;
    Ok(LegacyClaimPlan {
        files: Some(output),
        response,
        archive: Some(archive),
        reads: read_dependencies(owner),
        replayed: false,
    })
}

fn prepare_forms(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    owner: &Owner,
    current: &LegacyPackage,
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyClaimPlan> {
    validate_cut(ctx, cut, &owner.path, current, deadline, cancelled)?;
    let (_origin_request, _creation_receipt, original_claims) =
        validate_capture(current, owner, worker, ctx, deadline, cancelled)?;
    let history = validate_history(
        ctx,
        current,
        &owner.config,
        &original_claims,
        reader,
        worker,
        deadline,
        cancelled,
    )?;
    let (claim, set) = validate_claim_head(
        ctx,
        owner,
        current,
        &history,
        &original_claims,
        reader,
        worker,
        deadline,
        cancelled,
    )?;
    let fields = form_field_ids(&owner.config)?;
    let operation = cmd::text(&owner.request, "operation")?;
    if operation == "describe" {
        let response = form_result(owner, current, &claim, set.as_ref(), None, false, &fields)?;
        return Ok(LegacyClaimPlan {
            files: None,
            response,
            archive: None,
            reads: read_dependencies(owner),
            replayed: false,
        });
    }
    if operation == "prepare" {
        let id = cmd::text(&owner.request, "form_id")?;
        let field = cmd::text(&owner.request, "field_id")?;
        if !texts(&owner.config, "allowed_form_ids", 32)?
            .iter()
            .any(|item| item == id)
            || !fields.iter().any(|item| item == field)
        {
            return Err(SourceCommandError::Denied(
                "historical Claim form selector outside delegation",
            ));
        }
        let change = source_forms::prepare_form_change(
            &claim,
            set.as_ref(),
            cmd::text(&owner.config, "principal_id")?,
            id,
            field,
        )?;
        validate_forms_scope(
            &owner.config,
            &fields,
            std::slice::from_ref(&change),
            set.as_ref(),
        )?;
        if !cmd::array(&owner.config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == optional_text(&change, "operation"))
        {
            return Err(SourceCommandError::Denied(
                "historical Claim form operation not delegated",
            ));
        }
        let preview = source_forms::apply_form_changes(
            set.as_ref(),
            &subject(&claim)?,
            std::slice::from_ref(&change),
        )?;
        let view = source_forms::materialize_source_forms(&claim, &preview)?
            .into_iter()
            .find(|view| {
                view.object_get("form")
                    .and_then(|form| form.object_get("id"))
                    .and_then(JsonValue::as_str)
                    == Some(id)
            })
            .unwrap_or(JsonValue::Null);
        if view.is_null() || optional_text(&view, "state") != Some("ready") {
            return Err(SourceCommandError::Invalid(
                "historical Claim prepared form materialization absent",
            ));
        }
        let mut response = form_result(owner, current, &claim, set.as_ref(), None, false, &fields)?;
        cmd::set(&mut response, "prepared_change", change)?;
        cmd::set(&mut response, "prepared_materialization", view)?;
        return Ok(LegacyClaimPlan {
            files: None,
            response,
            archive: None,
            reads: read_dependencies(owner),
            replayed: false,
        });
    }
    let changes = validate_form_changes(&owner.config, &owner.request, &fields)?;
    let command_id = cmd::text(&owner.request, "command_id")?;
    if !(1..=256).contains(&command_id.chars().count()) {
        return Err(SourceCommandError::Invalid(
            "historical Claim form command identity",
        ));
    }
    let request_digest = cmd::record_digest(&owner.request)?.to_prefixed();
    if let Some(set) = &set {
        if let Some(receipts) = set
            .object_get("growth_history")
            .and_then(JsonValue::as_array)
        {
            for receipt in receipts {
                if cmd::text(receipt, "command_id")? == command_id {
                    if cmd::text(receipt, "request_digest")? != request_digest {
                        return Err(SourceCommandError::Conflict(
                            "historical Claim form command identity was reused",
                        ));
                    }
                    let expected_results = changes
                        .iter()
                        .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
                        .collect::<SourceCommandResult<Vec<_>>>()?;
                    let actual_results = cmd::array(receipt, "results")?;
                    let mut exact_results = actual_results.len() == expected_results.len();
                    for (actual, expected) in actual_results.iter().zip(&expected_results) {
                        exact_results &= same(actual, expected)?;
                    }
                    if !same(cmd::field(receipt, "source")?, &subject(&claim)?)? || !exact_results {
                        return Err(SourceCommandError::Conflict(
                            "historical Claim form replay receipt differs from exact results",
                        ));
                    }
                    // A retained retry is bound to its historical request and
                    // result. Later current form successors do not become an
                    // input to that replay, while current delegation still
                    // validates every selected request field above.
                    validate_forms_scope(&owner.config, &fields, &changes, None)?;
                    let response = form_result(
                        owner,
                        current,
                        &claim,
                        Some(set),
                        Some(receipt.clone()),
                        true,
                        &fields,
                    )?;
                    return Ok(LegacyClaimPlan {
                        files: None,
                        response,
                        archive: None,
                        reads: read_dependencies(owner),
                        replayed: true,
                    });
                }
            }
        }
    }
    validate_forms_scope(&owner.config, &fields, &changes, set.as_ref())?;
    if !same(
        cmd::field(&owner.request, "expected_source")?,
        &subject(&claim)?,
    )? || cmd::text(&owner.request, "expected_configuration")? != owner.config_digest
        || !same(
            cmd::field(&owner.request, "expected_revision")?,
            &current
                .get(&package_form_name(
                    cmd::text(&owner.config, "source_path")?,
                    &owner.claim_id,
                )?)
                .map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                .unwrap_or(JsonValue::Null),
        )?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim form compare-and-swap snapshot is stale",
        ));
    }
    let next = source_forms::apply_form_changes(set.as_ref(), &subject(&claim)?, &changes)?;
    let views = source_forms::materialize_source_forms(&claim, &next)?;
    if changes.iter().any(|change| {
        let form = change.object_get("form").unwrap_or(&JsonValue::Null);
        optional_text(
            form.object_get("content").unwrap_or(&JsonValue::Null),
            "kind",
        ) != Some("source-copy")
    }) || views
        .iter()
        .any(|view| optional_text(view, "state") != Some("ready"))
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim source-copy materialization is not ready",
        ));
    }
    let receipt = cmd::object(vec![
        (
            "command_id",
            cmd::field(&owner.request, "command_id")?.clone(),
        ),
        ("request_digest", cmd::string(&request_digest)),
        (
            "principal_id",
            cmd::field(&owner.config, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&owner.config, "authority_ref")?.clone(),
        ),
        ("owner_configuration", cmd::string(&owner.config_digest)),
        ("recorded_at", cmd::string(&ctx.recorded_at)),
        ("source_contracts", owner.contract_digests.clone()),
        ("source", subject(&claim)?),
        (
            "previous_revision",
            cmd::field(&owner.request, "expected_revision")?.clone(),
        ),
        (
            "results",
            JsonValue::Array(
                changes
                    .iter()
                    .map(|change| source_forms::form_reference(cmd::field(change, "form")?))
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        ),
    ]);
    let mut next = next;
    let mut receipts = next
        .object_get("growth_history")
        .and_then(JsonValue::as_array)
        .unwrap_or(&[])
        .to_vec();
    receipts.push(receipt.clone());
    cmd::set(&mut next, "growth_history", JsonValue::Array(receipts))?;
    let views = source_forms::materialize_source_forms(&claim, &next)?;
    if views
        .iter()
        .any(|view| optional_text(view, "state") != Some("ready"))
    {
        return Err(SourceCommandError::Invalid(
            "historical Claim retained form receipt is not source-ready",
        ));
    }
    let encoded = cmd::published(&next)?;
    if encoded.len() > 2_097_152 {
        return Err(SourceCommandError::Invalid(
            "historical Claim form set byte budget",
        ));
    }
    schema(
        cmd::text(&owner.config, "source_path")?,
        ctx,
        worker,
        &next,
        "ToS/contracts/human-form-set.schema.json",
        &["ToS/contracts/human-form-set.schema.json"],
        deadline,
        cancelled,
    )?;
    let name = package_form_name(cmd::text(&owner.config, "source_path")?, &owner.claim_id)?;
    let mut after = current.clone();
    after.insert(name, encoded);
    let response = form_result(
        owner,
        &after,
        &claim,
        Some(&next),
        Some(receipt),
        false,
        &fields,
    )?;
    Ok(LegacyClaimPlan {
        files: Some(after),
        response,
        archive: None,
        reads: read_dependencies(owner),
        replayed: false,
    })
}

fn validate_form_changes(
    config: &JsonValue,
    request: &JsonValue,
    fields: &[String],
) -> SourceCommandResult<Vec<JsonValue>> {
    let changes = cmd::array(request, "changes")?;
    if changes.is_empty() || changes.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "historical Claim form batch capacity",
        ));
    }
    for change in changes {
        cmd::exact_keys(change, &["operation", "expected_form", "form"])?;
        let form = cmd::field(change, "form")?;
        let operation = cmd::text(change, "operation")?;
        if !cmd::array(config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(operation))
            || cmd::text(form, "creator_id")? != cmd::text(config, "principal_id")?
        {
            return Err(SourceCommandError::Denied(
                "historical Claim form operation or creator scope",
            ));
        }
    }
    validate_forms_scope(config, fields, changes, None)?;
    Ok(changes.to_vec())
}

fn form_result(
    owner: &Owner,
    current: &LegacyPackage,
    claim: &JsonValue,
    set: Option<&JsonValue>,
    receipt: Option<JsonValue>,
    replayed: bool,
    fields: &[String],
) -> SourceCommandResult<JsonValue> {
    let source_path = cmd::text(&owner.config, "source_path")?;
    let target = package_form_name(source_path, &owner.claim_id)?;
    let target_path = format!(
        "{}/{}",
        source_path.rsplit_once('/').ok_or(invalid())?.0,
        target
    );
    let source_fields = source_forms::metadata_fields(claim)?
        .iter()
        .map(|field| field.public())
        .collect::<Vec<_>>();
    let forms = form_refs(set)?;
    let materials = materializations(claim, set)?;
    let result = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_command_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&owner.config_digest)),
        ("source", subject(claim)?),
        ("source_path", cmd::string(source_path)),
        ("target_path", cmd::string(&target_path)),
        (
            "revision",
            current
                .get(&target)
                .map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                .unwrap_or(JsonValue::Null),
        ),
        (
            "supported_operations",
            JsonValue::Array(vec![cmd::string("form.create"), cmd::string("form.revise")]),
        ),
        (
            "allowed_operations",
            cmd::field(&owner.config, "allowed_operations")?.clone(),
        ),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare", "apply"]
                    .iter()
                    .map(|v| cmd::string(v))
                    .collect(),
            ),
        ),
        ("source_fields", JsonValue::Array(source_fields)),
        (
            "allowed_form_ids",
            cmd::field(&owner.config, "allowed_form_ids")?.clone(),
        ),
        (
            "allowed_field_ids",
            JsonValue::Array(fields.iter().map(|v| cmd::string(v)).collect()),
        ),
        ("source_contracts", owner.contract_digests.clone()),
        ("forms", JsonValue::Array(forms)),
        ("materializations", JsonValue::Array(materials)),
        ("receipt", receipt.unwrap_or(JsonValue::Null)),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    Ok(result)
}

fn validate_authored_members(
    ctx: &CommandContext,
    authored_members: &[SourceFile],
) -> SourceCommandResult<()> {
    let mut selected = ctx
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    let mut supplied = authored_members.to_vec();
    supplied.sort_by(|left, right| left.path.cmp(&right.path));
    if selected != supplied
        || supplied.windows(2).any(|pair| pair[0].path == pair[1].path)
        || supplied
            .iter()
            .any(|file| !file.path.as_str().starts_with("ToS/"))
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim context differs from selected authored members",
        ));
    }
    Ok(())
}

fn prepare_with_source(
    ctx: &CommandContext,
    cut: &impl HistoricalCurrentSource,
    authored_members: &[SourceFile],
    current: &LegacyPackage,
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyClaimPlan> {
    validate_authored_members(ctx, authored_members)?;
    let owner = owner(ctx, cut, authored_members, worker, deadline, cancelled)?;
    // Validate both independently retained ledgers' exact locators and the
    // combined bounded archive set before streaming any historical packages.
    let _archive_refs = required_archives(current, &owner.config)?;
    if cmd::text(&owner.config, "schema_version")? == REVISION_OWNER {
        prepare_revision(
            ctx,
            cut,
            authored_members,
            &owner,
            current,
            reader,
            worker,
            deadline,
            cancelled,
        )
    } else {
        prepare_forms(
            ctx, cut, &owner, current, reader, worker, deadline, cancelled,
        )
    }
}

pub(crate) fn prepare(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    current: &LegacyPackage,
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyClaimPlan> {
    let authored_members = ctx
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    prepare_with_source(
        ctx,
        &CorpusHistoricalCurrentSource(cut),
        &authored_members,
        current,
        reader,
        worker,
        deadline,
        cancelled,
    )
}

pub(crate) fn prepare_from_indexed_members(
    ctx: &CommandContext,
    source_revision: SourceRevision,
    authored_members: &[SourceFile],
    identity_index: &HistoricalIdentityIndex,
    current: &LegacyPackage,
    reader: &mut impl LegacyArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyClaimPlan> {
    if source_revision != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "historical Claim PG source revision differs",
        ));
    }
    let request = cmd::parse(&ctx.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let required = required_identity_tokens(&ctx.configuration_raw, current, operation)?
        .into_iter()
        .filter(|token| !token.starts_with("ToS/"))
        .collect::<BTreeSet<_>>();
    if identity_index
        .entries
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        != required
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim PG identity lookup set differs",
        ));
    }
    prepare_with_source(
        ctx,
        &MembersHistoricalCurrentSource {
            revision: source_revision,
            members: authored_members,
            identity_index: Some(identity_index),
        },
        authored_members,
        current,
        reader,
        worker,
        deadline,
        cancelled,
    )
}

/// Rebuild the historical Claim owner projection for a recovered attempt.
/// The caller binds the attempt raw-request digest to its exact retry input;
/// this function separately checks the canonical digest retained in the
/// owner-generated history receipt without reconstructing the original input.
pub(crate) fn pg_revision_owner_contribution_for_attempt(
    ctx: &CommandContext,
    proposal: &PreparedCommand,
    command_id: &str,
    request_canonical_digest: Digest256,
    configuration_raw_digest: Digest256,
    configuration_canonical_digest: Digest256,
    family_tag: &str,
) -> SourceCommandResult<JsonValue> {
    if proposal.replayed || proposal.operation != "claim.revise" || proposal.changes.is_empty() {
        return Err(SourceCommandError::Conflict(
            "historical Claim owner projection requires a fresh revision",
        ));
    }
    let config = cmd::parse(&ctx.configuration_raw)?;
    let expected_family_tag =
        "tos-pg-historical-claim-v1:tos_local_historical_claim_revision_owner_v1:claim.revise";
    if cmd::text(&config, "schema_version")? != REVISION_OWNER
        || proposal.handler_id != "legacy-historical-claim-revision"
        || !(1..=256).contains(&command_id.chars().count())
        || family_tag != expected_family_tag
        || proposal.base_revision != ctx.base_revision
        || proposal.request_canonical_sha256 != request_canonical_digest
        || Digest256::of_bytes(&ctx.configuration_raw) != configuration_raw_digest
        || proposal.configuration_raw_sha256 != configuration_raw_digest
        || cmd::record_digest(&config)? != configuration_canonical_digest
        || proposal.configuration_canonical_sha256 != configuration_canonical_digest
    {
        return Err(SourceCommandError::Denied(
            "historical Claim owner projection configuration",
        ));
    }
    let source_path = cmd::text(&config, "source_path")?;
    let home = source_path
        .strip_suffix(&format!("/{STREAM}"))
        .ok_or(invalid())?;
    let claim_id = cmd::text(&config, "claim_id")?;
    let current = pg_package_from_context(ctx, home)?;
    let mut successor = current.clone();
    for change in &proposal.changes {
        let path = change.path.as_str();
        let Some(name) = path.strip_prefix(&format!("{home}/")) else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            return Err(SourceCommandError::Conflict(
                "historical Claim successor is not a flat owner package",
            ));
        }
        let before = successor.get(name);
        match (change.before, before) {
            (None, None) => {}
            (Some(expected), Some(raw)) if Digest256::of_bytes(raw) == expected => {}
            _ => {
                return Err(SourceCommandError::Conflict(
                    "historical Claim successor predecessor differs",
                ));
            }
        }
        match &change.after {
            Some(raw) => {
                successor.insert(name.to_owned(), raw.clone());
            }
            None => {
                successor.remove(name);
            }
        }
    }
    let current_stream = current.get(STREAM).ok_or(invalid())?;
    let successor_stream = successor.get(STREAM).ok_or(invalid())?;
    let current_claim =
        claim_rows(current_stream)?
            .remove(claim_id)
            .ok_or(SourceCommandError::Conflict(
                "historical Claim predecessor identity is absent",
            ))?;
    let successor_claim =
        claim_rows(successor_stream)?
            .remove(claim_id)
            .ok_or(SourceCommandError::Conflict(
                "historical Claim successor identity is absent",
            ))?;
    let current_version = cmd::integer(&current_claim, "claim_version")?;
    let successor_version = cmd::integer(&successor_claim, "claim_version")?;
    if current_version.checked_add(1) != Some(successor_version)
        || replace_row(current_stream, &successor_claim)?.as_slice() != successor_stream.as_slice()
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim successor row or version differs",
        ));
    }

    let form_name = package_form_name(source_path, claim_id)?;
    let form_raw = successor.get(&form_name);
    if let Some(raw) = form_raw {
        let _ =
            source_forms::pg_form_projection_contribution(&successor_claim, successor_stream, raw)?;
    }

    let history_raw = successor.get(HISTORY).ok_or(invalid())?;
    let history = cmd::parse(history_raw)?;
    let history_receipts = cmd::array(&history, "receipts")?;
    let previous_history_receipts = match current.get(HISTORY) {
        Some(raw) => {
            let current_history = cmd::parse(raw)?;
            cmd::array(&current_history, "receipts")?.to_vec()
        }
        None => Vec::new(),
    };
    if history_receipts.is_empty()
        || history_receipts.len() > 128
        || history_receipts.len() != previous_history_receipts.len() + 1
        || previous_history_receipts
            .iter()
            .zip(history_receipts)
            .any(|(before, after)| before != after)
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim successor history is not one exact append",
        ));
    }
    let last_receipt = history_receipts.last().ok_or(invalid())?;
    let predecessor_revision = source_revisions::revision(&current)?;
    if cmd::text(last_receipt, "command_id")? != command_id
        || cmd::text(last_receipt, "request_digest")? != request_canonical_digest.to_prefixed()
        || !same(
            cmd::field(last_receipt, "previous_source")?,
            &subject(&current_claim)?,
        )?
        || !same(
            cmd::field(last_receipt, "source")?,
            &subject(&successor_claim)?,
        )?
        || cmd::text(cmd::field(last_receipt, "source")?, "id")? != claim_id
        || cmd::text(last_receipt, "previous_revision")? != predecessor_revision
        || cmd::text(last_receipt, "archive_path")?
            != archive_reference(claim_id, &predecessor_revision)?
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim successor history tail differs",
        ));
    }

    let mut archives = Vec::new();
    for (index, receipt) in history_receipts.iter().enumerate() {
        let archive_path = cmd::text(receipt, "archive_path")?;
        let archived_bytes = pg_archive_from_context(ctx, proposal, archive_path)?;
        let (archived, _) = decode_archive(archived_bytes.clone(), &config, receipt)?;
        if index + 1 == history_receipts.len() && archived != current {
            return Err(SourceCommandError::Conflict(
                "historical Claim latest archive does not match predecessor package",
            ));
        }
        let members = archived
            .iter()
            .map(|(name, raw)| {
                cmd::object(vec![
                    ("name", cmd::string(name)),
                    (
                        "sha256",
                        cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                    ),
                    ("bytes", cmd::number(raw.len() as u64)),
                ])
            })
            .collect::<Vec<_>>();
        archives.push(cmd::object(vec![
            ("path", cmd::string(archive_path)),
            (
                "manifest_sha256",
                cmd::string(
                    &Digest256::of_bytes(archived_bytes.get("manifest.json").ok_or(invalid())?)
                        .to_prefixed(),
                ),
            ),
            (
                "revision",
                cmd::field(receipt, "previous_revision")?.clone(),
            ),
            ("members", JsonValue::Array(members)),
        ]));
    }

    let direct_members = successor
        .iter()
        .map(|(name, raw)| {
            let role = historical_package_role(name)?;
            Ok(cmd::object(vec![
                ("ref", cmd::string(&format!("{home}/{name}"))),
                ("role", cmd::string(role)),
                (
                    "sha256",
                    cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                ),
                ("bytes", cmd::number(raw.len() as u64)),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let claims = claim_rows(successor_stream)?
        .into_iter()
        .map(|(id, claim)| {
            let form_name = package_form_name(source_path, &id)?;
            let form = successor.get(&form_name);
            let ids = match form {
                Some(raw) => {
                    let set = cmd::parse(raw)?;
                    source_forms::projection_form_ids(&set)?
                }
                None => Vec::new(),
            };
            Ok(cmd::object(vec![
                ("claim_id", cmd::string(&id)),
                (
                    "claim_version",
                    cmd::number(cmd::integer(&claim, "claim_version")?),
                ),
                (
                    "claim_sha256",
                    cmd::string(&cmd::record_digest(&claim)?.to_prefixed()),
                ),
                ("form_ref", cmd::string(&format!("{home}/{form_name}"))),
                ("form_present", JsonValue::Bool(form.is_some())),
                (
                    "form_sha256",
                    form.map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                        .unwrap_or(JsonValue::Null),
                ),
                ("form_ids", JsonValue::Array(ids)),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let history_rows = history_receipts
        .iter()
        .map(|receipt| {
            Ok(cmd::object(vec![
                ("command_id", cmd::field(receipt, "command_id")?.clone()),
                (
                    "request_digest",
                    cmd::field(receipt, "request_digest")?.clone(),
                ),
                (
                    "previous_source",
                    cmd::field(receipt, "previous_source")?.clone(),
                ),
                ("source", cmd::field(receipt, "source")?.clone()),
                (
                    "previous_revision",
                    cmd::field(receipt, "previous_revision")?.clone(),
                ),
                ("archive_path", cmd::field(receipt, "archive_path")?.clone()),
                (
                    "receipt_sha256",
                    cmd::string(&cmd::record_digest(receipt)?.to_prefixed()),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let creation_receipt = cmd::parse(
        successor
            .get("source-create-receipt.json")
            .ok_or(invalid())?,
    )?;
    let owner_core = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_managed_historical_claim_owner_inventory_v1"),
        ),
        ("package_home", cmd::string(home)),
        ("source_path", cmd::string(source_path)),
        ("source_profiles", pg_source_profiles(ctx)?),
        (
            "stream_sha256",
            cmd::string(&Digest256::of_bytes(successor_stream).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        (
            "history",
            cmd::object(vec![
                ("ref", cmd::string(&format!("{home}/{HISTORY}"))),
                ("present", JsonValue::Bool(true)),
                (
                    "sha256",
                    cmd::string(&Digest256::of_bytes(history_raw).to_prefixed()),
                ),
                ("receipts", JsonValue::Array(history_rows)),
            ]),
        ),
        ("archives", JsonValue::Array(archives)),
        ("claims", JsonValue::Array(claims)),
        (
            "creation_receipt_sha256",
            cmd::string(
                &Digest256::of_bytes(
                    successor
                        .get("source-create-receipt.json")
                        .ok_or(invalid())?,
                )
                .to_prefixed(),
            ),
        ),
        (
            "creation_receipt_command_id",
            cmd::field(&creation_receipt, "command_id")?.clone(),
        ),
    ]);
    let closure = cmd::record_digest(&owner_core)?.to_prefixed();
    let mut owner = owner_core;
    cmd::set(&mut owner, "closure_sha256", cmd::string(&closure))?;
    Ok(owner)
}

/// Rebuild the current read-only Historical Claim owner from its exact
/// historical package, retained Claim archives and already-validated profile.
/// It does not create or infer an owner grant and does not require a revision
/// proposal.
pub(crate) fn pg_current_owner_inventory_contribution(
    ctx: &CommandContext,
    file: &SourceFile,
    source_profiles: JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let source_path = file.path.as_str();
    if !historical_path(source_path) {
        return Err(SourceCommandError::Unsupported(
            "historical Claim source path",
        ));
    }
    let home = source_path
        .strip_suffix(&format!("/{STREAM}"))
        .ok_or(invalid())?;
    let current = pg_selected_historical_package(ctx, home, source_path)?;
    let stream = current.get(STREAM).ok_or(invalid())?;
    let current_claims = claim_rows(stream)?;
    if current_claims.is_empty() || current_claims.len() > 128 {
        return Err(SourceCommandError::Conflict(
            "historical Claim current batch differs",
        ));
    }

    let request_raw = current.get("source-create-request.json").ok_or(invalid())?;
    let request = cmd::parse(request_raw)?;
    let receipt_raw = current.get("source-create-receipt.json").ok_or(invalid())?;
    let receipt = cmd::parse(receipt_raw)?;
    cmd::exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "record",
            "claims",
            "forms",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_source",
            "expected_revision",
        ],
    )?;
    cmd::exact_keys(
        &receipt,
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
    let record = cmd::field(&request, "record")?;
    let record_type = cmd::text(record, "record_type")?;
    let record_id = cmd::text(record, "record_id")?;
    let record_path = cmd::text(&receipt, "source_path")?;
    let record_form = format!("{record_type}.human-forms.json");
    let expected_original_files = BTreeSet::from([
        STREAM.to_owned(),
        format!("{record_type}.json"),
        record_form,
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
    ]);
    let receipt_files = cmd::field(&receipt, "files")?
        .as_object()
        .ok_or(invalid())?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(&request, "operation")? != "historical.create"
        || cmd::field(&request, "expected_source")? != &JsonValue::Null
        || cmd::field(&request, "expected_revision")? != &JsonValue::Null
        || !matches!(
            record_type,
            "historical-event" | "historical-process" | "historical-state"
        )
        || !historical_record_id(record_id)
        || cmd::text(&receipt, "schema_version")? != "tos_local_historical_create_receipt_v1"
        || cmd::text(&receipt, "command_id")? != cmd::text(&request, "command_id")?
        || cmd::text(&receipt, "request_digest")? != cmd::record_digest(&request)?.to_prefixed()
        || cmd::field(&receipt, "owner_configuration")?
            != cmd::field(&request, "expected_configuration")?
        || cmd::field(&receipt, "dependencies")? != cmd::field(&request, "expected_dependencies")?
        || !cmd::same(
            cmd::field(&receipt, "source")?,
            &source_forms::metadata_subject(record)?,
        )?
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || record_path.rsplit_once('/').map(|pair| pair.0) != Some(home)
        || record_path.rsplit('/').next() != Some(format!("{record_type}.json").as_str())
        || receipt_files.len() != expected_original_files.len()
        || receipt_files
            .iter()
            .filter_map(|(name, _)| name.as_str())
            .collect::<BTreeSet<_>>()
            != expected_original_files
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim retained creation receipt differs",
        ));
    }
    validate_captured_request_bytes(request_raw, &request)?;
    let request_claims = cmd::array(&request, "claims")?;
    let request_ids = request_claims
        .iter()
        .map(|claim| cmd::text(claim, "claim_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if request_ids != current_claims.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(SourceCommandError::Conflict(
            "historical Claim current identities differ from creation",
        ));
    }

    let history_raw = current.get(HISTORY);
    let history = if let Some(raw) = history_raw {
        cmd::parse(raw)?
    } else {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_revision_history_v1"),
            ),
            ("source_path", cmd::string(source_path)),
            ("receipts", JsonValue::Array(Vec::new())),
        ])
    };
    cmd::exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    let history_receipts = cmd::array(&history, "receipts")?;
    if cmd::text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || cmd::text(&history, "source_path")? != source_path
        || history_receipts.len() > 128
    {
        return Err(SourceCommandError::Conflict(
            "historical Claim history identity differs",
        ));
    }
    let history_config = cmd::object(vec![("source_path", cmd::string(source_path))]);
    let mut archive_reader = PgSelectedHistoricalArchiveReader { ctx };
    let validated_history = validate_history(
        ctx,
        &current,
        &history_config,
        request_claims,
        &mut archive_reader,
        worker,
        deadline,
        cancelled,
    )?;
    let expected_initial_stream =
        request_claims
            .iter()
            .try_fold(Vec::new(), |mut bytes, claim| {
                bytes.extend(cmd::canonical(claim)?);
                bytes.push(b'\n');
                Ok::<_, SourceCommandError>(bytes)
            })?;
    if validated_history.initial_stream != expected_initial_stream {
        return Err(SourceCommandError::Conflict(
            "historical Claim revision chain does not reach its creation stream",
        ));
    }
    validate_historical_creation_outputs(
        ctx,
        &current,
        source_path,
        record_path,
        &request,
        &receipt,
        request_claims,
        &validated_history.initial_stream,
        worker,
        deadline,
        cancelled,
    )?;
    let archive_config = cmd::object(vec![("source_path", cmd::string(source_path))]);
    let mut archives = Vec::with_capacity(history_receipts.len());
    let mut history_rows = Vec::with_capacity(history_receipts.len());
    let mut seen_archives = BTreeSet::new();
    for retained in history_receipts {
        let archive_path = cmd::text(retained, "archive_path")?;
        let id = cmd::text(cmd::field(retained, "previous_source")?, "id")?;
        let revision = cmd::text(retained, "previous_revision")?;
        if archive_path != archive_reference(id, revision)?
            || !seen_archives.insert(archive_path.to_owned())
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive locator differs",
            ));
        }
        let archived = pg_selected_archive_from_context(ctx, archive_path)?;
        let (decoded, _) = decode_archive(archived.clone(), &archive_config, retained)?;
        let members = decoded
            .iter()
            .map(|(name, raw)| {
                cmd::object(vec![
                    ("name", cmd::string(name)),
                    (
                        "sha256",
                        cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                    ),
                    ("bytes", cmd::number(raw.len() as u64)),
                ])
            })
            .collect::<Vec<_>>();
        let manifest = archived.get("manifest.json").ok_or(invalid())?;
        archives.push(cmd::object(vec![
            ("path", cmd::string(archive_path)),
            (
                "manifest_sha256",
                cmd::string(&Digest256::of_bytes(manifest).to_prefixed()),
            ),
            (
                "revision",
                cmd::field(retained, "previous_revision")?.clone(),
            ),
            ("members", JsonValue::Array(members)),
        ]));
        history_rows.push(cmd::object(vec![
            ("command_id", cmd::field(retained, "command_id")?.clone()),
            (
                "request_digest",
                cmd::field(retained, "request_digest")?.clone(),
            ),
            (
                "previous_source",
                cmd::field(retained, "previous_source")?.clone(),
            ),
            ("source", cmd::field(retained, "source")?.clone()),
            (
                "previous_revision",
                cmd::field(retained, "previous_revision")?.clone(),
            ),
            (
                "archive_path",
                cmd::field(retained, "archive_path")?.clone(),
            ),
            (
                "receipt_sha256",
                cmd::string(&cmd::record_digest(retained)?.to_prefixed()),
            ),
        ]));
    }

    let mut claims = Vec::with_capacity(current_claims.len());
    for (id, claim) in &current_claims {
        schema(
            source_path,
            ctx,
            worker,
            claim,
            CLAIM_SCHEMA,
            CLAIM_SCHEMA_CONTRACTS,
            deadline,
            cancelled,
        )?;
        if cmd::text(claim, "schema_version")? != "tos_historical_claim_v1"
            || cmd::text(claim, "subject_ref")? != record_id
            || ![
                "historical_participant",
                "historical_place",
                "historical_work",
                "historical_dating",
            ]
            .contains(&cmd::text(claim, "predicate")?)
            || !["public", "public_metadata_only"].contains(&cmd::text(claim, "visibility")?)
            || cmd::integer(claim, "claim_version")? == 0
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim source schema or family differs",
            ));
        }
        let form_name = package_form_name(source_path, id)?;
        let form = current.get(&form_name);
        let form_ids = if let Some(raw) = form {
            let set = cmd::parse(raw)?;
            schema(
                source_path,
                ctx,
                worker,
                &set,
                "ToS/contracts/human-form-set.schema.json",
                &["ToS/contracts/human-form-set.schema.json"],
                deadline,
                cancelled,
            )?;
            let subject = source_forms::metadata_subject(claim)?;
            if !cmd::same(cmd::field(&set, "subject")?, &subject)?
                || tos_validation::source_forms::inspect_lineage_raw(raw)
                    .map_err(|_| SourceCommandError::Invalid("historical Claim form lineage"))?
                    .subject_id
                    != *id
            {
                return Err(SourceCommandError::Conflict(
                    "historical Claim form source binding differs",
                ));
            }
            let _ = source_forms::materialize_source_forms(claim, &set)?;
            source_forms::stable_form_ids_after_history_validation(&set)?
        } else {
            Vec::new()
        };
        claims.push(cmd::object(vec![
            ("claim_id", cmd::string(id)),
            (
                "claim_version",
                cmd::number(cmd::integer(claim, "claim_version")?),
            ),
            (
                "claim_sha256",
                cmd::string(&cmd::record_digest(claim)?.to_prefixed()),
            ),
            ("form_ref", cmd::string(&format!("{home}/{form_name}"))),
            ("form_present", JsonValue::Bool(form.is_some())),
            (
                "form_sha256",
                form.map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                    .unwrap_or(JsonValue::Null),
            ),
            ("form_ids", JsonValue::Array(form_ids)),
        ]));
    }
    let direct_members = current
        .iter()
        .map(|(name, raw)| {
            Ok(cmd::object(vec![
                ("ref", cmd::string(&format!("{home}/{name}"))),
                ("role", cmd::string(historical_package_role(name)?)),
                (
                    "sha256",
                    cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                ),
                ("bytes", cmd::number(raw.len() as u64)),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let owner_core = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_managed_historical_claim_owner_inventory_v1"),
        ),
        ("package_home", cmd::string(home)),
        ("source_path", cmd::string(source_path)),
        ("source_profiles", source_profiles),
        (
            "stream_sha256",
            cmd::string(&Digest256::of_bytes(stream).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        (
            "history",
            cmd::object(vec![
                ("ref", cmd::string(&format!("{home}/{HISTORY}"))),
                ("present", JsonValue::Bool(history_raw.is_some())),
                (
                    "sha256",
                    history_raw
                        .map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                        .unwrap_or(JsonValue::Null),
                ),
                ("receipts", JsonValue::Array(history_rows)),
            ]),
        ),
        ("archives", JsonValue::Array(archives)),
        ("claims", JsonValue::Array(claims)),
        (
            "creation_receipt_sha256",
            cmd::string(&Digest256::of_bytes(receipt_raw).to_prefixed()),
        ),
        (
            "creation_receipt_command_id",
            cmd::field(&receipt, "command_id")?.clone(),
        ),
    ]);
    let closure_sha256 = cmd::record_digest(&owner_core)?.to_prefixed();
    let mut owner = owner_core;
    cmd::set(&mut owner, "closure_sha256", cmd::string(&closure_sha256))?;
    Ok(owner)
}

struct PgSelectedHistoricalArchiveReader<'a> {
    ctx: &'a CommandContext,
}

impl LegacyArchiveReader for PgSelectedHistoricalArchiveReader<'_> {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(SourceCommandError::Denied(
                "historical Claim archive read cancelled or expired",
            ));
        }
        pg_selected_archive_from_context(self.ctx, reference)
    }
}

fn pg_selected_historical_package(
    ctx: &CommandContext,
    home: &str,
    source_path: &str,
) -> SourceCommandResult<LegacyPackage> {
    let mut package = LegacyPackage::new();
    let stream = ctx.file(&rel(source_path)?)?.ok_or(invalid())?;
    let ids = claim_rows(stream)?.into_keys().collect::<Vec<_>>();
    let mut names = CAPTURE
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    names.insert(STREAM.to_owned());
    names.insert(HISTORY.to_owned());
    for id in ids {
        let name = package_form_name(source_path, &id)?;
        names.insert(name.clone());
        names.insert(format!(".{name}.writer.lock"));
    }
    for name in names {
        let path = format!("{home}/{name}");
        if let Some(raw) = ctx.file(&rel(&path)?)? {
            package.insert(name, raw.to_vec());
        } else if matches!(
            name.as_str(),
            STREAM
                | "source-create-request.json"
                | "source-create-environment.json"
                | "source-create-provenance.jsonl"
                | "source-create-receipt.json"
        ) {
            return Err(SourceCommandError::Conflict(
                "historical Claim current package member absent",
            ));
        }
    }
    Ok(package)
}

fn pg_selected_archive_from_context(
    ctx: &CommandContext,
    archive_path: &str,
) -> SourceCommandResult<LegacyPackage> {
    let manifest_path = rel(&format!("{archive_path}/manifest.json"))?;
    let manifest_raw = ctx.file(&manifest_path)?.ok_or(invalid())?;
    let manifest = cmd::parse(manifest_raw)?;
    let files = object_map(cmd::field(&manifest, "files")?)?;
    let mut package = LegacyPackage::new();
    package.insert("manifest.json".to_owned(), manifest_raw.to_vec());
    for (_, binding) in files {
        let blob = cmd::text(binding, "blob")?;
        let raw = ctx
            .file(&rel(&format!("{archive_path}/{blob}"))?)?
            .ok_or(invalid())?;
        if package.insert(blob.to_owned(), raw.to_vec()).is_some() {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive blob repeats",
            ));
        }
    }
    Ok(package)
}

fn pg_source_profiles(ctx: &CommandContext) -> SourceCommandResult<JsonValue> {
    let registry_refs = CLAIM_REGISTRY_REFS.iter().copied().collect::<BTreeSet<_>>();
    let mut profiles = cmd::object(Vec::new());
    for member in &ctx.files {
        let path = member.path.as_str();
        if (path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
            || registry_refs.contains(path)
        {
            cmd::set(
                &mut profiles,
                path,
                cmd::string(&Digest256::of_bytes(&member.raw).to_hex()),
            )?;
        }
    }
    Ok(profiles)
}

fn pg_package_from_context(ctx: &CommandContext, home: &str) -> SourceCommandResult<LegacyPackage> {
    let prefix = format!("{home}/");
    let mut package = LegacyPackage::new();
    for member in &ctx.files {
        let Some(name) = member.path.as_str().strip_prefix(&prefix) else {
            continue;
        };
        if name.is_empty()
            || name.contains('/')
            || package
                .insert(name.to_owned(), member.raw.clone())
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim selected package is not flat or unique",
            ));
        }
    }
    Ok(package)
}

fn pg_archive_from_context(
    ctx: &CommandContext,
    proposal: &PreparedCommand,
    archive_path: &str,
) -> SourceCommandResult<LegacyPackage> {
    let prefix = format!("{archive_path}/");
    let mut package = LegacyPackage::new();
    for member in &ctx.files {
        let Some(name) = member.path.as_str().strip_prefix(&prefix) else {
            continue;
        };
        if name.is_empty()
            || name.contains('/')
            || package
                .insert(name.to_owned(), member.raw.clone())
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive read is not flat or unique",
            ));
        }
    }
    for change in &proposal.changes {
        let Some(name) = change.path.as_str().strip_prefix(&prefix) else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            return Err(SourceCommandError::Conflict(
                "historical Claim archive successor is not flat",
            ));
        }
        if let Some(raw) = &change.after {
            package.insert(name.to_owned(), raw.clone());
        } else {
            package.remove(name);
        }
    }
    if package.is_empty() {
        return Err(SourceCommandError::Conflict(
            "historical Claim retained archive is absent",
        ));
    }
    Ok(package)
}

fn historical_package_role(name: &str) -> SourceCommandResult<&'static str> {
    match name {
        STREAM => Ok("claim-stream"),
        "source-create-request.json" => Ok("creation-request"),
        "source-create-environment.json" => Ok("creation-environment"),
        "source-create-provenance.jsonl" => Ok("creation-event"),
        "source-create-receipt.json" => Ok("creation-receipt"),
        HISTORY => Ok("revision-history"),
        name if name.ends_with(".human-forms.json") => Ok("human-form-set"),
        name if name.ends_with(".writer.lock") => Ok("operational-lock"),
        _ => Err(SourceCommandError::Conflict(
            "unrecognized historical Claim package member",
        )),
    }
}

#[cfg(test)]
mod pg_identity_selection_tests {
    use super::*;

    #[test]
    fn bootstrap_contract_historical_schema_uses_explicit_member_without_command_path() {
        const GRAMMAR: &str = "ToS/contracts/historical-path-fixture.schema.json";
        const SOURCE: &str = "ToS/source-witnesses/history/fixture/historical-claims.jsonl";
        let grammar =
            format!(r#"{{"$id":"https://tree-of-sophia.local/{GRAMMAR}","type":"object"}}"#)
                .into_bytes();
        let revision = SourceRevision(Digest256::of_bytes(b"historical schema fixture"));
        struct Worker {
            revision: SourceRevision,
            digest: Digest256,
            calls: Vec<String>,
            valid: bool,
        }
        impl tos_validation::source_cut::CutSchemaExecutor for Worker {
            fn check(
                &mut self,
                path: &str,
                raw: &[u8],
                contract: &str,
                _: Instant,
                _: &AtomicBool,
            ) -> Result<bool, tos_validation::item_rules::ItemRefusal> {
                assert_eq!(contract, GRAMMAR);
                assert_eq!(raw, b"{}");
                self.calls.push(path.to_owned());
                Ok(self.valid)
            }
        }
        impl source_revisions::RecordSchemaWorker for Worker {
            type Identity = SourceRevision;
            fn input_identity(&self) -> SourceRevision {
                self.revision
            }
            fn contract_digest(&self, contract: &str) -> Option<Digest256> {
                (contract == GRAMMAR).then_some(self.digest)
            }
        }
        let mut worker = Worker {
            revision,
            digest: Digest256::of_bytes(&grammar),
            calls: Vec::new(),
            valid: true,
        };
        let mut context = CommandContext {
            base_revision: revision,
            configuration_raw: b"{}".to_vec(),
            request_raw: b"{}".to_vec(),
            recorded_at: "2026-10-15T00:00:00Z".into(),
            effective_uid: 1000,
            files: vec![SourceFile {
                path: rel(GRAMMAR).unwrap(),
                raw: grammar,
            }],
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let cancelled = AtomicBool::new(false);
        let instance = cmd::object(vec![]);
        // The worker double tests this adapter's input binding only; it makes
        // no claim about validity under an actual historical schema.
        schema(
            SOURCE,
            &context,
            &mut worker,
            &instance,
            GRAMMAR,
            &[GRAMMAR],
            deadline,
            &cancelled,
        )
        .unwrap();
        assert_eq!(worker.calls, [SOURCE]);
        worker.valid = false;
        assert!(matches!(
            schema(
                SOURCE,
                &context,
                &mut worker,
                &instance,
                GRAMMAR,
                &[GRAMMAR],
                deadline,
                &cancelled
            ),
            Err(SourceCommandError::Invalid(_))
        ));
        assert_eq!(worker.calls.len(), 2);
        context.files[0].raw.push(b' ');
        assert!(matches!(
            schema(
                SOURCE,
                &context,
                &mut worker,
                &instance,
                GRAMMAR,
                &[GRAMMAR],
                deadline,
                &cancelled
            ),
            Err(SourceCommandError::Conflict(_))
        ));
        assert_eq!(worker.calls.len(), 2);
    }

    #[test]
    fn bootstrap_contract_captured_historical_request_keeps_command_serialization() {
        let raw = "{\"operation\":\"historical.create\",\"record\":{\"label\":\"История\"},\"schema_version\":\"tos_local_source_command_v1\"}\n".as_bytes();
        let request = cmd::parse(raw).unwrap();
        assert!(validate_captured_request_bytes(raw, &request).is_ok());
        assert!(
            validate_captured_request_bytes(&cmd::published(&request).unwrap(), &request).is_err()
        );
        assert!(validate_captured_request_bytes(&raw[..raw.len() - 1], &request).is_err());
        let mut changed = request.clone();
        cmd::set(&mut changed, "operation", cmd::string("changed")).unwrap();
        assert!(validate_captured_request_bytes(raw, &changed).is_err());
    }

    fn current_package() -> LegacyPackage {
        BTreeMap::from([(
            STREAM.to_owned(),
            br#"{"claim_id":"tos.claim.note-1","subject_ref":"tos.entity.author-1","object":"tos.entity.concept-1","provenance_event_ref":"tos.event.note-1","evidence_refs":["tos.entity.source-1","ToS/source-witnesses/history/episode/evidence.txt"],"counterevidence_refs":["tos.anchor.note-1"]}"#.to_vec(),
        )])
    }

    fn owner_config(schema: &str) -> Vec<u8> {
        format!(
            r#"{{"schema_version":"{schema}","claim_id":"tos.claim.note-1","allowed_form_ids":["tos.form.note-1"]}}"#
        )
        .into_bytes()
    }

    #[test]
    fn pg_historical_identity_selection_is_operation_scoped() {
        let current = current_package();
        let revision = owner_config(REVISION_OWNER);
        let form = owner_config(FORM_OWNER);

        assert!(
            required_identity_tokens(&revision, &current, "describe")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            required_identity_tokens(&revision, &current, "prepare-revise").unwrap(),
            BTreeSet::from([
                "tos.entity.author-1".to_owned(),
                "tos.entity.concept-1".to_owned(),
                "tos.event.note-1".to_owned(),
                "tos.entity.source-1".to_owned(),
                "ToS/source-witnesses/history/episode/evidence.txt".to_owned(),
                "tos.anchor.note-1".to_owned(),
                "tos.form.note-1".to_owned(),
            ])
        );
        assert_eq!(
            required_identity_tokens(&form, &current, "apply").unwrap(),
            BTreeSet::from(["tos.form.note-1".to_owned()])
        );
    }
}
