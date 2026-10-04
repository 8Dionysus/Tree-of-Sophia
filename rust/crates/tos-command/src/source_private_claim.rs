//! Owner-local Claim v1/v2 semantics over an explicitly selected private
//! package. This module never routes private input through the public Claim
//! command executor; storage, locks and publication remain with the private
//! owner store.
use crate::source_command::{self as cmd, *};
use crate::source_forms;
use crate::source_serialization::{self, PrivateMetadataFamily};
use crate::source_sign_native::SignNativeRead;
use crate::source_text_owner::OwnerTextContext;
use crate::source_text_private_store::{
    PrivateArchiveReader, PrivateIdentityInputs, PrivatePackage,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonLimits, JsonValue, RelativePath, emit_python_compact_json};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::assessment::MAX_RECORD_BYTES;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

pub(crate) const CONFIG_V1: &str = "tos_local_owner_claim_command_v1";
pub(crate) const CONFIG_V2: &str = "tos_local_owner_claim_command_v2";
pub(crate) const CLAIM_STREAM: &str = "source-claims.jsonl";
pub(crate) const HISTORY: &str = "claim-revision-history.json";
pub(crate) const CONFIG_FILE: &str = "source-create-owner-configuration.json";
pub(crate) const RECEIPT_FILE: &str = "source-create-receipt.json";
const CREATE_REQUEST: &str = "source-create-request.json";
const CREATE_ENVIRONMENT: &str = "source-create-environment.json";
const CREATE_PROVENANCE: &str = "source-create-provenance.jsonl";
const BASE_PACKAGE_FILES: &[&str] = &[
    CLAIM_STREAM,
    CONFIG_FILE,
    CREATE_REQUEST,
    CREATE_ENVIRONMENT,
    CREATE_PROVENANCE,
    RECEIPT_FILE,
];
const MAX_CLAIM_ROWS: usize = 1024;
const MAX_CLAIM_BYTES: usize = 1_048_576;
const MAX_ASSESSMENT_CLAIM_FILE_BYTES: usize = 16_777_216;
const MAX_PACKAGE_FILES: usize = 64;
const MAX_PACKAGE_BYTES: usize = 8_388_608;
const MAX_REVISIONS: usize = 128;
const MAX_SOURCE_RECORDS: usize = 16;
const MAX_NATIVE_BINDINGS: usize = 8;
const MAX_EVIDENCE_REFS: usize = 64;
const MAX_INVENTORY_ENTRIES: usize = 32_768;
const MAX_INVENTORY_FILES: usize = 2_048;
const MAX_INVENTORY_BYTES: usize = 67_108_864;
const MAX_INVENTORY_FILE_BYTES: usize = 8_388_608;
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CLAIM_REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
const ENTITY_REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const CLAIM_BASE_SCHEMA: &str = "ToS/contracts/source-claim-record.schema.json";
const SOURCE_METADATA_SCHEMA: &str = "ToS/contracts/source-metadata-record.schema.json";
const CLAIM_SHARED_SCHEMAS: &[&str] = &[
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    CLAIM_BASE_SCHEMA,
];
const CORPUS_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
const CONTEXT_SCHEMA_REF: &str = "ToS/contracts/owner-local-source-context.schema.json";
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const REVISION_FIELDS: &[&str] = &[
    "qualifiers",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "supporting_quotes",
    "epistemic_status",
    "confidence",
];
const COMPOUND_PREDICATES: &[&str] = &[
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
const NATIVE_CORPUS_KINDS: &[&str] = &[
    "agent",
    "place",
    "organization",
    "work",
    "expression",
    "edition",
    "collection",
    "item",
];
const FORM_SCHEMAS: &[&str] = &[
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];

/// Semantic output only. The private owner store compares `before`, publishes
/// `files`, and atomically retains `archive`; these bytes are proposals.
pub(crate) struct PrivateClaimPlan {
    pub(crate) before: Option<PrivatePackage>,
    pub(crate) files: Option<PrivatePackage>,
    pub(crate) response: JsonValue,
    pub(crate) archive: Option<(String, PrivatePackage)>,
    pub(crate) reads: Vec<SourceFile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigVersion {
    V1,
    V2,
}
impl ConfigVersion {
    fn schema(self) -> &'static str {
        match self {
            Self::V1 => CONFIG_V1,
            Self::V2 => CONFIG_V2,
        }
    }
    fn reader_allowed(self, value: &str) -> bool {
        match self {
            Self::V1 => matches!(value, "semantic-relation-v1" | "identity-relation-v1"),
            Self::V2 => matches!(
                value,
                "semantic-relation-v1" | "identity-relation-v1" | "structured-reference-value-v1"
            ),
        }
    }
    fn allows_object_revision(self) -> bool {
        self == Self::V2
    }
}

#[derive(Clone, Debug)]
struct Selection {
    claim_id: String,
    relation_type_id: String,
    origin_id: String,
    source_access: JsonValue,
    source_records: Vec<JsonValue>,
    native_bindings: Vec<JsonValue>,
    verify_content: bool,
}

#[derive(Clone, Debug)]
struct NativeSelection {
    key: String,
    binding: JsonValue,
    origin_id: String,
    source_access: JsonValue,
    source_ids: BTreeSet<String>,
}

/// One explicitly selected owner-local Claim and its bounded evidence closure
/// for a read-only assessment. This is deliberately separate from `Selection`
/// and `Grant`: assessment source access carries no caller-authored grant or
/// transaction authority.
#[derive(Clone, Debug)]
pub(crate) struct AssessmentClaimSelection {
    pub(crate) path: String,
    pub(crate) claim_id: String,
    pub(crate) relation_type_id: String,
    pub(crate) origin_id: String,
    pub(crate) source_access: JsonValue,
    pub(crate) source_records: Vec<JsonValue>,
    pub(crate) native_bindings: Vec<JsonValue>,
    pub(crate) verify_content: bool,
    pub(crate) form_ids: Vec<String>,
}

/// Read-only Claim closure returned to the assessment source adapter. Raw
/// bytes stay on the owner-private path; the journal receives envelopes,
/// exact source references, language requirements and snapshot inputs.
pub(crate) struct AssessmentClaimSources {
    pub(crate) records: Vec<JsonValue>,
    pub(crate) native_records: Vec<JsonValue>,
    pub(crate) required_source_refs: BTreeMap<String, Vec<JsonValue>>,
    pub(crate) required_languages: BTreeMap<String, Vec<String>>,
    pub(crate) native_summaries: Vec<JsonValue>,
    pub(crate) native_inputs: Vec<crate::source_sign_native::NativeInput>,
    pub(crate) native_snapshots: Vec<String>,
    pub(crate) schema_digests: BTreeMap<String, Digest256>,
    pub(crate) snapshots: Vec<String>,
    pub(crate) source_files: BTreeMap<String, Vec<u8>>,
    pub(crate) form_sets: BTreeMap<String, JsonValue>,
    pub(crate) form_paths: BTreeMap<String, String>,
    pub(crate) source_paths: BTreeMap<String, String>,
    /// Public semantic-annotation packet membership used by the selected
    /// Claim identity closure. `None` means no selected endpoint triggered
    /// the maintained native-identity reservation scan.
    pub(crate) public_native_identity_paths: Option<BTreeSet<String>>,
}

struct AssessmentClaimScopedReader<'a> {
    reader: &'a mut dyn SignNativeRead,
    allow_content: bool,
}

impl SignNativeRead for AssessmentClaimScopedReader<'_> {
    fn read(
        &mut self,
        reference: &str,
        kind: crate::source_sign_native::NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if kind == crate::source_sign_native::NativeReadKind::Content && !self.allow_content {
            return Err(SourceCommandError::Denied(
                "metadata-only private Claim cannot read native content",
            ));
        }
        self.reader
            .read(reference, kind, max_bytes, deadline, cancelled)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.reader.verify_current(deadline, cancelled)
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        self.reader.owner_local(reference)
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        self.reader.owner_context_snapshot(deadline, cancelled)
    }
}

#[derive(Clone, Debug)]
struct Grant {
    raw: Vec<u8>,
    value: JsonValue,
    version: ConfigVersion,
    selections: BTreeMap<String, Selection>,
    digest: String,
    context_snapshot: String,
    source_path: RelativePath,
    home: String,
}

fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_prefixed()
}
fn dotted_identity(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|tail| {
        !tail.is_empty()
            && tail.split(['.', '-']).all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
    })
}
fn claim_id(value: &str) -> bool {
    dotted_identity(value, "tos.claim.")
}
fn form_id(value: &str) -> bool {
    dotted_identity(value, "tos.form.")
}
fn event_id(value: &str) -> bool {
    dotted_identity(value, "tos.event.")
}
fn exact_list<'a>(
    value: &'a JsonValue,
    key: &str,
    max: usize,
) -> SourceCommandResult<Vec<&'a str>> {
    let values = cmd::array(value, key)?;
    if values.len() > max {
        return Err(SourceCommandError::Invalid(
            "private Claim grant list budget",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        let text = item
            .as_str()
            .filter(|text| cmd::nonblank(text))
            .ok_or(SourceCommandError::Invalid("private Claim grant text"))?;
        if !seen.insert(text) {
            return Err(SourceCommandError::Invalid("duplicate private Claim grant"));
        }
        out.push(text);
    }
    Ok(out)
}
fn optional_array<'a>(value: &'a JsonValue, key: &str) -> SourceCommandResult<&'a [JsonValue]> {
    match value.object_get(key) {
        None | Some(JsonValue::Null) => Ok(&[]),
        Some(JsonValue::Array(items)) => Ok(items),
        Some(_) => Err(SourceCommandError::Invalid("private Claim array field")),
    }
}
fn source_access(value: &JsonValue, allow_exact: bool) -> SourceCommandResult<()> {
    cmd::exact_keys(value, &["read_scope", "access_allowed", "authority_ref"])?;
    let scope = cmd::text(value, "read_scope")?;
    if !matches!(scope, "metadata_only" | "exact_owner_local")
        || cmd::field(value, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(value, "authority_ref")?)
        || !allow_exact && scope != "metadata_only"
    {
        return Err(SourceCommandError::Denied(
            "private Claim source access scope is not explicit",
        ));
    }
    Ok(())
}

/// Parse the maintained v4 assessment Claim selector shape and preflight every
/// nested source-access grant and path before the caller opens any selected
/// source. `exact_owner_local` remains an explicit selection; it does not
/// authorize content reads unless `verify_content` is also true.
pub(crate) fn preflight_assessment_claim_selections(
    value: &JsonValue,
    context: &JsonValue,
) -> SourceCommandResult<Vec<AssessmentClaimSelection>> {
    let rows = value.as_array().ok_or(SourceCommandError::Invalid(
        "private assessment Claim selection array",
    ))?;
    if rows.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private assessment Claim selection count budget",
        ));
    }
    let mut identities = BTreeSet::new();
    let mut claim_ids = BTreeSet::new();
    let mut selections = Vec::with_capacity(rows.len());
    for row in rows {
        cmd::exact_keys(
            row,
            &[
                "path",
                "claim_id",
                "relation_type_id",
                "origin_id",
                "source_access",
                "source_records",
                "native_bindings",
                "verify_content",
                "form_ids",
            ],
        )?;
        let path = cmd::text(row, "path")?.to_owned();
        owner_path_claim_stream(&path, context)?;
        let claim = cmd::text(row, "claim_id")?.to_owned();
        let relation = cmd::text(row, "relation_type_id")?.to_owned();
        let origin = cmd::text(row, "origin_id")?.to_owned();
        let verify_content =
            cmd::field(row, "verify_content")?
                .as_bool()
                .ok_or(SourceCommandError::Invalid(
                    "private assessment Claim content mode",
                ))?;
        if !claim_id(&claim)
            || !cmd::nonblank(&relation)
            || !cmd::nonblank(&origin)
            || !claim_ids.insert(claim.clone())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim selection identity",
            ));
        }
        let source_access_value = cmd::field(row, "source_access")?.clone();
        source_access(&source_access_value, true)?;
        if verify_content && cmd::text(&source_access_value, "read_scope")? != "exact_owner_local" {
            return Err(SourceCommandError::Denied(
                "exact Claim evidence is outside the selected source access scope",
            ));
        }
        let source_records = cmd::array(row, "source_records")?;
        let native_bindings = cmd::array(row, "native_bindings")?;
        if source_records.len() > MAX_SOURCE_RECORDS || native_bindings.len() > MAX_NATIVE_BINDINGS
        {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim closure budget",
            ));
        }
        for selector in source_records {
            cmd::exact_keys(
                selector,
                &[
                    "path",
                    "record_id",
                    "profile_type_id",
                    "origin_id",
                    "source_access",
                    "source_binding",
                ],
            )?;
            for key in ["path", "record_id", "profile_type_id", "origin_id"] {
                if !cmd::nonblank(cmd::text(selector, key)?) {
                    return Err(SourceCommandError::Invalid(
                        "private assessment Claim endpoint selector",
                    ));
                }
            }
            let source_path = cmd::text(selector, "path")?;
            RelativePath::parse(source_path)
                .map_err(|_| SourceCommandError::Invalid("private Claim endpoint path"))?;
            let private = private_reference(source_path, context)?;
            if !source_path.starts_with("ToS/source-witnesses/")
                || source_path.split('/').any(|part| {
                    part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
                })
                || private && !source_path.starts_with(cmd::text(context, "private_prefix")?)
                || !private && source_path.starts_with("ToS/source-witnesses/owner-local/")
            {
                return Err(SourceCommandError::Denied(
                    "private Claim endpoint leaves selected metadata source scope",
                ));
            }
            let binding = cmd::field(selector, "source_binding")?;
            if !binding.is_null() && binding.as_object().is_none() {
                return Err(SourceCommandError::Invalid(
                    "private Claim native source binding",
                ));
            }
            let access = cmd::field(selector, "source_access")?;
            source_access(access, true)?;
            if binding.is_null() && cmd::text(access, "read_scope")? != "metadata_only" {
                return Err(SourceCommandError::Denied(
                    "ordinary Claim endpoint selection is metadata-only",
                ));
            }
            if verify_content
                && !binding.is_null()
                && cmd::text(access, "read_scope")? != "exact_owner_local"
            {
                return Err(SourceCommandError::Denied(
                    "exact bound Claim evidence is outside its source selection access scope",
                ));
            }
        }
        for selector in native_bindings {
            cmd::exact_keys(selector, &["binding", "origin_id", "source_access"])?;
            if cmd::field(selector, "binding")?.as_object().is_none()
                || !cmd::nonblank(cmd::text(selector, "origin_id")?)
            {
                return Err(SourceCommandError::Invalid(
                    "private Claim native binding selector",
                ));
            }
            let access = cmd::field(selector, "source_access")?;
            source_access(access, true)?;
            if verify_content && cmd::text(access, "read_scope")? != "exact_owner_local" {
                return Err(SourceCommandError::Denied(
                    "exact native Claim evidence is outside its selected access scope",
                ));
            }
        }
        let form_values = cmd::array(row, "form_ids")?;
        if form_values.len() > 32 {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim form selection budget",
            ));
        }
        let mut form_ids = Vec::with_capacity(form_values.len());
        for form in form_values {
            let id = form
                .as_str()
                .filter(|value| form_id(value))
                .ok_or(SourceCommandError::Invalid(
                    "private assessment Claim form identity",
                ))?
                .to_owned();
            if !identities.insert(id.clone()) {
                return Err(SourceCommandError::Invalid(
                    "private assessment Claim or form identity repeats",
                ));
            }
            form_ids.push(id);
        }
        if !identities.insert(claim.clone()) {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim or form identity repeats",
            ));
        }
        if !form_ids.is_empty() {
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(SourceCommandError::Invalid("private Claim form path"))?;
            let form_path = format!("{parent}/{}", claim_form_filename(&claim));
            RelativePath::parse(&form_path)
                .map_err(|_| SourceCommandError::Invalid("private Claim form path"))?;
            if !form_path.starts_with(cmd::text(context, "private_prefix")?)
                || form_path.split('/').any(|part| {
                    part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
                })
            {
                return Err(SourceCommandError::Denied(
                    "private Claim forms leave the selected owner package",
                ));
            }
        }
        selections.push(AssessmentClaimSelection {
            path,
            claim_id: claim,
            relation_type_id: relation,
            origin_id: origin,
            source_access: source_access_value,
            source_records: source_records.to_vec(),
            native_bindings: native_bindings.to_vec(),
            verify_content,
            form_ids,
        });
    }
    if identities.len() > tos_validation::assessment::MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment Claim identity budget",
        ));
    }
    Ok(selections)
}
fn parse_selection(value: &JsonValue, verify_all: bool) -> SourceCommandResult<Selection> {
    cmd::exact_keys(
        value,
        &[
            "claim_id",
            "relation_type_id",
            "origin_id",
            "source_access",
            "source_records",
            "native_bindings",
            "verify_content",
        ],
    )?;
    let claim = cmd::text(value, "claim_id")?;
    let relation = cmd::text(value, "relation_type_id")?;
    let origin = cmd::text(value, "origin_id")?;
    if !claim_id(claim)
        || !cmd::nonblank(relation)
        || !cmd::nonblank(origin)
        || cmd::field(value, "verify_content")?.as_bool().is_none()
    {
        return Err(SourceCommandError::Invalid(
            "private Claim source selection identity",
        ));
    }
    let verify_content = cmd::field(value, "verify_content")?
        .as_bool()
        .ok_or(SourceCommandError::Invalid("private Claim content mode"))?;
    let source_records = cmd::array(value, "source_records")?;
    let native_bindings = cmd::array(value, "native_bindings")?;
    if source_records.len() > MAX_SOURCE_RECORDS || native_bindings.len() > MAX_NATIVE_BINDINGS {
        return Err(SourceCommandError::Invalid(
            "private Claim selection closure budget",
        ));
    }
    source_access(cmd::field(value, "source_access")?, true)?;
    if verify_content
        && cmd::text(cmd::field(value, "source_access")?, "read_scope")? != "exact_owner_local"
    {
        return Err(SourceCommandError::Denied(
            "exact Claim evidence is outside the selected source access scope",
        ));
    }
    for row in source_records {
        cmd::exact_keys(
            row,
            &[
                "path",
                "record_id",
                "profile_type_id",
                "origin_id",
                "source_access",
                "source_binding",
            ],
        )?;
        for key in ["path", "record_id", "profile_type_id", "origin_id"] {
            if !cmd::nonblank(cmd::text(row, key)?) {
                return Err(SourceCommandError::Invalid(
                    "private Claim endpoint selector",
                ));
            }
        }
        let path = cmd::text(row, "path")?;
        RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("private Claim endpoint path"))?;
        let bound = !cmd::field(row, "source_binding")?.is_null();
        if bound && cmd::field(row, "source_binding")?.as_object().is_none() {
            return Err(SourceCommandError::Invalid(
                "private Claim native source binding",
            ));
        }
        source_access(cmd::field(row, "source_access")?, true)?;
        if !bound && cmd::text(cmd::field(row, "source_access")?, "read_scope")? != "metadata_only"
        {
            return Err(SourceCommandError::Denied(
                "ordinary Claim endpoint selection is metadata-only",
            ));
        }
        if bound && !verify_content && verify_all {
            return Err(SourceCommandError::Denied(
                "native Claim endpoint needs exact source verification",
            ));
        }
        if bound
            && verify_content
            && cmd::text(cmd::field(row, "source_access")?, "read_scope")? != "exact_owner_local"
        {
            return Err(SourceCommandError::Denied(
                "exact bound Claim evidence is outside its source selection access scope",
            ));
        }
    }
    for row in native_bindings {
        cmd::exact_keys(row, &["binding", "origin_id", "source_access"])?;
        if cmd::field(row, "binding")?.as_object().is_none()
            || !cmd::nonblank(cmd::text(row, "origin_id")?)
        {
            return Err(SourceCommandError::Invalid(
                "private Claim native binding selector",
            ));
        }
        source_access(cmd::field(row, "source_access")?, verify_content)?;
        if !verify_content && verify_all {
            return Err(SourceCommandError::Denied(
                "native Claim evidence needs exact source verification",
            ));
        }
    }
    Ok(Selection {
        claim_id: claim.to_owned(),
        relation_type_id: relation.to_owned(),
        origin_id: origin.to_owned(),
        source_access: cmd::field(value, "source_access")?.clone(),
        source_records: source_records.to_vec(),
        native_bindings: native_bindings.to_vec(),
        verify_content,
    })
}

fn validate_grant(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Grant> {
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
    let store_id = cmd::text(context_config, "store_id")?;
    if cmd::text(context_config, "schema_version")? != "tos_owner_local_source_context_v1"
        || cmd::text(context_config, "public_root")? != owner.public_root().to_str().unwrap_or("")
        || cmd::text(context_config, "private_root")? != owner.private_root().to_str().unwrap_or("")
        || store_id.len() != 36
        || !store_id.starts_with("sid-")
        || !store_id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit)
        || store_id.as_bytes()[4..].iter().any(u8::is_ascii_uppercase)
        || cmd::text(context_config, "private_prefix")?
            != format!("ToS/source-witnesses/owner-local/{store_id}/")
    {
        return Err(SourceCommandError::Conflict(
            "private Claim context selection differs",
        ));
    }
    let raw = ctx.configuration_raw.clone();
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "private Claim grant byte budget",
        ));
    }
    let value = cmd::parse(&raw)?;
    let version = match cmd::text(&value, "schema_version")? {
        CONFIG_V1 => ConfigVersion::V1,
        CONFIG_V2 => ConfigVersion::V2,
        _ => {
            return Err(SourceCommandError::Unsupported(
                "private Claim grant version",
            ));
        }
    };
    let mut keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "maker_type",
        "authority_ref",
        "expires_at",
        "source_context_ref",
        "source_path",
        "provenance_event_id",
        "allowed_operations",
        "allowed_claim_ids",
        "allowed_subject_refs",
        "allowed_object_refs",
        "allowed_predicates",
        "allowed_evidence_refs",
        "allowed_form_ids",
        "allowed_fields",
        "claim_selections",
    ];
    if version == ConfigVersion::V2 {
        keys.push("allowed_object_values");
    }
    cmd::exact_keys(&value, &keys)?;
    if cmd::integer(&value, "uid")? != ctx.effective_uid
        || !cmd::nonblank(cmd::text(&value, "principal_id")?)
        || !cmd::nonblank(cmd::text(&value, "authority_ref")?)
        || !matches!(
            cmd::text(&value, "maker_type")?,
            "human" | "software" | "model"
        )
    {
        return Err(SourceCommandError::Denied("private Claim local identity"));
    }
    cmd::validate_expiry(
        cmd::text(&value, "expires_at")?,
        &source_serialization::instant()?,
    )?;
    let allowed_ops = exact_list(&value, "allowed_operations", 4)?;
    if allowed_ops.iter().any(|op| {
        !matches!(
            *op,
            "claims.create" | "claim.revise" | "form.create" | "form.revise"
        )
    }) {
        return Err(SourceCommandError::Invalid("private Claim operation grant"));
    }
    let allowed_claims = exact_list(&value, "allowed_claim_ids", 32)?;
    let allowed_forms = exact_list(&value, "allowed_form_ids", 32)?;
    let allowed_subjects = exact_list(&value, "allowed_subject_refs", 128)?;
    let allowed_objects = exact_list(&value, "allowed_object_refs", 128)?;
    let allowed_predicates = exact_list(&value, "allowed_predicates", 32)?;
    let allowed_evidence = exact_list(&value, "allowed_evidence_refs", 128)?;
    let allowed_fields = exact_list(&value, "allowed_fields", 8)?;
    if allowed_claims.iter().any(|v| !claim_id(v))
        || allowed_forms.iter().any(|v| !form_id(v))
        || !event_id(cmd::text(&value, "provenance_event_id")?)
        || allowed_fields.iter().any(|field| {
            !REVISION_FIELDS.contains(field)
                && !(version.allows_object_revision() && *field == "object")
        })
    {
        return Err(SourceCommandError::Invalid(
            "private Claim identity or revision scope",
        ));
    }
    if version == ConfigVersion::V2 {
        let values = cmd::array(&value, "allowed_object_values")?;
        if values.len() > 32
            || values.iter().any(|v| v.as_object().is_none())
            || duplicate_values(values)?
        {
            return Err(SourceCommandError::Invalid(
                "private Claim exact object-value scope",
            ));
        }
    }
    if allowed_subjects
        .iter()
        .chain(allowed_objects.iter())
        .chain(allowed_evidence.iter())
        .any(|reference| !cmd::nonblank(reference) || reference.contains('\0'))
    {
        return Err(SourceCommandError::Invalid("private Claim reference grant"));
    }
    let selections_raw = cmd::array(&value, "claim_selections")?;
    if !(1..=32).contains(&selections_raw.len()) {
        return Err(SourceCommandError::Invalid(
            "private Claim independent selection count",
        ));
    }
    let mut selections = BTreeMap::new();
    for selection_raw in selections_raw {
        let selection = parse_selection(selection_raw, true)?;
        if !allowed_claims.contains(&selection.claim_id.as_str())
            || selections
                .insert(selection.claim_id.clone(), selection)
                .is_some()
        {
            return Err(SourceCommandError::Denied(
                "private Claim selection is repeated or outside its identity grant",
            ));
        }
    }
    if selections.len() != allowed_claims.len()
        || allowed_claims
            .iter()
            .any(|claim| !selections.contains_key(*claim))
    {
        return Err(SourceCommandError::Denied(
            "each delegated Claim needs its own exact source selection",
        ));
    }
    let private_prefix = cmd::text(context_config, "private_prefix")?;
    let path = cmd::text(&value, "source_path")?;
    let relative = path
        .strip_prefix(private_prefix)
        .ok_or(SourceCommandError::Denied("private Claim store prefix"))?;
    let source_path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("private Claim source path"))?;
    let parts = relative.split('/').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[0] != "claims"
        || parts[1].is_empty()
        || !dotted_identity(&format!("tos.slug.{}", parts[1]), "tos.slug.")
        || parts[2] != CLAIM_STREAM
    {
        return Err(SourceCommandError::Denied(
            "private Claim source path must name one local claims package",
        ));
    }
    let home = format!("{}claims/{}/", private_prefix, parts[1]);
    // `source_context_ref` names the protected context configuration file,
    // not a child of either selected root. The entry point binds it to the
    // actual file before creating this call. Keep the selected roots live.
    let context_ref = cmd::text(&value, "source_context_ref")?;
    if !context_ref.starts_with('/') || context_ref.contains('\0') {
        return Err(SourceCommandError::Denied(
            "private Claim context reference",
        ));
    }
    let context_snapshot = owner.snapshot(deadline, cancelled)?.to_prefixed();
    Ok(Grant {
        raw,
        value,
        version,
        selections,
        digest: String::new(),
        context_snapshot,
        source_path,
        home,
    })
}

fn duplicate_values(values: &[JsonValue]) -> SourceCommandResult<bool> {
    let mut seen = BTreeSet::new();
    for value in values {
        let bytes = cmd::canonical(value)?;
        if !seen.insert(bytes) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_claims(raw: &[u8]) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    if raw.len() > MAX_CLAIM_BYTES {
        return Err(SourceCommandError::Invalid(
            "private Claim stream byte budget",
        ));
    }
    let mut records = BTreeMap::new();
    for line in raw.split(|byte| *byte == b'\n') {
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        if line.len() > MAX_CLAIM_BYTES {
            return Err(SourceCommandError::Invalid("private Claim row byte budget"));
        }
        let row = cmd::parse(line)?;
        let id = cmd::text(&row, "claim_id")?.to_owned();
        if !claim_id(&id) || records.insert(id, row).is_some() || records.len() > MAX_CLAIM_ROWS {
            return Err(SourceCommandError::Conflict(
                "private Claim stream has duplicate or excessive rows",
            ));
        }
    }
    Ok(records)
}

fn parse_assessment_claim_stream(raw: &[u8]) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    if raw.len() > MAX_ASSESSMENT_CLAIM_FILE_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment Claim stream byte budget",
        ));
    }
    let mut records = BTreeMap::new();
    for line in raw.split(|byte| *byte == b'\n') {
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        if line.len() > MAX_RECORD_BYTES {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim row byte budget",
            ));
        }
        let row = cmd::parse(line)?;
        let id = cmd::text(&row, "claim_id")?.to_owned();
        if !claim_id(&id) || records.insert(id, row).is_some() || records.len() > MAX_CLAIM_ROWS {
            return Err(SourceCommandError::Conflict(
                "private assessment Claim stream has duplicate or excessive rows",
            ));
        }
    }
    Ok(records)
}

fn encode(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    let mut raw = cmd::canonical(value)?;
    raw.push(b'\n');
    Ok(raw)
}

fn package_revision(files: &PrivatePackage) -> SourceCommandResult<String> {
    crate::source_revisions::revision(files)
}

fn archive_ref(
    record_id: &str,
    revision: &str,
    context: &JsonValue,
) -> SourceCommandResult<String> {
    let private_prefix = cmd::text(context, "private_prefix")?;
    let id_digest = Digest256::of_bytes(record_id.as_bytes()).to_hex();
    let rev = revision
        .strip_prefix("sha256:")
        .ok_or(SourceCommandError::Invalid(
            "private Claim archive revision",
        ))?;
    Ok(format!(
        "{}.record-revisions/{}-{}",
        private_prefix, &id_digest, rev
    ))
}

fn archive_package(
    grant: &Grant,
    context: &JsonValue,
    subject: &JsonValue,
    revision: &str,
    files: &PrivatePackage,
) -> SourceCommandResult<(String, PrivatePackage)> {
    let source = archive_ref(cmd::text(subject, "id")?, revision, context)?;
    let refs = crate::source_revisions::file_refs(files, true);
    let mut archived = PrivatePackage::new();
    for (name, raw) in files {
        let blob = format!("{}.blob", Digest256::of_bytes(raw).to_hex());
        if let Some(existing) = archived.insert(blob, raw.clone())
            && existing != *raw
        {
            return Err(SourceCommandError::Invalid(
                "Claim archive digest collision",
            ));
        }
    }
    let manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_source_package_archive_v1"),
        ),
        (
            "source_path",
            cmd::field(&grant.value, "source_path")?.clone(),
        ),
        ("source", subject.clone()),
        ("revision", cmd::string(revision)),
        ("files", refs),
    ]);
    archived.insert("manifest.json".into(), cmd::published(&manifest)?);
    if archived.len() > MAX_PACKAGE_FILES
        || archived.values().map(Vec::len).sum::<usize>() > 16_777_216
    {
        return Err(SourceCommandError::Unsupported(
            "private Claim archive budget",
        ));
    }
    Ok((source, archived))
}

fn read_archive(
    grant: &Grant,
    context: &JsonValue,
    archives: &mut (impl PrivateArchiveReader + ?Sized),
    receipt: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(PrivatePackage, JsonValue)> {
    let subject = cmd::field(receipt, "previous_source")?;
    let previous_revision = cmd::text(receipt, "previous_revision")?;
    let logical = archive_ref(cmd::text(subject, "id")?, previous_revision, context)?;
    if cmd::text(receipt, "archive_path")? != logical {
        return Err(SourceCommandError::Conflict(
            "private Claim archive path differs",
        ));
    }
    let mut archive = archives.read_archive(&logical, deadline, cancelled)?;
    let manifest_raw = archive
        .remove("manifest.json")
        .ok_or(SourceCommandError::Invalid(
            "private Claim archive manifest absent",
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
        || cmd::field(&manifest, "source")? != subject
        || cmd::text(&manifest, "revision")? != previous_revision
    {
        return Err(SourceCommandError::Conflict(
            "private Claim archive binding differs",
        ));
    }
    let bindings =
        cmd::field(&manifest, "files")?
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "private Claim archive file map",
            ))?;
    if bindings.is_empty() || bindings.len() > MAX_PACKAGE_FILES {
        return Err(SourceCommandError::Invalid(
            "private Claim archive file count",
        ));
    }
    let mut files = PrivatePackage::new();
    let mut locations = Vec::new();
    let mut used_blobs = BTreeSet::new();
    for (name, binding) in bindings {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("Claim archive filename"))?;
        if name.is_empty() || name.contains('/') || name.starts_with('.') {
            return Err(SourceCommandError::Invalid("Claim archive filename"));
        }
        cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let hash = cmd::text(binding, "sha256")?;
        let blob = cmd::text(binding, "blob")?;
        if !hash.starts_with("sha256:") || blob != format!("{}.blob", &hash[7..]) {
            return Err(SourceCommandError::Invalid("Claim archive blob locator"));
        }
        let raw = archive
            .get(blob)
            .cloned()
            .ok_or(SourceCommandError::Conflict("Claim archive blob absent"))?;
        used_blobs.insert(blob.to_owned());
        if digest(&raw) != hash || raw.len() as u64 != cmd::integer(binding, "bytes")? {
            return Err(SourceCommandError::Conflict("Claim archive fixity differs"));
        }
        locations.push((
            name.to_owned(),
            cmd::object(vec![
                ("archive_path", cmd::string(&format!("{logical}/{blob}"))),
                ("sha256", cmd::string(hash)),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        ));
        files.insert(name.to_owned(), raw);
    }
    if archive
        .keys()
        .any(|member| member != "manifest.json" && !used_blobs.contains(member))
        || package_revision(&files)? != previous_revision
        || !files.contains_key(CLAIM_STREAM)
    {
        return Err(SourceCommandError::Conflict(
            "Claim archive package differs",
        ));
    }
    Ok((
        files,
        JsonValue::Object(
            locations
                .into_iter()
                .map(|(name, location)| (tos_foundation::JsonString::from_utf8(&name), location))
                .collect(),
        ),
    ))
}

fn record_ref(record: &JsonValue) -> SourceCommandResult<JsonValue> {
    source_forms::metadata_subject(record)
        .map_err(|_| SourceCommandError::Invalid("private Claim source subject"))
}

fn insert_claim_dependency(
    records: &mut BTreeMap<String, JsonValue>,
    claim_id: &str,
    record: JsonValue,
) -> SourceCommandResult<()> {
    let identity = cmd::text(&record, "id")?.to_owned();
    if identity == claim_id {
        return Err(SourceCommandError::Conflict(
            "private Claim and source evidence cannot shadow one identity",
        ));
    }
    if let Some(previous) = records.get(&identity) {
        if !same_json(previous, &record)? {
            return Err(SourceCommandError::Conflict(
                "private Claim evidence identity resolves to different selected records",
            ));
        }
    } else {
        records.insert(identity, record);
    }
    Ok(())
}

#[derive(Default)]
struct ExactReads {
    files: BTreeMap<String, Vec<u8>>,
}
impl ExactReads {
    fn retain(&mut self, reference: &str, raw: Vec<u8>) -> SourceCommandResult<()> {
        if let Some(previous) = self.files.get(reference) {
            if previous != &raw {
                return Err(SourceCommandError::Conflict(
                    "private Claim input changed between exact reads",
                ));
            }
        } else {
            self.files.insert(reference.to_owned(), raw);
        }
        Ok(())
    }
    fn into_source_files(self) -> SourceCommandResult<Vec<SourceFile>> {
        self.files
            .into_iter()
            .map(|(path, raw)| {
                Ok(SourceFile {
                    path: RelativePath::parse(&path)
                        .map_err(|_| SourceCommandError::Invalid("private Claim read path"))?,
                    raw,
                })
            })
            .collect()
    }
}

fn retain_assessment_native_inputs(
    reader: &mut dyn SignNativeRead,
    reads: &mut ExactReads,
    inputs: &[crate::source_sign_native::NativeInput],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for input in inputs {
        let raw = reader.read(
            &input.reference,
            input.kind,
            input.raw_size.max(1),
            deadline,
            cancelled,
        )?;
        if raw.len() != input.raw_size || Digest256::of_bytes(&raw) != input.raw_sha256 {
            return Err(SourceCommandError::Conflict(
                "private Claim native input differs from its resolved bytes",
            ));
        }
        reads.retain(&input.reference, raw)?;
    }
    Ok(())
}

fn private_reference(reference: &str, context: &JsonValue) -> SourceCommandResult<bool> {
    let prefix = cmd::text(context, "private_prefix")?;
    Ok(reference.starts_with(prefix))
}

fn selected_owner_read(
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    reference: &str,
    limit: usize,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Invalid("private Claim selected source path"))?;
    let raw = owner.read(reference, limit, deadline, cancelled)?;
    if !private_reference(reference, context)? {
        let metadata = cut
            .current()
            .member(&path)
            .ok_or(SourceCommandError::Conflict(
                "private Claim public source is outside the current cut",
            ))?;
        if raw.len() as u64 != metadata.size_bytes || Digest256::of_bytes(&raw) != metadata.sha256 {
            return Err(SourceCommandError::Conflict(
                "private Claim public source differs from current cut",
            ));
        }
    }
    reads.retain(reference, raw.clone())?;
    Ok(raw)
}

fn selected_authored(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    reference: &str,
    limit: usize,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = selected_owner_read(
        owner, cut, context, reference, limit, reads, deadline, cancelled,
    )?;
    if !private_reference(reference, context)? {
        let selected =
            ctx.file(&RelativePath::parse(reference).map_err(|_| {
                SourceCommandError::Invalid("private Claim current authored path")
            })?)?
            .ok_or(SourceCommandError::Unsupported(
                "private Claim exact authored dependency is not selected",
            ))?;
        if selected != raw {
            return Err(SourceCommandError::Conflict(
                "private Claim authored input differs from selected command context",
            ));
        }
    }
    Ok(raw)
}

fn selected_schema(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    reference: &str,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let raw = selected_authored(
        ctx, owner, cut, context, reference, 1_048_576, reads, deadline, cancelled,
    )?;
    let schema = cmd::parse(&raw)?;
    let expected = format!("https://tree-of-sophia.local/{reference}");
    let alternate = format!("https://treeofsophia.local/{reference}");
    let id = cmd::text(&schema, "$id")?;
    if id != expected && id != alternate {
        return Err(SourceCommandError::Conflict(
            "private Claim schema identity differs from its owner path",
        ));
    }
    Ok(schema)
}

fn check_schema(
    worker: &mut CutWorkerSchemaExecutor,
    label: &str,
    raw: &[u8],
    contract: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let valid = worker
        .check_reusing_scalar(label, raw, contract, deadline, cancelled)
        .map_err(|_| SourceCommandError::Unsupported("private Claim selected schema worker"))?;
    if !valid {
        return Err(SourceCommandError::Conflict(
            "private Claim exact source violates its schema",
        ));
    }
    Ok(())
}

fn require_contract_digest(
    worker: &CutWorkerSchemaExecutor,
    reference: &str,
    raw: &[u8],
) -> SourceCommandResult<()> {
    if worker.contract_digest(reference) != Some(Digest256::of_bytes(raw)) {
        return Err(SourceCommandError::Conflict(
            "private Claim selected contract and schema worker differ",
        ));
    }
    Ok(())
}

fn field_texts(value: &JsonValue, field: &str, max: usize) -> SourceCommandResult<Vec<String>> {
    let values = cmd::array(value, field)?;
    if values.len() > max {
        return Err(SourceCommandError::Invalid(
            "private Claim registry list budget",
        ));
    }
    let mut result = Vec::with_capacity(values.len());
    let mut unique = BTreeSet::new();
    for value in values {
        let item = value
            .as_str()
            .ok_or(SourceCommandError::Invalid("private Claim registry text"))?;
        if !unique.insert(item) {
            return Err(SourceCommandError::Invalid(
                "private Claim duplicate registry member",
            ));
        }
        result.push(item.to_owned());
    }
    Ok(result)
}

#[derive(Clone)]
struct ClaimRoute {
    relation_type_id: String,
    predicate: String,
    reader: String,
    profile: JsonValue,
    domain: Vec<String>,
    range: Vec<String>,
    assertion_layers: Vec<String>,
    schema_ref: String,
    schema_dependencies: Vec<String>,
}

struct ClaimGrammar {
    relations: JsonValue,
    entities: JsonValue,
    routes: BTreeMap<String, ClaimRoute>,
    entity_by_id: BTreeMap<String, JsonValue>,
    profile_by_type: BTreeMap<String, JsonValue>,
    type_by_kind: BTreeMap<String, String>,
    schema_digests: BTreeMap<String, String>,
}

impl ClaimGrammar {
    fn load(
        ctx: &CommandContext,
        owner: &OwnerTextContext,
        cut: &CorpusCutReader,
        context: &JsonValue,
        worker: &mut CutWorkerSchemaExecutor,
        reads: &mut ExactReads,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        if cut.current().revision() != ctx.base_revision
            || worker.source_revision() != ctx.base_revision
        {
            return Err(SourceCommandError::Conflict(
                "private Claim grammar worker and selected source cut differ",
            ));
        }
        let entities = crate::source_revisions::validate_source_profile_registry(
            worker, deadline, cancelled, ctx,
        )?;
        let entity_registry_schema = selected_schema(
            ctx,
            owner,
            cut,
            context,
            ENTITY_REGISTRY_SCHEMA,
            reads,
            deadline,
            cancelled,
        )?;
        let entity_registry_raw = selected_authored(
            ctx, owner, cut, context, ENTITIES, 8_388_608, reads, deadline, cancelled,
        )?;
        check_schema(
            worker,
            ENTITIES,
            &entity_registry_raw,
            ENTITY_REGISTRY_SCHEMA,
            deadline,
            cancelled,
        )?;
        require_contract_digest(
            worker,
            ENTITY_REGISTRY_SCHEMA,
            reads
                .files
                .get(ENTITY_REGISTRY_SCHEMA)
                .ok_or(SourceCommandError::Invalid(
                    "Claim entity schema input absent",
                ))?,
        )?;
        if cmd::parse(&entity_registry_raw)? != entities {
            return Err(SourceCommandError::Conflict(
                "private Claim entity registry changed after its worker validation",
            ));
        }
        let mut entity_by_id = BTreeMap::new();
        let mut profile_by_type = BTreeMap::new();
        let mut type_by_kind = BTreeMap::new();
        for entry in cmd::array(&entities, "types")? {
            let type_id = cmd::text(entry, "type_id")?.to_owned();
            if entity_by_id
                .insert(type_id.clone(), entry.clone())
                .is_some()
            {
                return Err(SourceCommandError::Invalid(
                    "duplicate private Claim entity type",
                ));
            }
            for mapping in cmd::array(entry, "source_mappings")? {
                if cmd::text(mapping, "source_graph")? == "source-claims" {
                    let kind = cmd::text(mapping, "source_kind_id")?.to_owned();
                    if type_by_kind.insert(kind, type_id.clone()).is_some() {
                        return Err(SourceCommandError::Conflict(
                            "private Claim source kind has duplicate type mapping",
                        ));
                    }
                }
            }
            if let Some(profile) = entry.object_get("source_record_profile") {
                let kind = cmd::text(profile, "record_type")?.to_owned();
                if !type_by_kind.contains_key(&kind) || type_by_kind.get(&kind) != Some(&type_id) {
                    return Err(SourceCommandError::Conflict(
                        "private Claim profile type mapping differs",
                    ));
                }
                profile_by_type.insert(type_id, profile.clone());
            }
        }
        let relation_schema = selected_schema(
            ctx,
            owner,
            cut,
            context,
            CLAIM_REGISTRY_SCHEMA,
            reads,
            deadline,
            cancelled,
        )?;
        let relations_raw = selected_authored(
            ctx, owner, cut, context, RELATIONS, 8_388_608, reads, deadline, cancelled,
        )?;
        check_schema(
            worker,
            RELATIONS,
            &relations_raw,
            CLAIM_REGISTRY_SCHEMA,
            deadline,
            cancelled,
        )?;
        require_contract_digest(
            worker,
            CLAIM_REGISTRY_SCHEMA,
            reads
                .files
                .get(CLAIM_REGISTRY_SCHEMA)
                .ok_or(SourceCommandError::Invalid(
                    "Claim relation schema input absent",
                ))?,
        )?;
        let relations = cmd::parse(&relations_raw)?;
        let mut routes = BTreeMap::new();
        let mut type_ids = BTreeSet::new();
        for entry in cmd::array(&relations, "relations")? {
            let id = cmd::text(entry, "relation_type_id")?;
            if !type_ids.insert(id) {
                return Err(SourceCommandError::Invalid(
                    "duplicate private Claim relation type",
                ));
            }
            let Some(profile) = entry.object_get("source_claim_profile") else {
                continue;
            };
            let mut mappings = Vec::new();
            for mapping in cmd::array(entry, "source_mappings")? {
                if cmd::text(mapping, "source_graph")? == "source-claims"
                    && cmd::text(mapping, "scope")? == "claim-predicate"
                {
                    mappings.push(mapping);
                }
            }
            if mappings.len() != 1
                || cmd::field(entry, "abstract")? != &JsonValue::Bool(false)
                || cmd::text(entry, "assertion_mode")? != "reified-claim"
                || cmd::field(entry, "evidence_required")? != &JsonValue::Bool(true)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim relation profile is not one evidence-bearing reified predicate",
                ));
            }
            let predicate = cmd::text(mappings[0], "source_predicate_id")?.to_owned();
            let reader = cmd::text(profile, "reader")?;
            if !matches!(
                reader,
                "semantic-relation-v1"
                    | "identity-relation-v1"
                    | "structured-reference-value-v1"
                    | "historical-temporal-v1"
            ) || cmd::integer(profile, "profile_version")? != 1
            {
                continue;
            }
            let domain = field_texts(entry, "domain_type_ids", 32)?;
            let range = field_texts(entry, "range_type_ids", 32)?;
            if domain.is_empty() || range.is_empty() {
                return Err(SourceCommandError::Invalid(
                    "private Claim relation endpoint sets",
                ));
            }
            for endpoint in domain.iter().chain(range.iter()) {
                let ancestry =
                    crate::source_claims::ancestry(cmd::array(&entities, "types")?, endpoint)?;
                if matches!(
                    endpoint.as_str(),
                    "tos.entity.thing"
                        | "tos.entity.identity"
                        | "tos.entity.semantic-object"
                        | "tos.entity.unmapped"
                        | "tos.entity.unresolved-endpoint"
                ) {
                    return Err(SourceCommandError::Conflict(
                        "private Claim relation uses an abstract endpoint family",
                    ));
                }
                match reader {
                    "identity-relation-v1" if !ancestry.contains("tos.entity.identity") => {
                        return Err(SourceCommandError::Conflict(
                            "private Claim identity profile endpoint family",
                        ));
                    }
                    "semantic-relation-v1"
                        if !ancestry.contains("tos.entity.identity")
                            && !ancestry.contains("tos.entity.semantic-object") =>
                    {
                        return Err(SourceCommandError::Conflict(
                            "private Claim semantic profile endpoint family",
                        ));
                    }
                    "structured-reference-value-v1"
                        if !ancestry.contains("tos.entity.identity")
                            && !ancestry.contains("tos.entity.semantic-object")
                            && !ancestry.contains("tos.entity.literal") =>
                    {
                        return Err(SourceCommandError::Conflict(
                            "private Claim structured endpoint family",
                        ));
                    }
                    _ => (),
                }
            }
            if reader == "historical-temporal-v1" {
                for endpoint in &domain {
                    let ancestry =
                        crate::source_claims::ancestry(cmd::array(&entities, "types")?, endpoint)?;
                    if !ancestry.contains("tos.entity.historical-situation") {
                        return Err(SourceCommandError::Conflict(
                            "private Claim temporal profile domain family",
                        ));
                    }
                }
                for endpoint in &range {
                    let ancestry =
                        crate::source_claims::ancestry(cmd::array(&entities, "types")?, endpoint)?;
                    if !ancestry.contains("tos.entity.temporal-assertion") {
                        return Err(SourceCommandError::Conflict(
                            "private Claim temporal profile value family",
                        ));
                    }
                }
            }
            if routes.contains_key(&predicate) {
                return Err(SourceCommandError::Conflict(
                    "private Claim predicate has duplicate owner",
                ));
            }
            let mut schema_routes = BTreeSet::new();
            for route in cmd::array(profile, "schemas")? {
                if !schema_routes.insert(cmd::text(route, "schema_version")?) {
                    return Err(SourceCommandError::Invalid(
                        "duplicate private Claim schema route",
                    ));
                }
            }
            let selected_route =
                cmd::array(profile, "schemas")?
                    .first()
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim empty schema route",
                    ))?;
            let schema_ref = cmd::text(selected_route, "schema_ref")?.to_owned();
            let schema_dependencies = field_texts(selected_route, "schema_dependencies", 128)?;
            routes.insert(
                predicate.clone(),
                ClaimRoute {
                    relation_type_id: id.to_owned(),
                    predicate,
                    reader: reader.to_owned(),
                    profile: profile.clone(),
                    domain,
                    range,
                    assertion_layers: field_texts(profile, "assertion_layers", 32)?,
                    schema_ref,
                    schema_dependencies,
                },
            );
        }
        let mut schema_digests = BTreeMap::new();
        schema_digests.insert(
            RELATIONS.to_owned(),
            Digest256::of_bytes(&relations_raw).to_prefixed(),
        );
        schema_digests.insert(
            CLAIM_REGISTRY_SCHEMA.to_owned(),
            reads
                .files
                .get(CLAIM_REGISTRY_SCHEMA)
                .map(|raw| Digest256::of_bytes(raw).to_prefixed())
                .ok_or(SourceCommandError::Invalid(
                    "Claim relation schema input absent",
                ))?,
        );
        schema_digests.insert(
            ENTITIES.to_owned(),
            Digest256::of_bytes(&entity_registry_raw).to_prefixed(),
        );
        schema_digests.insert(
            ENTITY_REGISTRY_SCHEMA.to_owned(),
            reads
                .files
                .get(ENTITY_REGISTRY_SCHEMA)
                .map(|raw| Digest256::of_bytes(raw).to_prefixed())
                .ok_or(SourceCommandError::Invalid(
                    "Claim entity schema input absent",
                ))?,
        );
        let _ = (relation_schema, entity_registry_schema);
        Ok(Self {
            relations,
            entities,
            routes,
            entity_by_id,
            profile_by_type,
            type_by_kind,
            schema_digests,
        })
    }

    fn route(&self, predicate: &str, schema_version: &str) -> SourceCommandResult<ClaimRoute> {
        let route = self
            .routes
            .get(predicate)
            .ok_or(SourceCommandError::Unsupported(
                "private Claim predicate profile",
            ))?;
        let mut selected = None;
        for row in cmd::array(&route.profile, "schemas")? {
            if cmd::text(row, "schema_version")? == schema_version
                && selected.replace(row).is_some()
            {
                return Err(SourceCommandError::Unsupported(
                    "private Claim schema-version route is duplicated",
                ));
            }
        }
        let Some(selected) = selected else {
            return Err(SourceCommandError::Unsupported(
                "private Claim exact schema-version route",
            ));
        };
        let mut result = route.clone();
        result.schema_ref = cmd::text(selected, "schema_ref")?.to_owned();
        result.schema_dependencies = field_texts(selected, "schema_dependencies", 128)?;
        Ok(result)
    }

    fn type_for_source(&self, kind: &str) -> SourceCommandResult<&str> {
        self.type_by_kind
            .get(kind)
            .map(String::as_str)
            .ok_or(SourceCommandError::Unsupported(
                "private Claim source type mapping",
            ))
    }
}

fn validate_allowed_predicates(grant: &Grant, grammar: &ClaimGrammar) -> SourceCommandResult<()> {
    for predicate in exact_list(&grant.value, "allowed_predicates", 32)? {
        let route = grammar
            .routes
            .get(predicate)
            .ok_or(SourceCommandError::Denied(
                "private Claim grant names an unsupported relation profile",
            ))?;
        if !grant.version.reader_allowed(&route.reader) {
            return Err(SourceCommandError::Denied(
                "private Claim reader mode is outside this delegation version",
            ));
        }
    }
    Ok(())
}

fn inventory_ids(value: &JsonValue, output: &mut BTreeSet<String>) -> SourceCommandResult<()> {
    for key in ["record_id", "claim_id", "event_id"] {
        if let Some(identity) = value.object_get(key).and_then(JsonValue::as_str) {
            output.insert(identity.to_owned());
        }
    }
    for (array_name, field) in [
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
        if let Some(rows) = value.object_get(array_name).and_then(JsonValue::as_array) {
            for row in rows {
                if let Some(identity) = row.object_get(field).and_then(JsonValue::as_str) {
                    output.insert(identity.to_owned());
                }
            }
        }
    }
    Ok(())
}

fn inventory_value(raw: &[u8], jsonl: bool) -> SourceCommandResult<Vec<JsonValue>> {
    if jsonl {
        let mut rows = Vec::new();
        for line in raw.split(|b| *b == b'\n') {
            if crate::source_claims::python_bytes_blank(line) {
                continue;
            }
            rows.push(cmd::parse(line)?);
            if rows.len() > 65_536 {
                return Err(SourceCommandError::Invalid(
                    "private Claim identity row budget",
                ));
            }
        }
        Ok(rows)
    } else if crate::source_claims::python_bytes_blank(raw) {
        Ok(Vec::new())
    } else {
        Ok(vec![cmd::parse(raw)?])
    }
}

fn native_identity_subject(identity: &str) -> bool {
    [
        "tos.occurrence.",
        "tos.lexeme.",
        "tos.sense.",
        "tos.sign.",
        "tos.concept.",
    ]
    .iter()
    .any(|prefix| identity.starts_with(prefix))
}

fn public_native_identity_member(reference: &str) -> bool {
    let basename = reference.rsplit('/').next().unwrap_or(reference);
    reference.starts_with("ToS/source-witnesses/")
        && basename.starts_with("semantic-annotation")
        && basename.ends_with(".json")
        && !reference
            .split('/')
            .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
}

fn native_identity_inventory_context(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommandContext> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "private Claim native identity cut differs from command context",
        ));
    }
    let mut selected = ctx.clone();
    let mut packet_count = 0usize;
    let mut packet_bytes = 0usize;
    for member in cut.current().members() {
        let reference = member.path.as_str();
        if reference == "ToS/source-witnesses/owner-local"
            || reference.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace cannot enter public native identity inventory",
            ));
        }
        if !public_native_identity_member(reference) {
            continue;
        }
        packet_count = packet_count
            .checked_add(1)
            .filter(|count| *count <= 1024)
            .ok_or(SourceCommandError::Invalid(
                "private Claim native identity packet-count budget",
            ))?;
        let remaining = 8_388_608usize.saturating_sub(packet_bytes);
        let max_bytes = remaining.min(1_048_576);
        let size = usize::try_from(member.size_bytes).map_err(|_| {
            SourceCommandError::Invalid("private Claim native identity packet byte budget")
        })?;
        if size > max_bytes {
            return Err(SourceCommandError::Invalid(
                "private Claim native identity packet byte budget",
            ));
        }
        if let Some(existing) = selected.files.iter().find(|file| file.path == member.path) {
            if existing.raw.len() as u64 != member.size_bytes
                || Digest256::of_bytes(&existing.raw) != member.sha256
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim native identity context differs from selected cut",
                ));
            }
            packet_bytes =
                packet_bytes
                    .checked_add(existing.raw.len())
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim native identity packet byte budget",
                    ))?;
            continue;
        }
        let observed = cut
            .read_member(
                ctx.base_revision,
                &member.path,
                max_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported(
                    "exact private Claim native identity member read unavailable",
                )
            })?;
        if observed.raw.len() as u64 != member.size_bytes
            || Digest256::of_bytes(&observed.raw) != member.sha256
        {
            return Err(SourceCommandError::Conflict(
                "private Claim native identity member differs from selected cut",
            ));
        }
        packet_bytes = packet_bytes
            .checked_add(observed.raw.len())
            .filter(|total| *total <= 8_388_608)
            .ok_or(SourceCommandError::Invalid(
                "private Claim native identity packet byte budget",
            ))?;
        selected.files.push(SourceFile {
            path: member.path.clone(),
            raw: observed.raw,
        });
    }
    if selected.files.len() > SELECTED_SOURCE_MAX_FILES
        || selected
            .files
            .iter()
            .try_fold(0usize, |total, file| total.checked_add(file.raw.len()))
            .is_none_or(|total| total > SELECTED_SOURCE_MAX_BYTES)
    {
        return Err(SourceCommandError::Invalid(
            "private Claim native identity helper context budget",
        ));
    }
    Ok(selected)
}

fn identity_inventory(
    grant: &Grant,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    basenames: &BTreeSet<String>,
    private_inputs: &PrivateIdentityInputs,
    creating: bool,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, BTreeMap<String, String>, BTreeSet<String>)> {
    // The Claim-only owner-store census combines public and selected-private
    // entries and charges each visited directory entry before maintained
    // hidden/payload/catalog skips. Do not count cut file members again.
    if private_inputs.visited_entries > MAX_INVENTORY_ENTRIES {
        return Err(SourceCommandError::Invalid(
            "private Claim identity entry budget",
        ));
    }
    let mut selected = BTreeMap::<String, Vec<u8>>::new();
    for metadata in cut.current().members() {
        let reference = metadata.path.as_str();
        if !reference.starts_with("ToS/source-witnesses/")
            || reference.starts_with("ToS/source-witnesses/owner-local/")
        {
            continue;
        }
        let name = reference.rsplit('/').next().unwrap_or("");
        if reference.split('/').any(|part| {
            part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
        }) {
            continue;
        }
        if !(basenames.contains(name)
            || name.ends_with(".human-forms.json")
            || name.starts_with("semantic-annotation") && name.ends_with(".json")
            || creating && name.contains("provenance") && name.ends_with(".jsonl"))
        {
            continue;
        }
        if selected.len() >= MAX_INVENTORY_FILES {
            return Err(SourceCommandError::Invalid(
                "private Claim identity file budget",
            ));
        }
        let raw = selected_owner_read(
            owner,
            cut,
            context,
            reference,
            MAX_INVENTORY_FILE_BYTES,
            reads,
            deadline,
            cancelled,
        )?;
        selected.insert(reference.to_owned(), raw);
    }
    for (reference, raw) in &private_inputs.files {
        if selected.len() >= MAX_INVENTORY_FILES && !selected.contains_key(reference) {
            return Err(SourceCommandError::Invalid(
                "private Claim identity file budget",
            ));
        }
        let current = owner.read(reference, MAX_INVENTORY_FILE_BYTES, deadline, cancelled)?;
        if current != *raw {
            return Err(SourceCommandError::Conflict(
                "private Claim identity input changed",
            ));
        }
        reads.retain(reference, current.clone())?;
        if let Some(previous) = selected.insert(reference.clone(), current.clone())
            && previous != current
        {
            return Err(SourceCommandError::Conflict(
                "private/public identity path collision",
            ));
        }
    }
    let mut total = 0usize;
    let mut fingerprints = Vec::new();
    let mut ids = BTreeSet::new();
    for (reference, raw) in &selected {
        if raw.len() > MAX_INVENTORY_FILE_BYTES {
            return Err(SourceCommandError::Invalid(
                "private Claim identity member budget",
            ));
        }
        total = total
            .checked_add(raw.len())
            .filter(|n| *n <= MAX_INVENTORY_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "private Claim identity byte budget",
            ))?;
        fingerprints.push((
            tos_foundation::JsonString::from_utf8(reference),
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ));
        let jsonl = reference.ends_with(".jsonl");
        for row in inventory_value(raw, jsonl)? {
            inventory_ids(&row, &mut ids)?;
        }
    }
    let mut reserved = BTreeSet::new();
    reserved.extend(
        exact_list(&grant.value, "allowed_claim_ids", 32)?
            .into_iter()
            .map(str::to_owned),
    );
    reserved.extend(
        exact_list(&grant.value, "allowed_form_ids", 32)?
            .into_iter()
            .map(str::to_owned),
    );
    if creating {
        reserved.insert(cmd::text(&grant.value, "provenance_event_id")?.to_owned());
    }
    if ids.iter().any(|identity| reserved.contains(identity)) {
        return Err(SourceCommandError::Conflict(
            "delegated Claim, form or provenance identity already has an owner",
        ));
    }
    let inventory = JsonValue::Object(fingerprints);
    let digest = cmd::record_digest(&inventory)?.to_prefixed();
    Ok((digest, fingerprints_to_map(&inventory)?, ids))
}

fn fingerprints_to_map(value: &JsonValue) -> SourceCommandResult<BTreeMap<String, String>> {
    let fields = value
        .as_object()
        .ok_or(SourceCommandError::Invalid("private Claim inventory map"))?;
    fields
        .iter()
        .map(|(name, raw)| {
            Ok((
                name.as_str()
                    .ok_or(SourceCommandError::Invalid("private Claim inventory path"))?
                    .to_owned(),
                raw.as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim inventory digest",
                    ))?
                    .to_owned(),
            ))
        })
        .collect()
}

struct OwnerNativeRead<'a> {
    owner: &'a OwnerTextContext,
    cut: &'a CorpusCutReader,
    context: &'a JsonValue,
    reads: &'a mut ExactReads,
}

impl crate::source_sign_native::SignNativeRead for OwnerNativeRead<'_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: crate::source_sign_native::NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        selected_owner_read(
            self.owner,
            self.cut,
            self.context,
            reference,
            max_bytes,
            self.reads,
            deadline,
            cancelled,
        )
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.owner.snapshot(deadline, cancelled).map(|_| ())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        private_reference(reference, self.context)
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        Ok(Some(
            self.owner.snapshot(deadline, cancelled)?.to_prefixed(),
        ))
    }
}

pub(crate) fn owner_metadata_snapshot(
    owner: &OwnerTextContext,
    context: &JsonValue,
    inputs: &[crate::source_sign_native::NativeInput],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let owner_context = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let mut rows = Vec::with_capacity(inputs.len());
    for input in inputs {
        let role = if private_reference(&input.reference, context)? {
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
        ("owner_context", cmd::string(&owner_context)),
        ("inputs", JsonValue::Array(rows)),
    ]);
    let raw = emit_python_compact_json(
        &value,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("private Claim owner native snapshot encoding"))?;
    let snapshot = crate::source_sign_native::ascii_snapshot_bytes(&raw)?;
    if owner.snapshot(deadline, cancelled)?.to_prefixed() != owner_context {
        return Err(SourceCommandError::Conflict(
            "private Claim owner context changed during native profile snapshot",
        ));
    }
    Ok(snapshot)
}

fn public_native_snapshot(
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    bindings: &[JsonValue],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    if bindings.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private Claim public native snapshot needs selected bindings",
        ));
    }
    let mut reader = OwnerNativeRead {
        owner,
        cut,
        context,
        reads,
    };
    let resolved = crate::source_sign_native::resolve_bindings(
        &mut reader,
        worker,
        bindings,
        crate::source_sign_native::NativeReadScope::MetadataOnly,
        deadline,
        cancelled,
    )?;
    for summary in &resolved.summaries {
        if cmd::field(summary, "public_content_declared")? != &JsonValue::Bool(true) {
            return Err(SourceCommandError::Denied(
                "public Claim source binding does not declare public content",
            ));
        }
    }
    let rows = resolved
        .inputs
        .iter()
        .map(|input| {
            JsonValue::Array(vec![
                cmd::string(&input.reference),
                cmd::string(input.category),
                cmd::string(&input.raw_sha256.to_hex()),
            ])
        })
        .collect();
    let raw = emit_python_compact_json(
        &JsonValue::Array(rows),
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("private Claim public native snapshot encoding"))?;
    crate::source_sign_native::ascii_snapshot_bytes(&raw)
}

#[derive(Clone)]
struct GroundRecord {
    value: JsonValue,
    path: String,
    origin: String,
    profile_type_id: String,
    reference: JsonValue,
    private: bool,
    source_access: JsonValue,
    source_binding: Option<JsonValue>,
    profile_dependencies: BTreeMap<String, String>,
    private_native_metadata: Option<String>,
    private_native_exact: Option<String>,
}

fn private_profile_snapshot(
    record: &GroundRecord,
    context_snapshot: &str,
    reads: &ExactReads,
) -> SourceCommandResult<String> {
    let raw = reads
        .files
        .get(&record.path)
        .ok_or(SourceCommandError::Conflict(
            "private Claim profile source is absent from its exact read closure",
        ))?;
    let mut sources = BTreeMap::new();
    sources.insert(record.path.clone(), digest(raw));
    let binding = match &record.source_binding {
        Some(value) => cmd::string(&Digest256::of_bytes(&cmd::canonical(value)?).to_hex()),
        None => JsonValue::Null,
    };
    let native = cmd::object(vec![
        (
            "metadata",
            record
                .private_native_metadata
                .as_ref()
                .map(|value| cmd::string(value))
                .unwrap_or(JsonValue::Null),
        ),
        (
            "exact",
            record
                .private_native_exact
                .as_ref()
                .map(|value| cmd::string(value))
                .unwrap_or(JsonValue::Null),
        ),
    ]);
    let snapshot = cmd::object(vec![
        ("context", cmd::string(context_snapshot)),
        ("source_access", record.source_access.clone()),
        ("source_binding", binding),
        ("contracts", hash_map(record.profile_dependencies.clone())),
        ("sources", hash_map(sources)),
        ("native", native),
    ]);
    cmd::record_digest(&snapshot).map(|value| value.to_prefixed())
}

fn exact_profile_route(
    profile: &JsonValue,
    record: &JsonValue,
) -> SourceCommandResult<(String, Vec<String>)> {
    let version = cmd::text(record, "schema_version")?;
    let mut selected = None;
    for route in cmd::array(profile, "schemas")? {
        if cmd::text(route, "schema_version")? == version && selected.replace(route).is_some() {
            return Err(SourceCommandError::Unsupported(
                "private Claim endpoint schema route is duplicated",
            ));
        }
    }
    let Some(route) = selected else {
        return Err(SourceCommandError::Unsupported(
            "private Claim endpoint schema route unavailable",
        ));
    };
    let root = cmd::text(route, "schema_ref")?.to_owned();
    let mut refs = vec![CORPUS_SCHEMA.to_owned(), SOURCE_METADATA_SCHEMA.to_owned()];
    refs.extend(field_texts(route, "schema_dependencies", 128)?);
    refs.push(root.clone());
    let mut seen = BTreeSet::new();
    refs.retain(|reference| seen.insert(reference.clone()));
    Ok((root, refs))
}

fn preflight_claim_source_selectors(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    grammar: &ClaimGrammar,
    grant: &Grant,
    worker: &CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut required_schemas = BTreeSet::new();
    for selection in grant.selections.values() {
        for selector in &selection.source_records {
            let path = cmd::text(selector, "path")?;
            RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("private Claim endpoint path"))?;
            let private = private_reference(path, context)?;
            if !path.starts_with("ToS/source-witnesses/")
                || path.split('/').any(|part| {
                    part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
                })
                || private && !path.starts_with(cmd::text(context, "private_prefix")?)
                || !private && path.starts_with("ToS/source-witnesses/owner-local/")
            {
                return Err(SourceCommandError::Denied(
                    "private Claim endpoint leaves selected metadata source scope",
                ));
            }
            let profile_type_id = cmd::text(selector, "profile_type_id")?;
            if let Some(profile) = grammar.profile_by_type.get(profile_type_id) {
                let kind = cmd::text(profile, "record_type")?;
                let reader = cmd::text(profile, "reader")?;
                if !matches!(reader, "semantic-metadata-v1" | "corpus-metadata-v1")
                    || private && reader != "semantic-metadata-v1"
                    || path.rsplit('/').next() != Some(cmd::text(profile, "source_basename")?)
                    || grammar.type_by_kind.get(kind).map(String::as_str) != Some(profile_type_id)
                {
                    return Err(SourceCommandError::Denied(
                        "private Claim endpoint does not match its exact selected source profile",
                    ));
                }
                for route in cmd::array(profile, "schemas")? {
                    required_schemas.insert(CORPUS_SCHEMA.to_owned());
                    required_schemas.insert(SOURCE_METADATA_SCHEMA.to_owned());
                    required_schemas.insert(cmd::text(route, "schema_ref")?.to_owned());
                    required_schemas.extend(field_texts(route, "schema_dependencies", 128)?);
                }
                if !cmd::field(selector, "source_binding")?.is_null() {
                    if cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1"
                        || !selection.verify_content
                        || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                            != "exact_owner_local"
                    {
                        return Err(SourceCommandError::Denied(
                            "Claim endpoint native binding needs its exact declared adapter and access",
                        ));
                    }
                    required_schemas
                        .insert("ToS/contracts/native-text-unit-binding.schema.json".to_owned());
                    required_schemas.insert(
                        "ToS/contracts/native-text-unit-assessment-subject.schema.json".to_owned(),
                    );
                }
            } else {
                let kind = profile_type_id
                    .strip_prefix("tos.entity.")
                    .ok_or(SourceCommandError::Denied("Claim native endpoint type"))?;
                let entity =
                    grammar
                        .entity_by_id
                        .get(profile_type_id)
                        .ok_or(SourceCommandError::Denied(
                            "Claim native endpoint type is undeclared",
                        ))?;
                let ancestry = crate::source_claims::ancestry(
                    cmd::array(&grammar.entities, "types")?,
                    profile_type_id,
                )?;
                if !NATIVE_CORPUS_KINDS.contains(&kind)
                    || grammar.type_by_kind.get(kind).map(String::as_str) != Some(profile_type_id)
                    || cmd::field(entity, "abstract")? != &JsonValue::Bool(false)
                    || cmd::text(entity, "object_role")? != "identity"
                    || !ancestry.contains("tos.entity.identity")
                    || private
                    || path.rsplit('/').next() != Some(format!("{kind}.json").as_str())
                    || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                        != "metadata_only"
                    || !cmd::field(selector, "source_binding")?.is_null()
                {
                    return Err(SourceCommandError::Denied(
                        "Claim native Corpus endpoint needs its exact public identity mapping",
                    ));
                }
                required_schemas.insert(CORPUS_SCHEMA.to_owned());
            }
        }
        if !selection.native_bindings.is_empty() {
            if !selection.verify_content
                || cmd::text(&selection.source_access, "read_scope")? != "exact_owner_local"
            {
                return Err(SourceCommandError::Denied(
                    "Claim native evidence needs exact selected content verification",
                ));
            }
            required_schemas
                .insert("ToS/contracts/native-text-unit-binding.schema.json".to_owned());
            required_schemas
                .insert("ToS/contracts/native-text-unit-assessment-subject.schema.json".to_owned());
        }
    }
    for reference in required_schemas {
        let raw = selected_authored(
            ctx, owner, cut, context, &reference, 1_048_576, reads, deadline, cancelled,
        )?;
        let schema = cmd::parse(&raw)?;
        if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{reference}")
            && cmd::text(&schema, "$id")? != format!("https://treeofsophia.local/{reference}")
        {
            return Err(SourceCommandError::Conflict(
                "Claim endpoint schema identity differs from its owner path",
            ));
        }
        require_contract_digest(worker, &reference, &raw)?;
    }
    Ok(())
}

fn preflight_assessment_claim_sources(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    grammar: &ClaimGrammar,
    selections: &[AssessmentClaimSelection],
    worker: &CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut required_schemas = BTreeSet::new();
    for selection in selections {
        for selector in &selection.source_records {
            let path = cmd::text(selector, "path")?;
            let private = private_reference(path, context)?;
            let profile_type_id = cmd::text(selector, "profile_type_id")?;
            if let Some(profile) = grammar.profile_by_type.get(profile_type_id) {
                let kind = cmd::text(profile, "record_type")?;
                let reader = cmd::text(profile, "reader")?;
                if !matches!(reader, "semantic-metadata-v1" | "corpus-metadata-v1")
                    || private && reader != "semantic-metadata-v1"
                    || path.rsplit('/').next() != Some(cmd::text(profile, "source_basename")?)
                    || grammar.type_by_kind.get(kind).map(String::as_str) != Some(profile_type_id)
                {
                    return Err(SourceCommandError::Denied(
                        "private Claim endpoint does not match its exact selected source profile",
                    ));
                }
                for route in cmd::array(profile, "schemas")? {
                    required_schemas.insert(CORPUS_SCHEMA.to_owned());
                    required_schemas.insert(SOURCE_METADATA_SCHEMA.to_owned());
                    required_schemas.insert(cmd::text(route, "schema_ref")?.to_owned());
                    required_schemas.extend(field_texts(route, "schema_dependencies", 128)?);
                }
                if !cmd::field(selector, "source_binding")?.is_null() {
                    if cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1" {
                        return Err(SourceCommandError::Denied(
                            "Claim endpoint binding lacks its exact source-profile adapter",
                        ));
                    }
                    required_schemas
                        .insert("ToS/contracts/native-text-unit-binding.schema.json".to_owned());
                    required_schemas.insert(
                        "ToS/contracts/native-text-unit-assessment-subject.schema.json".to_owned(),
                    );
                }
            } else {
                let kind = profile_type_id
                    .strip_prefix("tos.entity.")
                    .ok_or(SourceCommandError::Denied("Claim native endpoint type"))?;
                let entity =
                    grammar
                        .entity_by_id
                        .get(profile_type_id)
                        .ok_or(SourceCommandError::Denied(
                            "Claim native endpoint type is undeclared",
                        ))?;
                let ancestry = crate::source_claims::ancestry(
                    cmd::array(&grammar.entities, "types")?,
                    profile_type_id,
                )?;
                if !NATIVE_CORPUS_KINDS.contains(&kind)
                    || grammar.type_by_kind.get(kind).map(String::as_str) != Some(profile_type_id)
                    || cmd::field(entity, "abstract")? != &JsonValue::Bool(false)
                    || cmd::text(entity, "object_role")? != "identity"
                    || !ancestry.contains("tos.entity.identity")
                    || private
                    || path.rsplit('/').next() != Some(format!("{kind}.json").as_str())
                    || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                        != "metadata_only"
                    || !cmd::field(selector, "source_binding")?.is_null()
                {
                    return Err(SourceCommandError::Denied(
                        "Claim native Corpus endpoint needs its exact public identity mapping",
                    ));
                }
                required_schemas.insert(CORPUS_SCHEMA.to_owned());
            }
        }
        if !selection.native_bindings.is_empty() {
            required_schemas
                .insert("ToS/contracts/native-text-unit-binding.schema.json".to_owned());
            required_schemas
                .insert("ToS/contracts/native-text-unit-assessment-subject.schema.json".to_owned());
        }
    }
    for reference in required_schemas {
        let raw = selected_authored(
            ctx, owner, cut, context, &reference, 1_048_576, reads, deadline, cancelled,
        )?;
        let schema = cmd::parse(&raw)?;
        if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{reference}")
            && cmd::text(&schema, "$id")? != format!("https://treeofsophia.local/{reference}")
        {
            return Err(SourceCommandError::Conflict(
                "Claim endpoint schema identity differs from its owner path",
            ));
        }
        require_contract_digest(worker, &reference, &raw)?;
    }
    Ok(())
}

fn assessment_claim_route(
    grammar: &ClaimGrammar,
    selection: &AssessmentClaimSelection,
    claim: &JsonValue,
) -> SourceCommandResult<ClaimRoute> {
    let identity = cmd::text(claim, "claim_id")?;
    let predicate = cmd::text(claim, "predicate")?;
    let route = grammar.route(predicate, cmd::text(claim, "schema_version")?)?;
    let maker = cmd::field(claim, "maker")?;
    let maker_type = cmd::text(maker, "maker_type")?;
    if identity != selection.claim_id
        || route.relation_type_id != selection.relation_type_id
        || cmd::text(claim, "claim_type")? != "relation"
        || cmd::text(claim, "visibility")? != "local_only"
        || !event_id(cmd::text(claim, "provenance_event_ref")?)
        || !cmd::nonblank(cmd::text(maker, "agent_ref")?)
        || !matches!(maker_type, "human" | "software" | "model")
        || !matches!(
            route.reader.as_str(),
            "semantic-relation-v1"
                | "identity-relation-v1"
                | "structured-reference-value-v1"
                | "historical-temporal-v1"
        )
        || selection.source_access.as_object().is_none()
        || !route
            .assertion_layers
            .contains(&cmd::text(claim, "assertion_layer")?.to_owned())
        || COMPOUND_PREDICATES.contains(&predicate)
        || cmd::text(claim, "subject_ref")? == identity
        || cmd::field(claim, "object")?.as_str() == Some(identity)
    {
        return Err(SourceCommandError::Denied(
            "private assessment Claim is outside its selected reified relation profile",
        ));
    }
    Ok(route)
}

fn assessment_reader_read(
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    reader: &mut dyn SignNativeRead,
    reference: &str,
    max_bytes: usize,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = reader.read(
        reference,
        crate::source_sign_native::NativeReadKind::Metadata,
        max_bytes,
        deadline,
        cancelled,
    )?;
    if !private_reference(reference, context)? {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("private Claim selected source path"))?;
        let member = cut
            .current()
            .member(&path)
            .ok_or(SourceCommandError::Conflict(
                "private Claim public source is outside the current cut",
            ))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "private Claim public source differs from current cut",
            ));
        }
    }
    // Keep OwnerTextContext currentness coupled to every callback read too.
    let _ = owner.snapshot(deadline, cancelled)?;
    reads.retain(reference, raw.clone())?;
    Ok(raw)
}

fn validate_endpoint_record(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    owner: &OwnerTextContext,
    context: &JsonValue,
    grammar: &ClaimGrammar,
    selector: &JsonValue,
    verify_content: bool,
    worker: &mut CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut pinned_reader: Option<&mut dyn SignNativeRead>,
) -> SourceCommandResult<GroundRecord> {
    let path = cmd::text(selector, "path")?;
    let selected_id = cmd::text(selector, "record_id")?;
    let profile_type_id = cmd::text(selector, "profile_type_id")?;
    let private = private_reference(path, context)?;
    if !path.starts_with("ToS/source-witnesses/")
        || path.split('/').any(|part| {
            part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
        })
        || private && !path.starts_with(cmd::text(context, "private_prefix")?)
        || !private && path.starts_with("ToS/source-witnesses/owner-local/")
    {
        return Err(SourceCommandError::Denied(
            "private Claim endpoint leaves selected metadata source scope",
        ));
    }
    let raw = if let Some(reader) = pinned_reader.as_deref_mut() {
        let raw = reader.read(
            path,
            crate::source_sign_native::NativeReadKind::Metadata,
            1_048_576,
            deadline,
            cancelled,
        )?;
        if !private {
            let member_path = RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("private Claim selected source path"))?;
            let metadata =
                cut.current()
                    .member(&member_path)
                    .ok_or(SourceCommandError::Conflict(
                        "private Claim public source is outside the current cut",
                    ))?;
            if raw.len() as u64 != metadata.size_bytes
                || Digest256::of_bytes(&raw) != metadata.sha256
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim public source differs from current cut",
                ));
            }
        }
        reads.retain(path, raw.clone())?;
        raw
    } else {
        selected_owner_read(
            owner, cut, context, path, 1_048_576, reads, deadline, cancelled,
        )?
    };
    let record = cmd::parse(&raw)?;
    let mut profile_dependencies = BTreeMap::new();
    let mut private_native_metadata = None;
    let mut private_native_exact = None;
    if let Some(profile) = grammar.profile_by_type.get(profile_type_id) {
        let kind = cmd::text(profile, "record_type")?;
        if cmd::text(profile, "reader")? != "semantic-metadata-v1"
            && cmd::text(profile, "reader")? != "corpus-metadata-v1"
        {
            return Err(SourceCommandError::Unsupported(
                "Claim endpoint profile reader",
            ));
        }
        if private && cmd::text(profile, "reader")? != "semantic-metadata-v1" {
            return Err(SourceCommandError::Denied(
                "private Claim endpoints require semantic local metadata",
            ));
        }
        if path.rsplit('/').next() != Some(cmd::text(profile, "source_basename")?)
            || cmd::text(&record, "record_type")? != kind
            || cmd::text(&record, "record_id")? != selected_id
            || !crate::source_revisions::valid_id(
                selected_id,
                cmd::text(profile, "id_prefix")?,
                false,
            )
            || cmd::integer(&record, "record_version")? == 0
            || !matches!(
                cmd::text(&record, "identity_status")?,
                "provisional" | "verified" | "disputed" | "superseded"
            )
            || cmd::text(&record, "preferred_label")?.trim().is_empty()
            || if private {
                cmd::text(&record, "visibility")? != "local_only"
            } else {
                !matches!(
                    cmd::text(&record, "visibility")?,
                    "public" | "public_metadata_only"
                )
            }
        {
            return Err(SourceCommandError::Denied(
                "Claim endpoint profile identity, visibility or source path differs",
            ));
        }
        let (root, refs) = exact_profile_route(profile, &record)?;
        for reference in refs {
            let schema_raw = selected_authored(
                ctx, owner, cut, context, &reference, 1_048_576, reads, deadline, cancelled,
            )?;
            let schema = cmd::parse(&schema_raw)?;
            let id = cmd::text(&schema, "$id")?;
            if id != format!("https://tree-of-sophia.local/{reference}")
                && id != format!("https://treeofsophia.local/{reference}")
            {
                return Err(SourceCommandError::Conflict(
                    "Claim endpoint schema identity differs from its owner path",
                ));
            }
            require_contract_digest(worker, &reference, &schema_raw)?;
        }
        let instance = cmd::canonical(&record)?;
        let valid = worker
            .check(&path, &instance, &root, deadline, cancelled)
            .map_err(|reason| SourceCommandError::SchemaExecution {
                path: path.to_owned(),
                root: root.clone(),
                reason,
            })?;
        if !valid {
            return Err(SourceCommandError::Conflict(
                "Claim endpoint violates its exact source-record route",
            ));
        }
        let instance = cmd::canonical(&record)?;
        check_schema(
            worker,
            path,
            &instance,
            SOURCE_METADATA_SCHEMA,
            deadline,
            cancelled,
        )?;
        let selected_binding = cmd::field(selector, "source_binding")?;
        let record_binding = record
            .object_get("native_text_binding")
            .cloned()
            .unwrap_or(JsonValue::Null);
        if !same_json(&selected_binding, &record_binding)? {
            return Err(SourceCommandError::Conflict(
                "Claim source record differs from its exact selected native binding",
            ));
        }
        if private && cmd::text(profile, "reader")? == "semantic-metadata-v1" {
            let validated = super::profile::validate_identity_record(
                profile_type_id,
                path,
                selected_id,
                &raw,
                ctx,
                cut,
                worker,
                deadline,
                cancelled,
            )?;
            if validated.record != record {
                return Err(SourceCommandError::Conflict(
                    "Claim private semantic endpoint differs from selected profile",
                ));
            }
            for dependency in validated.dependencies {
                let reference = dependency.path.as_str().to_owned();
                let digest = dependency.raw_sha256.to_hex();
                if profile_dependencies
                    .insert(reference, digest.clone())
                    .is_some_and(|previous| previous != digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim profile dependency changed during validation",
                    ));
                }
            }
            if !selected_binding.is_null() {
                if pinned_reader.is_some() {
                    if verify_content
                        && cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                            != "exact_owner_local"
                    {
                        return Err(SourceCommandError::Denied(
                            "exact private Claim binding is outside its selected source access",
                        ));
                    }
                } else {
                    if !verify_content
                        || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                            != "exact_owner_local"
                    {
                        return Err(SourceCommandError::Denied(
                            "private Claim source binding requires exact delegated verification",
                        ));
                    }
                    let mut native_reader = OwnerNativeRead {
                        owner,
                        cut,
                        context,
                        reads,
                    };
                    let resolved = crate::source_sign_native::resolve_owner_metadata_binding(
                        &mut native_reader,
                        worker,
                        &selected_binding,
                        deadline,
                        cancelled,
                    )?;
                    for (reference, schema_digest) in &resolved.schema_digests {
                        let digest = schema_digest.to_hex();
                        if profile_dependencies
                            .insert(reference.clone(), digest.clone())
                            .is_some_and(|previous| previous != digest)
                        {
                            return Err(SourceCommandError::Conflict(
                                "private Claim native profile grammar changed during validation",
                            ));
                        }
                    }
                    let metadata_inputs = resolved
                        .inputs
                        .iter()
                        .filter(|input| input.category != "content")
                        .cloned()
                        .collect::<Vec<_>>();
                    private_native_metadata = Some(owner_metadata_snapshot(
                        owner,
                        context,
                        &metadata_inputs,
                        deadline,
                        cancelled,
                    )?);
                    private_native_exact = Some(owner_metadata_snapshot(
                        owner,
                        context,
                        &resolved.inputs,
                        deadline,
                        cancelled,
                    )?);
                }
            }
        }
    } else {
        let kind = profile_type_id
            .strip_prefix("tos.entity.")
            .ok_or(SourceCommandError::Denied("Claim native endpoint type"))?;
        let entity =
            grammar
                .entity_by_id
                .get(profile_type_id)
                .ok_or(SourceCommandError::Denied(
                    "Claim native endpoint type is undeclared",
                ))?;
        let ancestry = crate::source_claims::ancestry(
            cmd::array(&grammar.entities, "types")?,
            profile_type_id,
        )?;
        if !NATIVE_CORPUS_KINDS.contains(&kind)
            || grammar.type_by_kind.get(kind).map(String::as_str) != Some(profile_type_id)
            || cmd::field(entity, "abstract")? != &JsonValue::Bool(false)
            || cmd::text(entity, "object_role")? != "identity"
            || !ancestry.contains("tos.entity.identity")
            || private
            || path.rsplit('/').next() != Some(format!("{kind}.json").as_str())
            || !path.starts_with("ToS/source-witnesses/")
            || cmd::text(&record, "schema_version")? != "tos_corpus_record_v1"
            || cmd::text(&record, "record_type")? != kind
            || cmd::text(&record, "record_id")? != selected_id
            || !selected_id.starts_with(&format!("tos.{kind}."))
            || cmd::integer(&record, "record_version")? == 0
            || record.object_get("visibility").is_some()
            || record.object_get("native_text_binding").is_some()
        {
            return Err(SourceCommandError::Denied(
                "Claim native Corpus endpoint identity or metadata differs",
            ));
        }
        let schema_raw = selected_authored(
            ctx,
            owner,
            cut,
            context,
            CORPUS_SCHEMA,
            1_048_576,
            reads,
            deadline,
            cancelled,
        )?;
        require_contract_digest(worker, CORPUS_SCHEMA, &schema_raw)?;
        check_schema(worker, path, &raw, CORPUS_SCHEMA, deadline, cancelled)?;
    }
    let reference = record_ref(&record)?;
    Ok(GroundRecord {
        value: record,
        path: path.to_owned(),
        origin: cmd::text(selector, "origin_id")?.to_owned(),
        profile_type_id: profile_type_id.to_owned(),
        reference,
        private,
        source_access: cmd::field(selector, "source_access")?.clone(),
        source_binding: match cmd::field(selector, "source_binding")? {
            JsonValue::Null => None,
            binding => Some(binding.clone()),
        },
        profile_dependencies,
        private_native_metadata,
        private_native_exact,
    })
}

fn source_type_allowed(
    grammar: &ClaimGrammar,
    profile_type_id: &str,
    allowed: &[String],
) -> SourceCommandResult<bool> {
    let types = cmd::array(&grammar.entities, "types")?;
    let ancestry = crate::source_claims::ancestry(types, profile_type_id)?;
    Ok(allowed.iter().any(|candidate| ancestry.contains(candidate)))
}

fn owner_path_claim_stream(path: &str, context: &JsonValue) -> SourceCommandResult<()> {
    let prefix = cmd::text(context, "private_prefix")?;
    let tail = path.strip_prefix(prefix).ok_or(SourceCommandError::Denied(
        "private Claim source path store prefix",
    ))?;
    let parts = tail.split('/').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[0] != "claims"
        || parts[1].is_empty()
        || !dotted_identity(&format!("tos.slug.{}", parts[1]), "tos.slug.")
        || parts[2] != CLAIM_STREAM
    {
        return Err(SourceCommandError::Denied(
            "private Claim target must be one named owner-local claims package",
        ));
    }
    Ok(())
}

fn allowed_text(config: &JsonValue, key: &str, value: &str, max: usize) -> SourceCommandResult<()> {
    if !exact_list(config, key, max)?.contains(&value) {
        return Err(SourceCommandError::Denied(
            "private Claim value is outside its exact delegation",
        ));
    }
    Ok(())
}

fn claim_profile_route<'a>(
    grammar: &'a ClaimGrammar,
    config: &JsonValue,
    selection: &Selection,
    claim: &JsonValue,
) -> SourceCommandResult<ClaimRoute> {
    let predicate = cmd::text(claim, "predicate")?;
    allowed_text(config, "allowed_predicates", predicate, 32)?;
    let route = grammar.route(predicate, cmd::text(claim, "schema_version")?)?;
    if route.relation_type_id != selection.relation_type_id
        || !selection.claim_id.eq(cmd::text(claim, "claim_id")?)
        || !matches!(
            route.reader.as_str(),
            "semantic-relation-v1" | "identity-relation-v1" | "structured-reference-value-v1"
        )
        || selection.source_access.as_object().is_none()
        || !route
            .assertion_layers
            .contains(&cmd::text(claim, "assertion_layer")?.to_owned())
        || COMPOUND_PREDICATES.contains(&predicate)
    {
        return Err(SourceCommandError::Denied(
            "private Claim is outside its selected reified relation profile",
        ));
    }
    if !ConfigVersion::from_schema(cmd::text(config, "schema_version")?)?
        .reader_allowed(&route.reader)
    {
        return Err(SourceCommandError::Denied(
            "private Claim reader mode is outside this delegation version",
        ));
    }
    Ok(route)
}

impl ConfigVersion {
    fn from_schema(value: &str) -> SourceCommandResult<Self> {
        match value {
            CONFIG_V1 => Ok(Self::V1),
            CONFIG_V2 => Ok(Self::V2),
            _ => Err(SourceCommandError::Unsupported(
                "private Claim grant version",
            )),
        }
    }
}

fn check_claim_schema(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    grammar: &ClaimGrammar,
    claim: &JsonValue,
    route: &ClaimRoute,
    worker: &mut CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut refs = vec![
        CLAIM_SHARED_SCHEMAS[0].to_owned(),
        CLAIM_SHARED_SCHEMAS[1].to_owned(),
    ];
    refs.extend(route.schema_dependencies.iter().cloned());
    refs.push(route.schema_ref.clone());
    refs.extend(grammar.schema_digests.keys().cloned().filter(|reference| {
        reference == CLAIM_REGISTRY_SCHEMA || reference == ENTITY_REGISTRY_SCHEMA
    }));
    let mut seen = BTreeSet::new();
    refs.retain(|reference| seen.insert(reference.clone()));
    for reference in &refs {
        let raw = selected_authored(
            ctx, owner, cut, context, reference, 1_048_576, reads, deadline, cancelled,
        )?;
        let schema = cmd::parse(&raw)?;
        let id = cmd::text(&schema, "$id")?;
        if id != format!("https://tree-of-sophia.local/{reference}")
            && id != format!("https://treeofsophia.local/{reference}")
        {
            return Err(SourceCommandError::Conflict(
                "private Claim schema identity differs from its owner path",
            ));
        }
        require_contract_digest(worker, reference, &raw)?;
    }
    let raw = cmd::canonical(claim)?;
    check_schema(
        worker,
        &route.schema_ref,
        &raw,
        &route.schema_ref,
        deadline,
        cancelled,
    )?;
    check_schema(
        worker,
        &route.schema_ref,
        &raw,
        CLAIM_BASE_SCHEMA,
        deadline,
        cancelled,
    )?;
    Ok(())
}

fn validate_claim_scope(
    grant: &Grant,
    route: &ClaimRoute,
    claim: &JsonValue,
    all_claim_ids: &BTreeSet<String>,
    initial: bool,
) -> SourceCommandResult<()> {
    let identity = cmd::text(claim, "claim_id")?;
    let maker = cmd::field(claim, "maker")?;
    let maker_id = cmd::text(maker, "agent_ref")?;
    let maker_type = cmd::text(maker, "maker_type")?;
    let provenance = cmd::text(claim, "provenance_event_ref")?;
    if !claim_id(identity)
        || !grant.selections.contains_key(identity)
        || cmd::text(claim, "claim_type")? != "relation"
        || cmd::text(claim, "visibility")? != "local_only"
        || !cmd::nonblank(maker_id)
        || !matches!(maker_type, "human" | "software" | "model")
        || !event_id(provenance)
        || initial
            && (maker_id != cmd::text(&grant.value, "principal_id")?
                || maker_type != cmd::text(&grant.value, "maker_type")?
                || provenance != cmd::text(&grant.value, "provenance_event_id")?)
    {
        return Err(SourceCommandError::Denied(
            "private Claim identity, maker or visibility differs from its delegation",
        ));
    }
    allowed_text(&grant.value, "allowed_claim_ids", identity, 32)?;
    allowed_text(
        &grant.value,
        "allowed_subject_refs",
        cmd::text(claim, "subject_ref")?,
        128,
    )?;
    allowed_text(&grant.value, "allowed_predicates", &route.predicate, 32)?;
    if !claim_id(identity) || cmd::text(claim, "subject_ref")? == identity {
        return Err(SourceCommandError::Invalid(
            "private Claim identity endpoint",
        ));
    }
    let object = cmd::field(claim, "object")?;
    if let Some(reference) = object.as_str() {
        allowed_text(&grant.value, "allowed_object_refs", reference, 128)?;
    } else {
        let values = cmd::array(&grant.value, "allowed_object_values")?;
        if !contains_same_json(values, object)? {
            return Err(SourceCommandError::Denied(
                "private Claim structured object is outside its exact v2 value grant",
            ));
        }
        if route.reader != "structured-reference-value-v1"
            || !ConfigVersion::V2.allows_object_revision()
            || cmd::field(object, "kind")? != cmd::field(&route.profile, "value_kind")?
        {
            return Err(SourceCommandError::Denied(
                "private Claim structured object needs its declared reference-value reader",
            ));
        }
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        for reference in optional_array(claim, key)? {
            allowed_text(
                &grant.value,
                "allowed_evidence_refs",
                reference.as_str().ok_or(SourceCommandError::Invalid(
                    "private Claim evidence reference",
                ))?,
                128,
            )?;
        }
    }
    for quote in optional_array(claim, "supporting_quotes")? {
        allowed_text(
            &grant.value,
            "allowed_evidence_refs",
            cmd::text(quote, "anchor_ref")?,
            128,
        )?;
    }
    for alternative in optional_array(claim, "alternative_claim_refs")? {
        let reference = alternative.as_str().ok_or(SourceCommandError::Invalid(
            "private Claim alternative identity",
        ))?;
        if !all_claim_ids.contains(reference) {
            return Err(SourceCommandError::Denied(
                "private Claim alternative must name an independently present Claim",
            ));
        }
    }
    if initial
        && (cmd::integer(claim, "claim_version")? != 1
            || !optional_array(claim, "assessment_refs")?.is_empty()
            || claim
                .object_get("supersedes_claim_ref")
                .is_some_and(|value| !value.is_null()))
    {
        return Err(SourceCommandError::Denied(
            "initial private Claims cannot revise or carry assessment admission",
        ));
    }
    Ok(())
}

fn validate_endpoint_closure(
    grant: &Grant,
    grammar: &ClaimGrammar,
    route: &ClaimRoute,
    claim: &JsonValue,
    records: &BTreeMap<String, GroundRecord>,
    _identity_ids: &BTreeSet<String>,
) -> SourceCommandResult<Vec<String>> {
    let mut endpoints = Vec::new();
    let subject = cmd::text(claim, "subject_ref")?;
    let subject_record = records.get(subject).ok_or(SourceCommandError::Unsupported(
        "private Claim exact subject metadata selection",
    ))?;
    if !source_type_allowed(grammar, &subject_record.profile_type_id, &route.domain)? {
        return Err(SourceCommandError::Invalid(
            "private Claim subject violates selected relation domain",
        ));
    }
    endpoints.push(subject.to_owned());
    if let Some(object) = cmd::field(claim, "object")?.as_str() {
        let object_record = records.get(object).ok_or(SourceCommandError::Unsupported(
            "private Claim exact object metadata selection",
        ))?;
        if !source_type_allowed(grammar, &object_record.profile_type_id, &route.range)? {
            return Err(SourceCommandError::Invalid(
                "private Claim object violates selected relation range",
            ));
        }
        endpoints.push(object.to_owned());
    } else if route.reader == "structured-reference-value-v1"
        && route.profile.object_get("object_reference_set").is_some()
    {
        let object = cmd::field(claim, "object")?;
        let members = cmd::array(object, "members")?;
        let constraint = cmd::field(&route.profile, "object_reference_set")?;
        let min_items = cmd::integer(constraint, "min_items")? as usize;
        let max_items = cmd::integer(constraint, "max_items")? as usize;
        if members.len() < min_items || members.len() > max_items {
            return Err(SourceCommandError::Invalid(
                "private Claim reference value member budget",
            ));
        }
        let allowed = field_texts(constraint, "member_type_ids", 32)?;
        let mut seen = BTreeSet::new();
        for member in members {
            let id = member.as_str().ok_or(SourceCommandError::Invalid(
                "private Claim reference member identity",
            ))?;
            allowed_text(&grant.value, "allowed_object_refs", id, 128)?;
            if !seen.insert(id) || id == cmd::text(claim, "claim_id")? {
                return Err(SourceCommandError::Invalid(
                    "private Claim reference member repeats",
                ));
            }
            let row = records.get(id).ok_or(SourceCommandError::Unsupported(
                "private Claim exact reference member selection",
            ))?;
            if !source_type_allowed(grammar, &row.profile_type_id, &allowed)? {
                return Err(SourceCommandError::Invalid(
                    "private Claim reference member violates selected type set",
                ));
            }
            endpoints.push(id.to_owned());
        }
        if cmd::field(constraint, "subject_is_member")? == &JsonValue::Bool(true)
            && !seen.contains(subject)
        {
            return Err(SourceCommandError::Invalid(
                "private Claim reference value omits its required subject member",
            ));
        }
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        for reference in optional_array(claim, key)? {
            let reference = reference.as_str().ok_or(SourceCommandError::Invalid(
                "private Claim evidence identity",
            ))?;
            if reference.starts_with("ToS/") {
                let path = RelativePath::parse(reference)
                    .map_err(|_| SourceCommandError::Invalid("private Claim evidence path"))?;
                if path.as_str().split('/').any(|part| {
                    part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
                }) {
                    return Err(SourceCommandError::Denied(
                        "private Claim evidence excludes hidden and content carriers",
                    ));
                }
            } else if !cmd::nonblank(reference) {
                return Err(SourceCommandError::Invalid(
                    "private Claim evidence identity",
                ));
            }
        }
    }
    Ok(endpoints)
}

fn claim_form_filename(identity: &str) -> String {
    format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(identity.as_bytes()).to_hex()
    )
}

fn same_json(left: &JsonValue, right: &JsonValue) -> SourceCommandResult<bool> {
    Ok(cmd::canonical(left)? == cmd::canonical(right)?)
}

fn contains_same_json(values: &[JsonValue], target: &JsonValue) -> SourceCommandResult<bool> {
    for value in values {
        if same_json(value, target)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn pointer_value(source: &JsonValue, pointer: &str) -> Option<JsonValue> {
    if pointer.is_empty() {
        return Some(source.clone());
    }
    let mut current = source;
    for token in pointer.strip_prefix('/')?.split('/') {
        let token = token.replace("~1", "/").replace("~0", "~");
        current = match current {
            JsonValue::Object(_) => current.object_get(&token)?,
            JsonValue::Array(values) => values.get(token.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current.clone())
}

fn private_claim_materializations(
    record: &JsonValue,
    payload: &JsonValue,
) -> SourceCommandResult<Vec<JsonValue>> {
    let subject = record_ref(record)?;
    let fields = source_forms::metadata_fields(record)
        .map_err(|_| SourceCommandError::Invalid("private Claim field catalog"))?;
    let mut output = Vec::new();
    for form in cmd::array(payload, "forms")? {
        let form_subject = cmd::field(form, "subject")?;
        let ref_value = source_forms::form_reference(form)
            .map_err(|_| SourceCommandError::Invalid("private Claim form reference"))?;
        if !same_json(form_subject, &subject)?
            || cmd::text(cmd::field(form, "content")?, "kind")? != "source-copy"
            || cmd::text(cmd::field(form, "content")?, "slot")? != "wording"
        {
            return Err(SourceCommandError::Conflict(
                "private Claim source-copy form does not bind its exact source",
            ));
        }
        let role = cmd::text(form, "role")?;
        let bindings = cmd::field(form, "bindings")?;
        let selected = cmd::field(bindings, "wording")?;
        if !same_json(cmd::field(selected, "record")?, &subject)? {
            return Err(SourceCommandError::Conflict(
                "private Claim source-copy form selected another record",
            ));
        }
        let pointer = cmd::text(selected, "pointer")?;
        let matching = fields
            .iter()
            .filter(|field| field.role == role && field.pointer == pointer)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "private Claim source-copy form field is not in the exact Claim catalog",
            ));
        }
        let field = matching[0];
        if !same_json(cmd::field(form, "language")?, &field.language)?
            || !same_json(cmd::field(form, "script")?, &field.script)?
            || form
                .object_get("language_context")
                .is_some_and(|value| !value.is_null())
        {
            return Err(SourceCommandError::Conflict(
                "private Claim source-copy language is not bound to its exact field",
            ));
        }
        for (_, bound) in bindings.as_object().ok_or(SourceCommandError::Invalid(
            "private Claim form binding map",
        ))? {
            if !same_json(cmd::field(bound, "record")?, &subject)?
                || pointer_value(record, cmd::text(bound, "pointer")?).is_none()
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim source-copy binding is stale or unresolved",
                ));
            }
        }
        let wording = pointer_value(record, pointer).ok_or(SourceCommandError::Invalid(
            "private Claim source-copy pointer",
        ))?;
        let wording_text = wording.as_str().ok_or(SourceCommandError::Invalid(
            "private Claim source-copy wording is not text",
        ))?;
        if !cmd::nonblank(wording_text) {
            return Err(SourceCommandError::Invalid(
                "private Claim source-copy wording is empty",
            ));
        }
        let mut context = Vec::new();
        for (index, context_pointer) in field.context.iter().enumerate() {
            let slot = format!("context-{index}");
            let bound = cmd::field(bindings, &slot)?;
            if cmd::text(bound, "pointer")? != *context_pointer
                || !same_json(cmd::field(bound, "record")?, &subject)?
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim form omitted its selected source context",
                ));
            }
            context.push(cmd::object(vec![
                ("slot", cmd::string(&slot)),
                ("binding", bound.clone()),
                (
                    "value",
                    pointer_value(record, context_pointer).ok_or(SourceCommandError::Invalid(
                        "private Claim form context pointer",
                    ))?,
                ),
            ]));
        }
        output.push(cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_human_form_materialization_v1"),
            ),
            ("form", ref_value),
            ("subject", subject.clone()),
            ("state", cmd::string("ready")),
            ("display_text", wording),
            ("context", JsonValue::Array(context)),
            ("issues", JsonValue::Array(Vec::new())),
            ("admission", JsonValue::Null),
            ("performs_semantic_assessment", JsonValue::Bool(false)),
            ("role", cmd::string(role)),
            ("language", field.language.clone()),
            ("script", field.script.clone()),
            ("derivation", cmd::string("source-copy")),
            ("dependencies", JsonValue::Array(vec![subject.clone()])),
            ("standalone_reading", JsonValue::Bool(false)),
        ]));
    }
    Ok(output)
}

fn validate_claim_form_change(
    grant: &Grant,
    change: &JsonValue,
    payload: Option<&JsonValue>,
) -> SourceCommandResult<()> {
    cmd::exact_keys(change, &["operation", "expected_form", "form"])?;
    let form = cmd::field(change, "form")?;
    let identity = cmd::text(form, "form_id")?;
    if !exact_list(&grant.value, "allowed_form_ids", 32)?.contains(&identity)
        || cmd::text(form, "creator_id")? != cmd::text(&grant.value, "principal_id")?
        || !matches!(cmd::text(form, "role")?, "statement")
        || cmd::text(cmd::field(form, "content")?, "kind")? != "source-copy"
        || cmd::text(cmd::field(form, "content")?, "slot")? != "wording"
        || cmd::text(
            cmd::field(cmd::field(form, "bindings")?, "wording")?,
            "pointer",
        )? != "/qualifiers/statement"
    {
        return Err(SourceCommandError::Denied(
            "private Claim form is outside its exact statement-copy grant",
        ));
    }
    let expected = cmd::field(change, "expected_form")?;
    if !same_json(expected, cmd::field(form, "revises")?)? {
        return Err(SourceCommandError::Conflict(
            "private Claim form request differs from its bound predecessor",
        ));
    }
    let operation = cmd::text(change, "operation")?;
    if (operation == "form.create") != expected.is_null()
        || !exact_list(&grant.value, "allowed_operations", 4)?.contains(&operation)
    {
        return Err(SourceCommandError::Denied(
            "private Claim form operation differs from its delegated lineage",
        ));
    }
    if let Some(payload) = payload {
        let mut current = None;
        for retained in cmd::array(payload, "forms")? {
            if cmd::text(retained, "form_id")? == identity && current.replace(retained).is_some() {
                return Err(SourceCommandError::Conflict(
                    "private Claim current form identity is repeated",
                ));
            }
        }
        match (operation, current) {
            ("form.create", Some(_)) | ("form.revise", None) => {
                return Err(SourceCommandError::Conflict(
                    "private Claim form operation does not match current identity",
                ));
            }
            ("form.revise", Some(old)) => {
                let previous = source_forms::form_reference(old)
                    .map_err(|_| SourceCommandError::Invalid("private Claim form predecessor"))?;
                if !same_json(expected, &previous)?
                    || cmd::integer(form, "form_version")?
                        != cmd::integer(old, "form_version")?.saturating_add(1)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim form revision does not extend current version",
                    ));
                }
            }
            _ => (),
        }
    }
    Ok(())
}

struct PreparedForms {
    payload: JsonValue,
    views: Vec<JsonValue>,
    references: Vec<JsonValue>,
}

fn prepare_claim_forms(
    ctx: &CommandContext,
    grant: &Grant,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    record: &JsonValue,
    payload: Option<&JsonValue>,
    selections: &[JsonValue],
    rebind: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedForms> {
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "each private Claim requires one to thirty-two source-copy form selections",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut changes = Vec::new();
    for row in selections {
        cmd::exact_keys(row, &["form_id", "field_id"])?;
        let id = cmd::text(row, "form_id")?;
        let field = cmd::text(row, "field_id")?;
        if !form_id(id)
            || !exact_list(&grant.value, "allowed_form_ids", 32)?.contains(&id)
            || !seen.insert(id.to_owned())
        {
            return Err(SourceCommandError::Denied(
                "private Claim form is repeated or outside delegated identity scope",
            ));
        }
        if field != "claim.statement" {
            return Err(SourceCommandError::Denied(
                "private Claim grant permits only the exact statement field",
            ));
        }
        let change = source_forms::prepare_form_change(
            record,
            payload,
            cmd::text(&grant.value, "principal_id")?,
            id,
            field,
        )
        .map_err(|_| SourceCommandError::Invalid("private Claim form change"))?;
        validate_claim_form_change(grant, &change, payload)?;
        changes.push(change);
    }
    if let Some(payload) = payload {
        if cmd::text(cmd::field(payload, "subject")?, "id")? != cmd::text(record, "claim_id")? {
            return Err(SourceCommandError::Conflict(
                "private Claim forms bind a different subject",
            ));
        }
        if rebind {
            let current = cmd::array(payload, "forms")?
                .iter()
                .map(|form| cmd::text(form, "form_id").map(str::to_owned))
                .collect::<SourceCommandResult<BTreeSet<_>>>()?;
            if !current.is_subset(&seen) {
                return Err(SourceCommandError::Denied(
                    "private Claim correction must explicitly rebind every current form",
                ));
            }
        }
    }
    let subject = record_ref(record)?;
    let payload = source_forms::apply_form_changes(payload, &subject, &changes)
        .map_err(|_| SourceCommandError::Conflict("private Claim form successor"))?;
    super::profile::validate_form_set(
        ctx,
        worker,
        grant.source_path.as_str(),
        &payload,
        deadline,
        cancelled,
    )?;
    let views = private_claim_materializations(record, &payload)?;
    let mut all_ready = true;
    let mut has_statement = false;
    for view in &views {
        all_ready &= cmd::text(view, "state")? == "ready";
        has_statement |= cmd::text(view, "role")? == "statement";
    }
    if !all_ready || !has_statement {
        return Err(SourceCommandError::Invalid(
            "private Claim forms require ready source copies including the full statement",
        ));
    }
    let references = changes
        .iter()
        .map(|change| {
            source_forms::form_reference(cmd::field(change, "form")?)
                .map_err(|_| SourceCommandError::Invalid("private Claim form reference"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(PreparedForms {
        payload,
        views,
        references,
    })
}

fn creation_files(
    ctx: &CommandContext,
    grant: &Grant,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    claims: &[JsonValue],
    selections: &[JsonValue],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(PrivatePackage, Vec<JsonValue>)> {
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private Claim batch requires one to thirty-two source-copy forms",
        ));
    }
    let mut grouped = claims
        .iter()
        .map(|claim| {
            Ok((
                cmd::text(claim, "claim_id")?.to_owned(),
                Vec::<JsonValue>::new(),
            ))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    let mut seen_forms = BTreeSet::new();
    for row in selections {
        cmd::exact_keys(row, &["claim_id", "form_id", "field_id"])?;
        let identity = cmd::text(row, "claim_id")?;
        let form = cmd::text(row, "form_id")?;
        if !grouped.contains_key(identity) || !seen_forms.insert(form.to_owned()) {
            return Err(SourceCommandError::Denied(
                "private Claim form is unselected or collides with a sibling",
            ));
        }
        grouped
            .get_mut(identity)
            .ok_or(SourceCommandError::Invalid(
                "private Claim form group absent",
            ))?
            .push(cmd::object(vec![
                ("form_id", cmd::string(form)),
                ("field_id", cmd::string(cmd::text(row, "field_id")?)),
            ]));
    }
    let mut files = PrivatePackage::new();
    files.insert(
        cmd::text(&grant.value, "source_path")?
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid("Claim stream basename"))?
            .to_owned(),
        claims
            .iter()
            .map(cmd::canonical)
            .collect::<SourceCommandResult<Vec<_>>>()?
            .into_iter()
            .flat_map(|mut row| {
                row.push(b'\n');
                row
            })
            .collect(),
    );
    files.insert(CONFIG_FILE.to_owned(), cmd::published(&grant.value)?);
    let mut views = Vec::new();
    for claim in claims {
        let identity = cmd::text(claim, "claim_id")?;
        let grouped_forms = grouped.get(identity).ok_or(SourceCommandError::Invalid(
            "private Claim form group absent",
        ))?;
        let prepared = prepare_claim_forms(
            ctx,
            grant,
            owner,
            cut,
            context,
            worker,
            claim,
            None,
            grouped_forms,
            false,
            deadline,
            cancelled,
        )?;
        files.insert(claim_form_filename(identity), cmd::published(&prepared.payload)?);
        views.extend(prepared.views);
    }
    if files.len() > MAX_PACKAGE_FILES
        || files.values().map(Vec::len).sum::<usize>() > MAX_PACKAGE_BYTES
    {
        return Err(SourceCommandError::Invalid(
            "private Claim package size budget",
        ));
    }
    Ok((files, views))
}

fn claim_identity_refs(claim: &JsonValue) -> SourceCommandResult<BTreeSet<String>> {
    let mut refs = BTreeSet::new();
    refs.insert(cmd::text(claim, "subject_ref")?.to_owned());
    if let Some(object) = cmd::field(claim, "object")?.as_str() {
        refs.insert(object.to_owned());
    } else {
        let object = cmd::field(claim, "object")?;
        if let Some(members) = object.object_get("members") {
            for member in members.as_array().ok_or(SourceCommandError::Invalid(
                "private Claim reference member list",
            ))? {
                refs.insert(
                    member
                        .as_str()
                        .ok_or(SourceCommandError::Invalid(
                            "private Claim reference member identity",
                        ))?
                        .to_owned(),
                );
            }
        }
        if let Some(anchor) = object
            .object_get("relative")
            .and_then(|relative| relative.object_get("anchor_ref"))
            .and_then(JsonValue::as_str)
        {
            refs.insert(anchor.to_owned());
        }
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        for item in optional_array(claim, key)? {
            refs.insert(
                item.as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim evidence identity",
                    ))?
                    .to_owned(),
            );
        }
    }
    Ok(refs)
}

fn binding_identities(binding: &JsonValue) -> SourceCommandResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for key in ["packet_ref", "packet_id", "unit_id", "layer_id"] {
        if let Some(value) = binding.object_get(key).and_then(JsonValue::as_str) {
            result.insert(value.to_owned());
        }
    }
    if let Some(text_layer) = binding.object_get("text_layer") {
        for key in ["record_ref", "layer_id"] {
            if let Some(value) = text_layer.object_get(key).and_then(JsonValue::as_str) {
                result.insert(value.to_owned());
            }
        }
    }
    if let Some(anchors) = binding.object_get("ordered_anchor_refs") {
        for value in anchors.as_array().ok_or(SourceCommandError::Invalid(
            "private Claim binding anchor list",
        ))? {
            result.insert(
                value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim binding anchor identity",
                    ))?
                    .to_owned(),
            );
        }
    }
    Ok(result)
}

struct GroundedClaims {
    dependencies: String,
    bindings: Vec<JsonValue>,
    source_fields: Vec<JsonValue>,
    materializations: Vec<JsonValue>,
    identity_digest: String,
    identity_files: BTreeMap<String, String>,
    identity_ids: BTreeSet<String>,
    snapshots: BTreeMap<String, String>,
    reads: Vec<SourceFile>,
}

fn form_fields_for_claim(record: &JsonValue) -> SourceCommandResult<Vec<JsonValue>> {
    let fields = source_forms::metadata_fields(record)
        .map_err(|_| SourceCommandError::Invalid("private Claim field catalog"))?;
    Ok(fields
        .iter()
        .map(|field| {
            let mut value = field.public();
            cmd::set(
                &mut value,
                "claim_id",
                cmd::string(cmd::text(record, "claim_id")?),
            )?;
            Ok(value)
        })
        .collect::<SourceCommandResult<Vec<_>>>()?)
}

fn implementation_digests(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
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
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_claim_commands.py",
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py",
        "scripts/source_owner_claim_profiles.py",
    ];
    if components.capture() != software.selection() {
        return Err(SourceCommandError::Conflict(
            "private Claim software implementation capture differs",
        ));
    }
    let mut remaining = 16_777_216usize;
    let mut values = BTreeMap::new();
    for reference in IMPLEMENTATIONS {
        crate::source_creation_store::active(deadline, cancelled)?;
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("private Claim implementation path"))?;
        let member = components
            .member(&path)
            .ok_or(SourceCommandError::Unsupported(
                "selected private Claim rule-source component absent",
            ))?;
        if member.size_bytes as usize > remaining {
            return Err(SourceCommandError::Invalid(
                "private Claim implementation byte budget",
            ));
        }
        let raw = software
            .read_selected_component(components, &path, 8_388_608, deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("private Claim rule-source capture changed")
            })?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "private Claim authored rule-source differs from selected software capture",
            ));
        }
        remaining = remaining.saturating_sub(raw.len());
        values.insert(
            (*reference).to_owned(),
            Digest256::of_bytes(&raw).to_prefixed(),
        );
    }
    Ok(hash_map(values))
}

fn hash_map(values: BTreeMap<String, String>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(name, hash)| {
                (
                    tos_foundation::JsonString::from_utf8(&name),
                    cmd::string(&hash),
                )
            })
            .collect(),
    )
}

// Configuration binds freshly constructed delegated readers, before any Claim
// or record is loaded. Execution schemas and form/public-identity fingerprints
// have their own bindings and must not enter this constructor grammar.
fn configuration_grammars(
    ctx: &CommandContext,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    grant: &Grant,
    reads: &mut ExactReads,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let mut grammars = Vec::new();
    for (identity, selection) in &grant.selections {
        let mut refs = BTreeSet::from([
            RELATIONS,
            ENTITIES,
            CLAIM_REGISTRY_SCHEMA,
            ENTITY_REGISTRY_SCHEMA,
            CONTEXT_SCHEMA_REF,
        ]);
        for selector in &selection.source_records {
            let type_id = cmd::text(selector, "profile_type_id")?;
            if type_id
                .strip_prefix("tos.entity.")
                .is_some_and(|kind| NATIVE_CORPUS_KINDS.contains(&kind))
            {
                refs.insert(CORPUS_SCHEMA);
            }
            if !cmd::field(selector, "source_binding")?.is_null() {
                refs.insert("ToS/contracts/native-text-unit-binding.schema.json");
            }
        }
        if !selection.native_bindings.is_empty() {
            refs.insert("ToS/contracts/native-text-unit-binding.schema.json");
        }
        let mut digests = BTreeMap::new();
        for reference in refs {
            let raw = selected_authored(
                ctx, owner, cut, context, reference, 1_048_576, reads, deadline, cancelled,
            )?;
            if reference != RELATIONS && reference != ENTITIES {
                require_contract_digest(worker, reference, &raw)?;
            }
            // Python reader input_digests use bare hex; the outer configuration
            // digest and public identity fingerprints remain prefixed digests.
            digests.insert(reference.to_owned(), Digest256::of_bytes(&raw).to_hex());
        }
        grammars.push((
            tos_foundation::JsonString::from_utf8(identity),
            hash_map(digests),
        ));
    }
    Ok(JsonValue::Object(grammars))
}

fn source_schema_fingerprints(
    reads: &ExactReads,
    route: &ClaimRoute,
) -> SourceCommandResult<JsonValue> {
    let mut refs = BTreeSet::new();
    refs.extend([
        RELATIONS,
        ENTITIES,
        CLAIM_REGISTRY_SCHEMA,
        ENTITY_REGISTRY_SCHEMA,
    ]);
    refs.extend(CLAIM_SHARED_SCHEMAS.iter().copied());
    refs.extend(route.schema_dependencies.iter().map(String::as_str));
    refs.insert(route.schema_ref.as_str());
    let mut digests = BTreeMap::new();
    for reference in refs {
        if let Some(raw) = reads.files.get(reference) {
            digests.insert(reference.to_owned(), Digest256::of_bytes(raw).to_prefixed());
        }
    }
    Ok(hash_map(digests))
}

fn resolve_native_selection(
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    reads: &mut ExactReads,
    binding: &JsonValue,
    origin: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<GroundedNativeBinding> {
    let mut native_reader = OwnerNativeRead {
        owner,
        cut,
        context,
        reads,
    };
    let mut resolved = crate::source_sign_native::resolve_owner_metadata_binding(
        &mut native_reader,
        worker,
        binding,
        deadline,
        cancelled,
    )?;

    // Claim profiles use NativeTextBindingResolver.assessment_records(), whose
    // frozen view includes this additional selected grammar. Keep that input
    // inside the same exact owner transport and reproduce the owner's opaque
    // Python snapshot shape before constructing the two assessment Records.
    const ASSESSMENT_SCHEMA: &str = "ToS/contracts/native-text-unit-assessment-subject.schema.json";
    let assessment_raw = native_reader.read(
        ASSESSMENT_SCHEMA,
        crate::source_sign_native::NativeReadKind::Schema,
        1_048_576,
        deadline,
        cancelled,
    )?;
    let assessment_schema = cmd::parse(&assessment_raw)?;
    if cmd::text(&assessment_schema, "$id")?
        != format!("https://tree-of-sophia.local/{ASSESSMENT_SCHEMA}")
        && cmd::text(&assessment_schema, "$id")?
            != format!("https://treeofsophia.local/{ASSESSMENT_SCHEMA}")
    {
        return Err(SourceCommandError::Conflict(
            "private Claim native assessment schema identity differs",
        ));
    }
    require_contract_digest(worker, ASSESSMENT_SCHEMA, &assessment_raw)?;
    resolved
        .inputs
        .push(crate::source_sign_native::NativeInput {
            reference: ASSESSMENT_SCHEMA.to_owned(),
            kind: crate::source_sign_native::NativeReadKind::Schema,
            category: "metadata",
            raw_sha256: Digest256::of_bytes(&assessment_raw),
            raw_size: assessment_raw.len(),
        });
    resolved.inputs.sort_by(|left, right| {
        left.reference
            .cmp(&right.reference)
            .then_with(|| left.category.cmp(right.category))
    });

    let context_before = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let mut remaining_metadata = 8_388_608usize;
    let mut remaining_content = 8_388_608usize;
    let mut inputs = Vec::with_capacity(resolved.inputs.len());
    for input in &resolved.inputs {
        let remaining = if input.category == "content" {
            &mut remaining_content
        } else {
            &mut remaining_metadata
        };
        let limit = if input.category == "content" {
            *remaining
        } else {
            (*remaining).min(1_048_576)
        };
        if input.raw_size > limit {
            return Err(SourceCommandError::Invalid(
                "private Claim native snapshot exceeds its input budget",
            ));
        }
        let raw = native_reader.read(&input.reference, input.kind, limit, deadline, cancelled)?;
        if raw.len() != input.raw_size || Digest256::of_bytes(&raw) != input.raw_sha256 {
            return Err(SourceCommandError::Conflict(
                "private Claim native input changed during its snapshot",
            ));
        }
        *remaining -= raw.len();
        let role = if private_reference(&input.reference, context)? {
            "owner-local-root"
        } else {
            "source-contract-root"
        };
        inputs.push(JsonValue::Array(vec![
            cmd::string(&input.reference),
            cmd::string(input.category),
            cmd::string(role),
            cmd::string(&input.raw_sha256.to_hex()),
        ]));
    }
    let owner_context = owner.snapshot(deadline, cancelled)?.to_prefixed();
    if owner_context != context_before {
        return Err(SourceCommandError::Conflict(
            "private Claim owner context changed during native snapshot",
        ));
    }
    let snapshot_value = cmd::object(vec![
        ("owner_context", cmd::string(&owner_context)),
        ("inputs", JsonValue::Array(inputs)),
    ]);
    let snapshot_raw = emit_python_compact_json(
        &snapshot_value,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("private Claim native snapshot encoding"))?;
    let input_snapshot = crate::source_sign_native::ascii_snapshot_bytes(&snapshot_raw)?;
    let layer_ref = cmd::object(vec![
        ("id", cmd::field(&resolved.layer, "layer_id")?.clone()),
        (
            "version",
            cmd::field(&resolved.layer, "layer_version")?.clone(),
        ),
        (
            "digest",
            cmd::string(&cmd::record_digest(&resolved.layer)?.to_prefixed()),
        ),
    ]);
    let payload = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_text_unit_assessment_subject_v1"),
        ),
        ("native_binding", binding.clone()),
        ("packet", resolved.packet.clone()),
        ("text_layer", layer_ref.clone()),
        (
            "content_verified",
            cmd::field(&resolved.summary, "content_verified")?.clone(),
        ),
        ("input_snapshot", cmd::string(&input_snapshot)),
    ]);
    let instance = cmd::canonical(&payload)?;
    let valid = worker
        .check_reusing_scalar(
            ASSESSMENT_SCHEMA,
            &instance,
            ASSESSMENT_SCHEMA,
            deadline,
            cancelled,
        )
        .map_err(|_| {
            SourceCommandError::Unsupported("private Claim native assessment schema worker")
        })?;
    if !valid {
        return Err(SourceCommandError::Conflict(
            "private Claim native assessment view violates its selected schema",
        ));
    }
    cmd::set(
        &mut resolved.summary,
        "owner_local_transport",
        JsonValue::Bool(inputs_have_private(&resolved.inputs, context)?),
    )?;
    let unit = cmd::object(vec![
        ("id", cmd::field(binding, "unit_id")?.clone()),
        ("version", cmd::field(binding, "unit_version")?.clone()),
        ("payload", payload),
        ("origin_id", cmd::string(origin)),
    ]);
    let layer = cmd::object(vec![
        ("id", cmd::field(&layer_ref, "id")?.clone()),
        ("version", cmd::field(&layer_ref, "version")?.clone()),
        ("payload", resolved.layer.clone()),
        ("origin_id", cmd::string(origin)),
    ]);
    let record_refs = [&unit, &layer]
        .iter()
        .map(|record| {
            Ok(cmd::object(vec![
                ("id", cmd::field(record, "id")?.clone()),
                ("version", cmd::field(record, "version")?.clone()),
                (
                    "digest",
                    cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(GroundedNativeBinding {
        summary: resolved.summary,
        records: vec![unit, layer],
        record_refs,
        input_snapshot,
    })
}

fn inputs_have_private(
    inputs: &[crate::source_sign_native::NativeInput],
    context: &JsonValue,
) -> SourceCommandResult<bool> {
    inputs.iter().try_fold(false, |found, input| {
        Ok(found || private_reference(&input.reference, context)?)
    })
}

struct GroundedNativeBinding {
    summary: JsonValue,
    records: Vec<JsonValue>,
    record_refs: Vec<JsonValue>,
    input_snapshot: String,
}

fn ground_claims(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context: &JsonValue,
    grant: &mut Grant,
    claims: &[JsonValue],
    private_inputs: &PrivateIdentityInputs,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    initial: bool,
) -> SourceCommandResult<GroundedClaims> {
    if claims.is_empty() || claims.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private source growth requires one to thirty-two Claims",
        ));
    }
    let mut reads = ExactReads::default();
    let grammar = ClaimGrammar::load(
        ctx, owner, cut, context, worker, &mut reads, deadline, cancelled,
    )?;
    validate_allowed_predicates(grant, &grammar)?;
    let form_grammar = super::profile::form_grammar_digests(ctx, worker, deadline, cancelled)?;
    let mut routes = BTreeMap::new();
    let claim_ids = claims
        .iter()
        .map(|claim| cmd::text(claim, "claim_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if claim_ids.len() != claims.len() {
        return Err(SourceCommandError::Conflict(
            "private Claim identity repeats",
        ));
    }
    for claim in claims {
        let identity = cmd::text(claim, "claim_id")?.to_owned();
        let selection = grant
            .selections
            .get(&identity)
            .ok_or(SourceCommandError::Denied("private Claim selection absent"))?;
        let route = claim_profile_route(&grammar, &grant.value, selection, claim)?;
        validate_claim_scope(grant, &route, claim, &claim_ids, initial)?;
        check_claim_schema(
            ctx, owner, cut, context, &grammar, claim, &route, worker, &mut reads, deadline,
            cancelled,
        )?;
        routes.insert(identity, route);
    }
    if grant
        .selections
        .keys()
        .any(|identity| !claim_ids.contains(identity))
    {
        return Err(SourceCommandError::Denied(
            "private Claim source selection is not one-to-one with its batch",
        ));
    }
    preflight_claim_source_selectors(
        ctx, owner, cut, context, &grammar, grant, worker, &mut reads, deadline, cancelled,
    )?;
    // Alternatives can refer to another Claim later in request order; validate
    // them only after the complete batch identity set has been assembled.
    for claim in claims {
        let route = routes
            .get(cmd::text(claim, "claim_id")?)
            .ok_or(SourceCommandError::Invalid("private Claim route absent"))?;
        validate_claim_scope(grant, route, claim, &claim_ids, initial)?;
    }
    let mut records_by_claim = BTreeMap::<String, BTreeMap<String, GroundRecord>>::new();
    let mut native_by_claim = BTreeMap::<String, Vec<NativeSelection>>::new();
    for claim in claims {
        let identity = cmd::text(claim, "claim_id")?;
        let selection = grant
            .selections
            .get(identity)
            .ok_or(SourceCommandError::Denied("private Claim selection absent"))?;
        let used_refs = claim_identity_refs(claim)?;
        let mut records = BTreeMap::new();
        let mut seen_paths = BTreeSet::new();
        for selector in &selection.source_records {
            let source_id = cmd::text(selector, "record_id")?;
            let path = cmd::text(selector, "path")?;
            if !used_refs.contains(source_id) && !used_refs.contains(path) {
                return Err(SourceCommandError::Denied(
                    "private Claim selected an unrelated source record",
                ));
            }
            if !seen_paths.insert(path.to_owned()) || records.contains_key(source_id) {
                return Err(SourceCommandError::Conflict(
                    "private Claim source record selection repeats",
                ));
            }
            let source = validate_endpoint_record(
                ctx,
                cut,
                owner,
                context,
                &grammar,
                selector,
                selection.verify_content,
                worker,
                &mut reads,
                deadline,
                cancelled,
                None,
            )?;
            let profile = grammar.profile_by_type.get(&source.profile_type_id);
            let bound = cmd::field(selector, "source_binding")?;
            if !bound.is_null() {
                let profile = profile.ok_or(SourceCommandError::Denied(
                    "native Claim binding cannot attach to an implicit Corpus profile",
                ))?;
                if cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1"
                    || !selection.verify_content
                    || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                        != "exact_owner_local"
                {
                    return Err(SourceCommandError::Denied(
                        "Claim endpoint native binding needs its exact declared adapter and access",
                    ));
                }
                let binding_key = String::from_utf8(cmd::canonical(bound)?)
                    .map_err(|_| SourceCommandError::Invalid("Claim native binding encoding"))?;
                native_by_claim
                    .entry(identity.to_owned())
                    .or_default()
                    .push(
                        (NativeSelection {
                            key: binding_key,
                            binding: bound.clone(),
                            origin_id: cmd::text(selector, "origin_id")?.to_owned(),
                            source_access: cmd::field(selector, "source_access")?.clone(),
                            source_ids: BTreeSet::from([source_id.to_owned()]),
                        }),
                    );
            }
            records.insert(source_id.to_owned(), source);
        }
        let direct = native_by_claim.entry(identity.to_owned()).or_default();
        for selector in &selection.native_bindings {
            if !selection.verify_content
                || cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                    != "exact_owner_local"
            {
                return Err(SourceCommandError::Denied(
                    "Claim native evidence needs exact selected content verification",
                ));
            }
            let binding = cmd::field(selector, "binding")?;
            let binding_key = String::from_utf8(cmd::canonical(binding)?)
                .map_err(|_| SourceCommandError::Invalid("Claim native binding encoding"))?;
            direct.push(NativeSelection {
                key: binding_key,
                binding: binding.clone(),
                origin_id: cmd::text(selector, "origin_id")?.to_owned(),
                source_access: cmd::field(selector, "source_access")?.clone(),
                source_ids: BTreeSet::new(),
            });
        }
        records_by_claim.insert(identity.to_owned(), records);
    }
    let (identity_digest, identity_files, mut identity_ids) = identity_inventory(
        grant,
        owner,
        cut,
        context,
        &identity_basenames(ctx, cut, worker, deadline, cancelled)?,
        private_inputs,
        initial,
        &mut reads,
        deadline,
        cancelled,
    )?;
    let mut native_identity_claims = BTreeSet::new();
    let mut native_identity_subjects = BTreeSet::new();
    for (claim_id, selected_records) in &records_by_claim {
        for (record_id, record) in selected_records {
            if !record.private && native_identity_subject(record_id) {
                native_identity_claims.insert(claim_id.clone());
                native_identity_subjects.insert(record_id.clone());
            }
        }
    }
    let public_native_identity = if native_identity_subjects.is_empty() {
        None
    } else {
        let (identities, snapshot, schema_used) =
            crate::source_revisions::native_identity_inventory_from_cut(
                ctx, cut, worker, deadline, cancelled,
            )?;
        if native_identity_subjects
            .iter()
            .any(|identity| identities.contains_key(identity))
        {
            return Err(SourceCommandError::Denied(
                "Claim source identity is already owned by a native semantic packet; explicit owner migration required",
            ));
        }
        for member in cut.current().members() {
            let reference = member.path.as_str();
            if public_native_identity_member(reference) {
                selected_owner_read(
                    owner, cut, context, reference, 1_048_576, &mut reads, deadline, cancelled,
                )?;
            }
        }
        if schema_used {
            let raw = selected_authored(
                ctx,
                owner,
                cut,
                context,
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                1_048_576,
                &mut reads,
                deadline,
                cancelled,
            )?;
            require_contract_digest(
                worker,
                "ToS/contracts/semantic-annotation-packet-v2.schema.json",
                &raw,
            )?;
        }
        Some(snapshot)
    };
    let form_map = form_grammar.as_object().ok_or(SourceCommandError::Invalid(
        "private Claim form grammar digest map",
    ))?;
    let mut form_digests = BTreeMap::new();
    for (path, raw) in form_map {
        form_digests.insert(
            path.as_str()
                .ok_or(SourceCommandError::Invalid("Claim form grammar path"))?
                .to_owned(),
            raw.as_str()
                .ok_or(SourceCommandError::Invalid("Claim form grammar digest"))?
                .to_owned(),
        );
    }
    let implementation = implementation_digests(software, components, deadline, cancelled)?;
    let mut provenance_contract = selected_authored(
        ctx,
        owner,
        cut,
        context,
        PROVENANCE_SCHEMA,
        1_048_576,
        &mut reads,
        deadline,
        cancelled,
    )?;
    let parsed_provenance = cmd::parse(&provenance_contract)?;
    if cmd::text(&parsed_provenance, "$id")?
        != format!("https://tree-of-sophia.local/{PROVENANCE_SCHEMA}")
        && cmd::text(&parsed_provenance, "$id")?
            != format!("https://treeofsophia.local/{PROVENANCE_SCHEMA}")
    {
        return Err(SourceCommandError::Conflict(
            "private Claim provenance contract identity",
        ));
    }
    require_contract_digest(worker, PROVENANCE_SCHEMA, &provenance_contract)?;
    let provenance_digest = Digest256::of_bytes(&provenance_contract).to_prefixed();
    let context_schema = selected_authored(
        ctx,
        owner,
        cut,
        context,
        CONTEXT_SCHEMA_REF,
        1_048_576,
        &mut reads,
        deadline,
        cancelled,
    )?;
    require_contract_digest(worker, CONTEXT_SCHEMA_REF, &context_schema)?;
    let configuration_grammars = configuration_grammars(
        ctx, owner, cut, context, grant, &mut reads, worker, deadline, cancelled,
    )?;
    let configuration_basis = cmd::object(vec![
        (
            "configuration_bytes",
            cmd::string(&Digest256::of_bytes(&grant.raw).to_prefixed()),
        ),
        ("context", cmd::string(&grant.context_snapshot)),
        ("grammars", configuration_grammars),
    ]);
    grant.digest = cmd::record_digest(&configuration_basis)?.to_prefixed();
    let mut snapshots = BTreeMap::new();
    let mut bindings = Vec::new();
    let mut source_fields = Vec::new();
    let mut materializations = Vec::new();
    for claim in claims {
        let identity = cmd::text(claim, "claim_id")?;
        let route = routes
            .get(identity)
            .ok_or(SourceCommandError::Invalid("private Claim route absent"))?;
        let selection = grant
            .selections
            .get(identity)
            .ok_or(SourceCommandError::Denied("private Claim selection absent"))?;
        let records = records_by_claim
            .get(identity)
            .ok_or(SourceCommandError::Invalid(
                "private Claim source closure absent",
            ))?;
        let _endpoints =
            validate_endpoint_closure(grant, &grammar, route, claim, records, &identity_ids)?;
        let used_refs = claim_identity_refs(claim)?;
        let mut native_summaries = Vec::new();
        let mut native_snapshots = Vec::new();
        let mut native_records = Vec::new();
        let mut native_entries = Vec::<NativeSelection>::new();
        let mut native_entry_indexes = BTreeMap::<String, usize>::new();
        if let Some(items) = native_by_claim.get(identity) {
            for entry in items {
                if let Some(index) = native_entry_indexes.get(&entry.key).copied() {
                    let prior =
                        native_entries
                            .get_mut(index)
                            .ok_or(SourceCommandError::Invalid(
                                "private Claim native selection index",
                            ))?;
                    if prior.origin_id != entry.origin_id
                        || prior.source_access != entry.source_access
                    {
                        return Err(SourceCommandError::Conflict(
                            "one native Claim binding has conflicting origins or access scopes",
                        ));
                    }
                    prior.source_ids.extend(entry.source_ids.iter().cloned());
                } else {
                    native_entry_indexes.insert(entry.key.clone(), native_entries.len());
                    native_entries.push(entry.clone());
                }
            }
        }
        let evidence_refs = optional_array(claim, "evidence_refs")?
            .iter()
            .chain(optional_array(claim, "counterevidence_refs")?.iter())
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim evidence identity",
                    ))
            })
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let quote_anchors = optional_array(claim, "supporting_quotes")?
            .iter()
            .map(|quote| cmd::text(quote, "anchor_ref").map(str::to_owned))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let source_aliases = records
            .iter()
            .flat_map(|(record_id, record)| [record_id.clone(), record.path.clone()])
            .collect::<BTreeSet<_>>();
        let mut native_aliases = BTreeMap::<String, BTreeSet<String>>::new();
        let mut native_anchors = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in &native_entries {
            for alias in binding_identities(&entry.binding)? {
                native_aliases
                    .entry(alias)
                    .or_default()
                    .insert(entry.key.clone());
            }
            for anchor in cmd::array(&entry.binding, "ordered_anchor_refs")? {
                let anchor = anchor.as_str().ok_or(SourceCommandError::Invalid(
                    "private Claim binding anchor identity",
                ))?;
                native_anchors
                    .entry(anchor.to_owned())
                    .or_default()
                    .insert(entry.key.clone());
            }
        }
        for entry in &native_entries {
            let aliases = binding_identities(&entry.binding)?;
            let evidence_used = evidence_refs
                .iter()
                .any(|reference| aliases.contains(reference));
            let quote_used = entry
                .binding
                .object_get("ordered_anchor_refs")
                .and_then(JsonValue::as_array)
                .is_some_and(|anchors| {
                    anchors
                        .iter()
                        .filter_map(JsonValue::as_str)
                        .any(|anchor| quote_anchors.contains(anchor))
                });
            if entry.source_ids.is_empty() && !evidence_used && !quote_used {
                return Err(SourceCommandError::Denied(
                    "private Claim native selection is outside its exact evidence closure",
                ));
            }
            if entry
                .source_ids
                .iter()
                .any(|source_id| !records.contains_key(source_id))
            {
                return Err(SourceCommandError::Denied(
                    "private Claim native selection is missing its exact source-record carrier",
                ));
            }
            let resolved = resolve_native_selection(
                owner,
                cut,
                context,
                worker,
                &mut reads,
                &entry.binding,
                &entry.origin_id,
                deadline,
                cancelled,
            )?;
            let mut summary = resolved.summary.clone();
            cmd::set(&mut summary, "origin_id", cmd::string(&entry.origin_id))?;
            cmd::set(
                &mut summary,
                "record_refs",
                JsonValue::Array(resolved.record_refs),
            )?;
            cmd::set(&mut summary, "supporting_only", JsonValue::Bool(true))?;
            native_summaries.push(summary);
            native_snapshots.push(cmd::string(&resolved.input_snapshot));
            native_records.extend(resolved.records);
        }
        for record in records.values() {
            if !used_refs.contains(cmd::text(&record.value, "record_id")?)
                && !used_refs.contains(&record.path)
            {
                return Err(SourceCommandError::Denied(
                    "private Claim source selection is outside its exact endpoint/evidence closure",
                ));
            }
        }
        native_snapshots.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
        for key in ["evidence_refs", "counterevidence_refs"] {
            for item in optional_array(claim, key)? {
                let reference = item.as_str().ok_or(SourceCommandError::Invalid(
                    "private Claim evidence reference",
                ))?;
                let selected_source = source_aliases.contains(reference);
                let native_matches = native_aliases.get(reference).map_or(0, BTreeSet::len);
                if selected_source {
                    if native_matches != 0 {
                        return Err(SourceCommandError::Conflict(
                            "private Claim evidence alias is ambiguous across source and native adapters",
                        ));
                    }
                } else if native_matches == 0 {
                    return Err(SourceCommandError::Unsupported(
                        "private Claim evidence source is not in its exact selected closure",
                    ));
                } else if native_matches != 1 {
                    return Err(SourceCommandError::Conflict(
                        "private Claim native evidence alias is ambiguous",
                    ));
                }
            }
        }
        for quote in optional_array(claim, "supporting_quotes")? {
            let anchor = cmd::text(quote, "anchor_ref")?;
            if native_anchors.get(anchor).map_or(0, BTreeSet::len) != 1 {
                return Err(SourceCommandError::Denied(
                    "private Claim quote anchor requires exactly one selected native binding",
                ));
            }
        }
        // The maintained reader's dependency_refs includes both selected
        // source records and the unit/layer assessment records emitted by its
        // native adapter. Preserve its identity deduplication and ordering.
        let mut required_records = BTreeMap::<String, JsonValue>::new();
        for source in records.values() {
            let envelope = cmd::object(vec![
                ("id", cmd::field(&source.reference, "id")?.clone()),
                ("version", cmd::field(&source.reference, "version")?.clone()),
                ("payload", source.value.clone()),
                ("origin_id", cmd::string(&source.origin)),
            ]);
            insert_claim_dependency(&mut required_records, identity, envelope)?;
        }
        for native in native_records {
            insert_claim_dependency(&mut required_records, identity, native)?;
        }
        let sorted_sources = required_records
            .values()
            .map(assessment_native_ref)
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let summaries = native_summaries.clone();
        bindings.push(cmd::object(vec![
            ("claim", record_ref(claim)?),
            ("required_sources", JsonValue::Array(sorted_sources)),
            ("native_sources", JsonValue::Array(summaries)),
        ]));
        let mut source_map = BTreeMap::new();
        for record in records.values() {
            let raw = reads
                .files
                .get(&record.path)
                .ok_or(SourceCommandError::Conflict(
                    "private Claim source is absent from its exact read closure",
                ))?;
            source_map.insert(record.path.clone(), digest(raw));
        }
        let mut private_profiles = BTreeMap::<String, &GroundRecord>::new();
        for record in records.values().filter(|record| record.private) {
            if let Some(previous) = private_profiles.insert(record.path.clone(), record)
                && (previous.source_access != record.source_access
                    || previous.source_binding != record.source_binding
                    || previous.profile_dependencies != record.profile_dependencies)
            {
                return Err(SourceCommandError::Conflict(
                    "one private Claim profile source has conflicting selected readers",
                ));
            }
        }
        let mut private_record_snapshots = private_profiles
            .values()
            .map(|record| private_profile_snapshot(record, &grant.context_snapshot, &reads))
            .collect::<SourceCommandResult<Vec<_>>>()?;
        private_record_snapshots.sort();
        let mut public_profile_bindings = Vec::new();
        for record in records.values().filter(|record| !record.private) {
            let Some(profile) = grammar.profile_by_type.get(&record.profile_type_id) else {
                if record.source_binding.is_some() {
                    return Err(SourceCommandError::Unsupported(
                        "native Corpus endpoints cannot carry a source-profile binding",
                    ));
                }
                continue;
            };
            let has_adapter = profile
                .object_get("native_binding_adapter")
                .and_then(JsonValue::as_str)
                .is_some();
            match (has_adapter, record.source_binding.as_ref()) {
                (true, Some(binding)) => public_profile_bindings.push(binding.clone()),
                (true, None) => {
                    return Err(SourceCommandError::Denied(
                        "public Claim occurrence lacks its selected native text binding",
                    ));
                }
                (false, Some(_)) => {
                    return Err(SourceCommandError::Unsupported(
                        "public Claim profile has no declared native binding adapter",
                    ));
                }
                (false, None) => {}
            }
        }
        let public_native = if public_profile_bindings.is_empty() {
            None
        } else {
            Some(public_native_snapshot(
                owner,
                cut,
                context,
                worker,
                &mut reads,
                &public_profile_bindings,
                deadline,
                cancelled,
            )?)
        };
        let public_identity_snapshot = if native_identity_claims.contains(identity) {
            Some(
                public_native_identity
                    .as_ref()
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim native identity snapshot is absent",
                    ))?,
            )
        } else {
            None
        };
        let mut contract_map = BTreeMap::new();
        for (path, raw) in &reads.files {
            if path.starts_with("ToS/contracts/") || path == RELATIONS || path == ENTITIES {
                contract_map.insert(path.clone(), digest(raw));
            }
        }
        let candidate_digest = cmd::record_digest(claim)?.to_prefixed();
        let selection_value = JsonValue::Array(vec![
            cmd::string(grant.source_path.as_str()),
            cmd::string(identity),
            cmd::string(&selection.origin_id),
            cmd::string(&selection.relation_type_id),
        ]);
        let snapshot_basis = cmd::object(vec![
            ("context", cmd::string(&grant.context_snapshot)),
            ("selection", selection_value),
            ("source_access", selection.source_access.clone()),
            (
                "source_selections",
                JsonValue::Array(selection.source_records.clone()),
            ),
            (
                "native_selections",
                JsonValue::Array(selection.native_bindings.clone()),
            ),
            ("verify_content", JsonValue::Bool(selection.verify_content)),
            ("contracts", hash_map(contract_map)),
            ("sources", hash_map(source_map)),
            ("claim_stream", JsonValue::Null),
            (
                "candidate",
                cmd::object(vec![
                    ("mode", cmd::string("candidate")),
                    ("digest", cmd::string(&candidate_digest)),
                ]),
            ),
            ("native", JsonValue::Array(native_snapshots)),
            (
                "private_records",
                JsonValue::Array(
                    private_record_snapshots
                        .into_iter()
                        .map(|value| cmd::string(&value))
                        .collect(),
                ),
            ),
            (
                "public_native_identity",
                public_identity_snapshot
                    .map(|value| cmd::string(value))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "public_native",
                public_native
                    .as_ref()
                    .map(|value| cmd::string(value))
                    .unwrap_or(JsonValue::Null),
            ),
        ]);
        snapshots.insert(
            identity.to_owned(),
            cmd::record_digest(&snapshot_basis)?.to_prefixed(),
        );
        source_fields.extend(form_fields_for_claim(claim)?);
    }
    let grounding = hash_map(snapshots.clone());
    let dependency_basis = cmd::object(vec![
        ("grounding", grounding),
        ("identity", cmd::string(&identity_digest)),
        ("form_grammar", form_grammar.clone()),
        ("provenance_contract", cmd::string(&provenance_digest)),
        ("implementation", implementation),
    ]);
    let dependencies = cmd::record_digest(&dependency_basis)?.to_prefixed();
    Ok(GroundedClaims {
        dependencies,
        bindings,
        source_fields,
        materializations,
        identity_digest,
        identity_files,
        identity_ids,
        snapshots,
        reads: reads.into_source_files()?,
    })
}

fn assessment_claim_endpoint_closure(
    grammar: &ClaimGrammar,
    route: &ClaimRoute,
    claim: &JsonValue,
    records: &BTreeMap<String, GroundRecord>,
) -> SourceCommandResult<()> {
    let subject = cmd::text(claim, "subject_ref")?;
    let subject_record = records.get(subject).ok_or(SourceCommandError::Unsupported(
        "private Claim exact subject metadata selection",
    ))?;
    if !source_type_allowed(grammar, &subject_record.profile_type_id, &route.domain)? {
        return Err(SourceCommandError::Invalid(
            "private Claim subject violates selected relation domain",
        ));
    }
    if let Some(object) = cmd::field(claim, "object")?.as_str() {
        let object_record = records.get(object).ok_or(SourceCommandError::Unsupported(
            "private Claim exact object metadata selection",
        ))?;
        if !source_type_allowed(grammar, &object_record.profile_type_id, &route.range)? {
            return Err(SourceCommandError::Invalid(
                "private Claim object violates selected relation range",
            ));
        }
    } else if route.reader == "structured-reference-value-v1"
        && route.profile.object_get("object_reference_set").is_some()
    {
        let object = cmd::field(claim, "object")?;
        let members = cmd::array(object, "members")?;
        let constraint = cmd::field(&route.profile, "object_reference_set")?;
        let min_items = cmd::integer(constraint, "min_items")? as usize;
        let max_items = cmd::integer(constraint, "max_items")? as usize;
        if members.len() < min_items || members.len() > max_items {
            return Err(SourceCommandError::Invalid(
                "private Claim reference value member budget",
            ));
        }
        let allowed = field_texts(constraint, "member_type_ids", 32)?;
        let mut seen = BTreeSet::new();
        for member in members {
            let id = member.as_str().ok_or(SourceCommandError::Invalid(
                "private Claim reference member identity",
            ))?;
            if !seen.insert(id) || id == cmd::text(claim, "claim_id")? {
                return Err(SourceCommandError::Invalid(
                    "private Claim reference member repeats",
                ));
            }
            let row = records.get(id).ok_or(SourceCommandError::Unsupported(
                "private Claim exact reference member selection",
            ))?;
            if !source_type_allowed(grammar, &row.profile_type_id, &allowed)? {
                return Err(SourceCommandError::Invalid(
                    "private Claim reference member violates selected type set",
                ));
            }
        }
        if cmd::field(constraint, "subject_is_member")? == &JsonValue::Bool(true)
            && !seen.contains(subject)
        {
            return Err(SourceCommandError::Invalid(
                "private Claim reference value omits its required subject member",
            ));
        }
    } else if route.reader == "historical-temporal-v1"
        && cmd::field(cmd::field(claim, "object")?, "kind")?.as_str() == Some("relative-order")
    {
        let object = cmd::field(claim, "object")?;
        let relative = cmd::field(object, "relative")?;
        let anchor = cmd::text(relative, "anchor_ref")?;
        let anchor_record = records.get(anchor).ok_or(SourceCommandError::Invalid(
            "private Claim relative-order anchor is not selected",
        ))?;
        if !source_type_allowed(
            grammar,
            &anchor_record.profile_type_id,
            &["tos.entity.historical-situation".to_owned()],
        )? {
            return Err(SourceCommandError::Invalid(
                "private Claim relative-order anchor violates the historical-situation profile",
            ));
        }
    } else if cmd::field(claim, "object")?
        .object_get("relative")
        .and_then(|value| value.object_get("anchor_ref"))
        .and_then(JsonValue::as_str)
        .is_some()
    {
        return Err(SourceCommandError::Invalid(
            "private Claim relative-order anchor is outside the historical-temporal profile",
        ));
    }
    for key in ["evidence_refs", "counterevidence_refs"] {
        for reference in optional_array(claim, key)? {
            let reference = reference.as_str().ok_or(SourceCommandError::Invalid(
                "private Claim evidence identity",
            ))?;
            if reference.starts_with("ToS/") {
                let path = RelativePath::parse(reference)
                    .map_err(|_| SourceCommandError::Invalid("private Claim evidence path"))?;
                if path.as_str().split('/').any(|part| {
                    part.starts_with('.') || matches!(part, "catalog" | "payload" | "local-content")
                }) {
                    return Err(SourceCommandError::Denied(
                        "private Claim evidence excludes hidden and content carriers",
                    ));
                }
            } else if !cmd::nonblank(reference) {
                return Err(SourceCommandError::Invalid(
                    "private Claim evidence identity",
                ));
            }
        }
    }
    Ok(())
}

fn assessment_native_ref(record: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::field(record, "id")?.clone()),
        ("version", cmd::field(record, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
        ),
    ]))
}

fn assessment_languages(
    claim: &JsonValue,
    route: &ClaimRoute,
    sources: &BTreeMap<String, GroundRecord>,
    native_summaries: &[JsonValue],
) -> SourceCommandResult<Vec<String>> {
    let mut languages = BTreeSet::new();
    let mut add = |value: Option<&str>| {
        if let Some(value) = value.filter(|value| cmd::nonblank(value)) {
            languages.insert(value.to_lowercase());
        }
    };
    if let Some(language) = claim
        .object_get("qualifiers")
        .and_then(|value| value.object_get("statement_language"))
        .and_then(JsonValue::as_str)
    {
        add(Some(language));
    }
    if route.reader == "structured-reference-value-v1" {
        if let Some(language) = claim
            .object_get("object")
            .and_then(|value| value.object_get("source_wording"))
            .and_then(|value| value.object_get("language"))
            .and_then(JsonValue::as_str)
        {
            add(Some(language));
        }
    }
    for source in sources.values() {
        let payload = &source.value;
        if let Some(fields) = payload.object_get("field_languages") {
            for (_, field) in fields
                .as_object()
                .ok_or(SourceCommandError::Invalid("private Claim field languages"))?
            {
                add(field.object_get("language").and_then(JsonValue::as_str));
            }
        }
        for field in ["semantic_scope", "semantic_content", "form_identity"] {
            add(payload
                .object_get(field)
                .and_then(|value| value.object_get("language"))
                .and_then(JsonValue::as_str));
        }
    }
    for summary in native_summaries {
        add(cmd::field(summary, "language")
            .ok()
            .and_then(JsonValue::as_str));
    }
    Ok(languages.into_iter().collect())
}

fn assessment_payload_languages(payload: &JsonValue) -> SourceCommandResult<Vec<String>> {
    let mut languages = BTreeSet::new();
    let mut add = |value: Option<&str>| {
        if let Some(value) = value.filter(|value| cmd::nonblank(value)) {
            languages.insert(value.to_lowercase());
        }
    };
    add(payload.object_get("language").and_then(JsonValue::as_str));
    if let Some(fields) = payload.object_get("field_languages") {
        for (_, field) in fields
            .as_object()
            .ok_or(SourceCommandError::Invalid("private Claim field languages"))?
        {
            add(field.object_get("language").and_then(JsonValue::as_str));
        }
    }
    for field in ["semantic_scope", "semantic_content", "form_identity"] {
        add(payload
            .object_get(field)
            .and_then(|value| value.object_get("language"))
            .and_then(JsonValue::as_str));
    }
    Ok(languages.into_iter().collect())
}

/// Resolve selected owner-local Claims into assessment-only record envelopes
/// and exact source closure. All storage reads are read-only; native sources
/// use the caller's pinned `SignNativeRead` transport and configured scope.
pub(crate) fn resolve_assessment_claim_sources(
    owner: &OwnerTextContext,
    context: &JsonValue,
    selections: &[AssessmentClaimSelection],
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    reader: &mut dyn SignNativeRead,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentClaimSources> {
    if selections.is_empty() {
        return Ok(AssessmentClaimSources {
            records: Vec::new(),
            native_records: Vec::new(),
            required_source_refs: BTreeMap::new(),
            required_languages: BTreeMap::new(),
            native_summaries: Vec::new(),
            native_inputs: Vec::new(),
            native_snapshots: Vec::new(),
            schema_digests: BTreeMap::new(),
            snapshots: Vec::new(),
            source_files: BTreeMap::new(),
            form_sets: BTreeMap::new(),
            form_paths: BTreeMap::new(),
            source_paths: BTreeMap::new(),
            public_native_identity_paths: None,
        });
    }
    if selections.len() > 32
        || cut.current().revision() != ctx.base_revision
        || worker.source_revision() != ctx.base_revision
    {
        return Err(SourceCommandError::Conflict(
            "private assessment Claim worker and source cut differ",
        ));
    }
    let mut claim_ids = BTreeSet::new();
    for selection in selections {
        owner_path_claim_stream(&selection.path, context)?;
        source_access(&selection.source_access, true)?;
        if selection.verify_content
            && cmd::text(&selection.source_access, "read_scope")? != "exact_owner_local"
        {
            return Err(SourceCommandError::Denied(
                "exact Claim evidence is outside its selected source access scope",
            ));
        }
        if !claim_ids.insert(selection.claim_id.clone()) {
            return Err(SourceCommandError::Invalid(
                "private assessment Claim selection repeats",
            ));
        }
    }
    let mut reads = ExactReads::default();
    let grammar = ClaimGrammar::load(
        ctx, owner, cut, context, worker, &mut reads, deadline, cancelled,
    )?;
    preflight_assessment_claim_sources(
        ctx, owner, cut, context, &grammar, selections, worker, &mut reads, deadline, cancelled,
    )?;
    let form_grammar = super::profile::form_grammar_digests(ctx, worker, deadline, cancelled)?;
    let mut claims = BTreeMap::<String, JsonValue>::new();
    let mut routes = BTreeMap::<String, ClaimRoute>::new();
    let mut claim_streams = BTreeMap::<String, String>::new();
    for selection in selections {
        crate::source_creation_store::active(deadline, cancelled)?;
        let raw = assessment_reader_read(
            owner,
            cut,
            context,
            reader,
            &selection.path,
            MAX_ASSESSMENT_CLAIM_FILE_BYTES,
            &mut reads,
            deadline,
            cancelled,
        )?;
        let all_claims = parse_assessment_claim_stream(&raw)?;
        let claim = all_claims
            .get(&selection.claim_id)
            .ok_or(SourceCommandError::Invalid(
                "selected private Claim is absent from its source stream",
            ))?
            .clone();
        let route = assessment_claim_route(&grammar, selection, &claim)?;
        check_claim_schema(
            ctx, owner, cut, context, &grammar, &claim, &route, worker, &mut reads, deadline,
            cancelled,
        )?;
        let stream_digest = Digest256::of_bytes(&raw).to_hex();
        if claim_streams
            .insert(selection.path.clone(), stream_digest.clone())
            .is_some_and(|prior| prior != stream_digest)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim stream changed between selections",
            ));
        }
        claims.insert(selection.claim_id.clone(), claim);
        routes.insert(selection.claim_id.clone(), route);
    }

    let mut records = Vec::new();
    let mut native_records = Vec::new();
    let mut required_source_refs = BTreeMap::new();
    let mut required_languages = BTreeMap::new();
    let mut native_summaries = Vec::new();
    let mut native_snapshots = Vec::new();
    let mut native_inputs =
        BTreeMap::<(String, String, &'static str), crate::source_sign_native::NativeInput>::new();
    let mut schema_digests = BTreeMap::<String, Digest256>::new();
    let mut claim_snapshots = Vec::new();
    let mut form_sets = BTreeMap::new();
    let mut form_paths = BTreeMap::new();
    let mut source_paths = BTreeMap::new();
    let mut public_native_identity_paths: Option<BTreeSet<String>> = None;
    let mut seen_record_bodies = BTreeMap::<String, JsonValue>::new();

    for selection in selections {
        crate::source_creation_store::active(deadline, cancelled)?;
        let claim = claims
            .get(&selection.claim_id)
            .ok_or(SourceCommandError::Invalid("private Claim row absent"))?;
        let route = routes
            .get(&selection.claim_id)
            .ok_or(SourceCommandError::Invalid("private Claim route absent"))?;
        if source_paths
            .insert(selection.claim_id.clone(), selection.path.clone())
            .is_some_and(|previous| previous != selection.path)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim identity has conflicting selected paths",
            ));
        }
        let used_refs = claim_identity_refs(claim)?;
        let mut source_records = BTreeMap::<String, GroundRecord>::new();
        let mut seen_paths = BTreeSet::new();
        let mut native_entries = Vec::<NativeSelection>::new();
        let mut native_entry_indexes = BTreeMap::<String, usize>::new();
        for selector in &selection.source_records {
            let source_id = cmd::text(selector, "record_id")?;
            let path = cmd::text(selector, "path")?;
            if !used_refs.contains(source_id) && !used_refs.contains(path) {
                return Err(SourceCommandError::Denied(
                    "private Claim selected an unrelated source record",
                ));
            }
            if !seen_paths.insert(path.to_owned()) || source_records.contains_key(source_id) {
                return Err(SourceCommandError::Conflict(
                    "private Claim source record selection repeats",
                ));
            }
            if source_paths
                .insert(source_id.to_owned(), path.to_owned())
                .is_some_and(|previous| previous != path)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim endpoint has conflicting selected paths",
                ));
            }
            let source = validate_endpoint_record(
                ctx,
                cut,
                owner,
                context,
                &grammar,
                selector,
                selection.verify_content,
                worker,
                &mut reads,
                deadline,
                cancelled,
                Some(&mut *reader),
            )?;
            let bound = cmd::field(selector, "source_binding")?;
            if !bound.is_null() {
                let profile = grammar.profile_by_type.get(&source.profile_type_id).ok_or(
                    SourceCommandError::Denied(
                        "native Claim binding cannot attach to an implicit Corpus profile",
                    ),
                )?;
                if cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1"
                    || selection.verify_content
                        && cmd::text(cmd::field(selector, "source_access")?, "read_scope")?
                            != "exact_owner_local"
                {
                    return Err(SourceCommandError::Denied(
                        "Claim endpoint native binding needs its exact declared adapter and access",
                    ));
                }
                let key = String::from_utf8(cmd::canonical(bound)?)
                    .map_err(|_| SourceCommandError::Invalid("Claim native binding encoding"))?;
                native_entries.push(NativeSelection {
                    key,
                    binding: bound.clone(),
                    origin_id: cmd::text(selector, "origin_id")?.to_owned(),
                    source_access: cmd::field(selector, "source_access")?.clone(),
                    source_ids: BTreeSet::from([source_id.to_owned()]),
                });
            }
            source_records.insert(source_id.to_owned(), source);
        }
        for selector in &selection.native_bindings {
            let binding = cmd::field(selector, "binding")?;
            let access = cmd::field(selector, "source_access")?;
            if selection.verify_content && cmd::text(access, "read_scope")? != "exact_owner_local" {
                return Err(SourceCommandError::Denied(
                    "Claim native evidence needs exact selected content verification",
                ));
            }
            let key = String::from_utf8(cmd::canonical(binding)?)
                .map_err(|_| SourceCommandError::Invalid("Claim native binding encoding"))?;
            native_entries.push(NativeSelection {
                key,
                binding: binding.clone(),
                origin_id: cmd::text(selector, "origin_id")?.to_owned(),
                source_access: access.clone(),
                source_ids: BTreeSet::new(),
            });
        }
        let candidates = std::mem::take(&mut native_entries);
        for entry in candidates {
            if let Some(index) = native_entry_indexes.get(&entry.key).copied() {
                let prior = native_entries
                    .get_mut(index)
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim native selection index",
                    ))?;
                if prior.origin_id != entry.origin_id || prior.source_access != entry.source_access
                {
                    return Err(SourceCommandError::Conflict(
                        "one native Claim binding has conflicting origins or access scopes",
                    ));
                }
                prior.source_ids.extend(entry.source_ids);
            } else {
                native_entry_indexes.insert(entry.key.clone(), native_entries.len());
                native_entries.push(entry);
            }
        }
        assessment_claim_endpoint_closure(&grammar, route, claim, &source_records)?;

        let evidence_refs = optional_array(claim, "evidence_refs")?
            .iter()
            .chain(optional_array(claim, "counterevidence_refs")?.iter())
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "private Claim evidence identity",
                    ))
            })
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let quote_anchors = optional_array(claim, "supporting_quotes")?
            .iter()
            .map(|quote| cmd::text(quote, "anchor_ref").map(str::to_owned))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        if evidence_refs.len() + quote_anchors.len() > MAX_EVIDENCE_REFS {
            return Err(SourceCommandError::Invalid(
                "private Claim evidence reference budget",
            ));
        }
        let source_aliases = source_records
            .iter()
            .flat_map(|(identity, record)| [identity.clone(), record.path.clone()])
            .collect::<BTreeSet<_>>();
        let mut native_aliases = BTreeMap::<String, BTreeSet<String>>::new();
        let mut native_anchors = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in &native_entries {
            for alias in binding_identities(&entry.binding)? {
                native_aliases
                    .entry(alias)
                    .or_default()
                    .insert(entry.key.clone());
            }
            for anchor in cmd::array(&entry.binding, "ordered_anchor_refs")? {
                let anchor = anchor.as_str().ok_or(SourceCommandError::Invalid(
                    "private Claim binding anchor identity",
                ))?;
                native_anchors
                    .entry(anchor.to_owned())
                    .or_default()
                    .insert(entry.key.clone());
            }
        }
        let mut used_native = BTreeSet::new();
        for entry in &native_entries {
            let aliases = binding_identities(&entry.binding)?;
            let evidence_used = evidence_refs
                .iter()
                .any(|reference| aliases.contains(reference));
            let quote_used = entry
                .binding
                .object_get("ordered_anchor_refs")
                .and_then(JsonValue::as_array)
                .is_some_and(|anchors| {
                    anchors
                        .iter()
                        .filter_map(JsonValue::as_str)
                        .any(|anchor| quote_anchors.contains(anchor))
                });
            if !entry.source_ids.is_empty() || evidence_used || quote_used {
                used_native.insert(entry.key.clone());
            } else {
                return Err(SourceCommandError::Denied(
                    "private Claim native selection is outside its exact evidence closure",
                ));
            }
            if entry
                .source_ids
                .iter()
                .any(|identity| !source_records.contains_key(identity))
            {
                return Err(SourceCommandError::Denied(
                    "private Claim native selection lacks its exact source-record carrier",
                ));
            }
        }
        for reference in &evidence_refs {
            let source_match = source_aliases.contains(reference);
            let native_match = native_aliases.get(reference).map_or(0, BTreeSet::len);
            if source_match {
                if native_match != 0 {
                    return Err(SourceCommandError::Conflict(
                        "private Claim evidence alias is ambiguous across source and native adapters",
                    ));
                }
            } else if native_match == 0 {
                return Err(SourceCommandError::Unsupported(
                    "private Claim evidence source is not in its exact selected closure",
                ));
            } else if native_match != 1 {
                return Err(SourceCommandError::Conflict(
                    "private Claim native evidence alias is ambiguous",
                ));
            } else if let Some(keys) = native_aliases.get(reference) {
                used_native.extend(keys.iter().cloned());
            }
        }
        for anchor in &quote_anchors {
            let keys = native_anchors
                .get(anchor)
                .ok_or(SourceCommandError::Denied(
                    "private Claim quote anchor requires exactly one selected native binding",
                ))?;
            if keys.len() != 1 {
                return Err(SourceCommandError::Denied(
                    "private Claim quote anchor requires exactly one selected native binding",
                ));
            }
            used_native.extend(keys.iter().cloned());
        }
        if used_native.len() != native_entries.len() {
            return Err(SourceCommandError::Denied(
                "private Claim native selection is outside its exact evidence closure",
            ));
        }

        let mut claim_native_summaries = Vec::new();
        let mut claim_native_snapshots = Vec::new();
        let mut claim_required = BTreeMap::<String, JsonValue>::new();
        for source in source_records.values() {
            let envelope = cmd::object(vec![
                ("id", cmd::field(&source.reference, "id")?.clone()),
                ("version", cmd::field(&source.reference, "version")?.clone()),
                ("payload", source.value.clone()),
                ("origin_id", cmd::string(&source.origin)),
            ]);
            insert_claim_dependency(&mut claim_required, &selection.claim_id, envelope)?;
        }
        let mut metadata_resolutions = Vec::with_capacity(native_entries.len());
        for entry in &native_entries {
            let resolved = {
                let mut scoped_reader = AssessmentClaimScopedReader {
                    reader,
                    allow_content: false,
                };
                crate::source_sign_native::resolve_owner_assessment(
                    &mut scoped_reader,
                    worker,
                    &entry.binding,
                    &entry.origin_id,
                    crate::source_sign_native::NativeReadScope::MetadataOnly,
                    deadline,
                    cancelled,
                )?
            };
            if cmd::field(&resolved.summary, "content_verified")? != &JsonValue::Bool(false) {
                return Err(SourceCommandError::Conflict(
                    "metadata-only private Claim preflight disclosed native content",
                ));
            }
            let metadata_inputs = resolved
                .inputs
                .iter()
                .filter(|input| input.category != "content")
                .cloned()
                .collect::<Vec<_>>();
            let metadata_snapshot =
                owner_metadata_snapshot(owner, context, &metadata_inputs, deadline, cancelled)?;
            for source_id in &entry.source_ids {
                let source =
                    source_records
                        .get_mut(source_id)
                        .ok_or(SourceCommandError::Invalid(
                            "private Claim native binding source record is absent",
                        ))?;
                source.private_native_metadata = Some(metadata_snapshot.clone());
            }
            metadata_resolutions.push((entry.clone(), resolved));
        }

        // The maintained private reader closes every binding's metadata and
        // rights before exact content is opened for any selected binding.
        for (entry, metadata) in metadata_resolutions {
            let resolved = if selection.verify_content {
                let mut scoped_reader = AssessmentClaimScopedReader {
                    reader,
                    allow_content: true,
                };
                crate::source_sign_native::resolve_owner_assessment(
                    &mut scoped_reader,
                    worker,
                    &entry.binding,
                    &entry.origin_id,
                    crate::source_sign_native::NativeReadScope::ExactOwnerLocal,
                    deadline,
                    cancelled,
                )?
            } else {
                metadata
            };
            let verified = cmd::field(&resolved.summary, "content_verified")?;
            if verified != &JsonValue::Bool(selection.verify_content) {
                return Err(SourceCommandError::Conflict(
                    "private Claim native content observation differs from selected verify mode",
                ));
            }
            retain_assessment_native_inputs(
                reader,
                &mut reads,
                &resolved.inputs,
                deadline,
                cancelled,
            )?;
            let refs = resolved
                .records
                .iter()
                .map(assessment_native_ref)
                .collect::<SourceCommandResult<Vec<_>>>()?;
            if selection.verify_content {
                let exact_snapshot =
                    owner_metadata_snapshot(owner, context, &resolved.inputs, deadline, cancelled)?;
                for source_id in &entry.source_ids {
                    let source =
                        source_records
                            .get_mut(source_id)
                            .ok_or(SourceCommandError::Invalid(
                                "private Claim native binding source record is absent",
                            ))?;
                    source.private_native_exact = Some(exact_snapshot.clone());
                }
            }
            let mut summary = resolved.summary.clone();
            cmd::set(&mut summary, "origin_id", cmd::string(&entry.origin_id))?;
            cmd::set(
                &mut summary,
                "read_scope",
                cmd::field(&entry.source_access, "read_scope")?.clone(),
            )?;
            cmd::set(&mut summary, "record_refs", JsonValue::Array(refs))?;
            cmd::set(&mut summary, "supporting_only", JsonValue::Bool(true))?;
            claim_native_summaries.push(summary.clone());
            native_summaries.push(summary);
            claim_native_snapshots.push(resolved.input_snapshot.clone());
            native_snapshots.push(resolved.input_snapshot);
            for input in &resolved.inputs {
                let kind = match input.kind {
                    crate::source_sign_native::NativeReadKind::Metadata => "metadata",
                    crate::source_sign_native::NativeReadKind::Schema => "schema",
                    crate::source_sign_native::NativeReadKind::Support => "support",
                    crate::source_sign_native::NativeReadKind::Content => "content",
                }
                .to_owned();
                let key = (input.reference.clone(), kind, input.category);
                if native_inputs
                    .insert(key, input.clone())
                    .is_some_and(|prior| prior.raw_sha256 != input.raw_sha256)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim native input changed during selection",
                    ));
                }
            }
            for (reference, digest) in resolved.schema_digests {
                if schema_digests
                    .insert(reference, digest)
                    .is_some_and(|prior| prior != digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim native schema dependency changed",
                    ));
                }
            }
            for native in resolved.records {
                insert_claim_dependency(&mut claim_required, &selection.claim_id, native.clone())?;
                native_records.push(native);
            }
        }
        for source in source_records.values() {
            if !used_refs.contains(cmd::text(&source.value, "record_id")?)
                && !used_refs.contains(&source.path)
            {
                return Err(SourceCommandError::Denied(
                    "private Claim source selection is outside its exact endpoint/evidence closure",
                ));
            }
        }

        let claim_envelope = cmd::object(vec![
            ("id", cmd::text(claim, "claim_id").map(cmd::string)?),
            ("version", cmd::field(claim, "claim_version")?.clone()),
            ("payload", claim.clone()),
            ("origin_id", cmd::string(&selection.origin_id)),
        ]);
        let claim_reference = assessment_native_ref(&claim_envelope)?;
        for source in source_records.values() {
            let envelope = cmd::object(vec![
                ("id", cmd::field(&source.reference, "id")?.clone()),
                ("version", cmd::field(&source.reference, "version")?.clone()),
                ("payload", source.value.clone()),
                ("origin_id", cmd::string(&source.origin)),
            ]);
            let identity = cmd::text(&envelope, "id")?.to_owned();
            retain_assessment_record(&mut seen_record_bodies, &mut records, identity, envelope)?;
        }
        retain_assessment_record(
            &mut seen_record_bodies,
            &mut records,
            selection.claim_id.clone(),
            claim_envelope.clone(),
        )?;
        let mut languages =
            assessment_languages(claim, route, &source_records, &claim_native_summaries)?;
        required_languages.insert(selection.claim_id.clone(), languages.clone());
        let mut dependency_refs = claim_required
            .values()
            .map(assessment_native_ref)
            .collect::<SourceCommandResult<Vec<_>>>()?;
        dependency_refs.sort_by(|left, right| {
            left.object_get("id")
                .and_then(JsonValue::as_str)
                .cmp(&right.object_get("id").and_then(JsonValue::as_str))
        });
        required_source_refs.insert(selection.claim_id.clone(), dependency_refs.clone());

        if !selection.form_ids.is_empty() {
            let parent = selection
                .path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(SourceCommandError::Invalid("private Claim form path"))?;
            let form_path = format!("{parent}/{}", claim_form_filename(&selection.claim_id));
            let raw = assessment_reader_read(
                owner,
                cut,
                context,
                reader,
                &form_path,
                2 * MAX_RECORD_BYTES,
                &mut reads,
                deadline,
                cancelled,
            )?;
            let forms = cmd::parse(&raw)?;
            if cmd::canonical(&forms)?.len() > MAX_RECORD_BYTES {
                return Err(SourceCommandError::Invalid(
                    "private Claim form-set canonical byte budget",
                ));
            }
            super::profile::validate_form_set(
                ctx, worker, &form_path, &forms, deadline, cancelled,
            )?;
            let subject = assessment_native_ref(&claim_envelope)?;
            if cmd::canonical(cmd::field(&forms, "subject")?)? != cmd::canonical(&subject)? {
                return Err(SourceCommandError::Conflict(
                    "private Claim form set binds a different source snapshot",
                ));
            }
            tos_validation::source_forms::source_copy_kernel::validate_history(&forms, &subject)
                .map_err(|_| SourceCommandError::Invalid("private Claim form history"))?;
            form_sets.insert(form_path.clone(), forms.clone());
            form_paths.insert(selection.claim_id.clone(), form_path.clone());
            let current_forms = cmd::array(&forms, "forms")?;
            for identifier in &selection.form_ids {
                if form_paths
                    .insert(identifier.clone(), form_path.clone())
                    .is_some_and(|previous| previous != form_path)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim form identity has conflicting adjacent form paths",
                    ));
                }
                let matching = current_forms
                    .iter()
                    .filter(|form| cmd::text(form, "form_id").ok() == Some(identifier.as_str()))
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err(SourceCommandError::Invalid(
                        "private Claim selected form is absent or not current",
                    ));
                }
                let form = matching[0];
                if cmd::canonical(cmd::field(form, "subject")?)? != cmd::canonical(&subject)? {
                    return Err(SourceCommandError::Conflict(
                        "private Claim selected form binds another source snapshot",
                    ));
                }
                let form_envelope = cmd::object(vec![
                    ("id", cmd::string(identifier)),
                    ("version", cmd::field(form, "form_version")?.clone()),
                    ("payload", form.clone()),
                    ("origin_id", cmd::string(&selection.origin_id)),
                ]);
                retain_assessment_record(
                    &mut seen_record_bodies,
                    &mut records,
                    identifier.clone(),
                    form_envelope.clone(),
                )?;
                let mut form_refs = Vec::with_capacity(dependency_refs.len() + 1);
                form_refs.push(claim_reference.clone());
                form_refs.extend(dependency_refs.clone());
                required_source_refs.insert(identifier.clone(), form_refs);
                let mut form_languages = languages.clone();
                form_languages.extend(assessment_payload_languages(form)?);
                required_languages.insert(identifier.clone(), form_languages);
            }
        }

        // The per-Claim digest follows the maintained owner-local Claim source
        // snapshot fields; the adapter separately binds all raw bytes and held
        // descriptors for post-lock currentness.
        let mut source_map = BTreeMap::new();
        let mut private_records = Vec::new();
        for selector in &selection.source_records {
            let path = cmd::text(selector, "path")?;
            let raw = reads.files.get(path).ok_or(SourceCommandError::Conflict(
                "private Claim selected endpoint is absent from its exact read closure",
            ))?;
            let digest = Digest256::of_bytes(raw).to_hex();
            if source_map
                .insert(path.to_owned(), digest.clone())
                .is_some_and(|previous| previous != digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim selected endpoint changed in its snapshot",
                ));
            }
        }
        let mut private_profiles = BTreeMap::<String, &GroundRecord>::new();
        for source in source_records.values().filter(|source| source.private) {
            private_profiles.insert(source.path.clone(), source);
        }
        for source in private_profiles.values() {
            private_records.push(private_profile_snapshot(
                source,
                &owner.snapshot(deadline, cancelled)?.to_prefixed(),
                &reads,
            )?);
        }
        private_records.sort();

        let native_identity_subjects = source_records
            .values()
            .filter(|source| !source.private)
            .filter_map(|source| {
                source
                    .value
                    .object_get("record_id")
                    .and_then(JsonValue::as_str)
                    .filter(|identity| native_identity_subject(identity))
                    .map(str::to_owned)
            })
            .collect::<BTreeSet<_>>();
        let public_native_identity = if native_identity_subjects.is_empty() {
            JsonValue::Null
        } else {
            let inventory_context =
                native_identity_inventory_context(ctx, cut, deadline, cancelled)?;
            let (identities, snapshot, schema_used) =
                crate::source_revisions::native_identity_inventory_from_cut(
                    &inventory_context,
                    cut,
                    worker,
                    deadline,
                    cancelled,
                )?;
            if native_identity_subjects
                .iter()
                .any(|identity| identities.contains_key(identity))
            {
                return Err(SourceCommandError::Denied(
                    "Claim source identity is already owned by a native semantic packet; explicit owner migration required",
                ));
            }
            let mut inventory_paths = BTreeSet::new();
            for member in cut.current().members() {
                let reference = member.path.as_str();
                if public_native_identity_member(reference) {
                    selected_owner_read(
                        owner, cut, context, reference, 1_048_576, &mut reads, deadline, cancelled,
                    )?;
                    inventory_paths.insert(reference.to_owned());
                }
            }
            if schema_used {
                let reference = "ToS/contracts/semantic-annotation-packet-v2.schema.json";
                let raw = selected_authored(
                    ctx, owner, cut, context, reference, 1_048_576, &mut reads, deadline, cancelled,
                )?;
                require_contract_digest(worker, reference, &raw)?;
                let digest = Digest256::of_bytes(&raw);
                if schema_digests
                    .insert(reference.to_owned(), digest)
                    .is_some_and(|previous| previous != digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim native identity schema dependency changed",
                    ));
                }
            }
            public_native_identity_paths
                .get_or_insert_with(BTreeSet::new)
                .extend(inventory_paths);
            cmd::string(&snapshot)
        };

        let public_bindings = source_records
            .values()
            .filter(|source| !source.private)
            .filter_map(|source| source.source_binding.clone())
            .collect::<Vec<_>>();
        let public_native = if public_bindings.is_empty() {
            JsonValue::Null
        } else {
            let resolved = crate::source_sign_native::resolve_bindings(
                reader,
                worker,
                &public_bindings,
                crate::source_sign_native::NativeReadScope::MetadataOnly,
                deadline,
                cancelled,
            )?;
            for summary in &resolved.summaries {
                if cmd::field(summary, "public_content_declared")? != &JsonValue::Bool(true) {
                    return Err(SourceCommandError::Denied(
                        "public Claim source binding does not declare public content",
                    ));
                }
            }
            retain_assessment_native_inputs(
                reader,
                &mut reads,
                &resolved.inputs,
                deadline,
                cancelled,
            )?;
            for input in &resolved.inputs {
                let kind = match input.kind {
                    crate::source_sign_native::NativeReadKind::Metadata => "metadata",
                    crate::source_sign_native::NativeReadKind::Schema => "schema",
                    crate::source_sign_native::NativeReadKind::Support => "support",
                    crate::source_sign_native::NativeReadKind::Content => "content",
                }
                .to_owned();
                let key = (input.reference.clone(), kind, input.category);
                if native_inputs
                    .insert(key, input.clone())
                    .is_some_and(|previous| previous.raw_sha256 != input.raw_sha256)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim public native input changed during selection",
                    ));
                }
            }
            for (reference, digest) in resolved.schema_digests {
                if schema_digests
                    .insert(reference, digest)
                    .is_some_and(|previous| previous != digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim public native schema dependency changed",
                    ));
                }
            }
            cmd::string(&resolved.input_snapshot)
        };

        let mut contracts = BTreeMap::<String, String>::new();
        for (path, value) in &grammar.schema_digests {
            let digest = value.strip_prefix("sha256:").unwrap_or(value).to_owned();
            if contracts
                .insert(path.clone(), digest.clone())
                .is_some_and(|previous| previous != digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim grammar digest conflicts in its snapshot",
                ));
            }
        }
        for source in source_records.values() {
            for (path, digest) in &source.profile_dependencies {
                if contracts
                    .insert(path.clone(), digest.clone())
                    .is_some_and(|previous| previous != *digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim endpoint grammar digest conflicts in its snapshot",
                    ));
                }
            }
        }
        for (path, digest) in &schema_digests {
            let value = digest.to_hex();
            if contracts
                .insert(path.clone(), value.clone())
                .is_some_and(|previous| previous != value)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim native grammar digest conflicts in its snapshot",
                ));
            }
        }
        for (path, value) in &contracts {
            let parsed = if let Some(hex) = value.strip_prefix("sha256:") {
                Digest256::from_hex(hex).ok()
            } else {
                Digest256::from_hex(value).ok()
            };
            if let Some(digest) = parsed {
                if schema_digests
                    .insert(path.clone(), digest)
                    .is_some_and(|previous| previous != digest)
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim schema dependency changed",
                    ));
                }
            }
        }
        let source_map_value = hash_map(source_map);
        let contract_map = hash_map(
            contracts
                .iter()
                .map(|(path, value)| (path.clone(), value.clone()))
                .collect(),
        );
        let selection_value = JsonValue::Array(vec![
            cmd::string(&selection.path),
            cmd::string(&selection.claim_id),
            cmd::string(&selection.origin_id),
            cmd::string(&selection.relation_type_id),
        ]);
        let claim_stream = JsonValue::Array(vec![
            cmd::string(&selection.path),
            cmd::string(
                claim_streams
                    .get(&selection.path)
                    .ok_or(SourceCommandError::Invalid("private Claim stream snapshot"))?,
            ),
        ]);
        claim_native_snapshots.sort();
        let basis = cmd::object(vec![
            (
                "context",
                cmd::string(&owner.snapshot(deadline, cancelled)?.to_prefixed()),
            ),
            ("selection", selection_value),
            ("source_access", selection.source_access.clone()),
            (
                "source_selections",
                JsonValue::Array(selection.source_records.clone()),
            ),
            (
                "native_selections",
                JsonValue::Array(selection.native_bindings.clone()),
            ),
            ("verify_content", JsonValue::Bool(selection.verify_content)),
            ("contracts", contract_map),
            ("sources", source_map_value),
            ("claim_stream", claim_stream),
            (
                "native",
                JsonValue::Array(
                    claim_native_snapshots
                        .iter()
                        .map(|s| cmd::string(s))
                        .collect(),
                ),
            ),
            (
                "private_records",
                JsonValue::Array(private_records.iter().map(|s| cmd::string(s)).collect()),
            ),
            ("public_native_identity", public_native_identity),
            ("public_native", public_native),
        ]);
        claim_snapshots.push(cmd::record_digest(&basis)?.to_prefixed());
    }

    if records.len() + native_records.len() > tos_validation::assessment::MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment Claim source record budget",
        ));
    }
    let mut final_schema_digests = schema_digests;
    for (path, raw) in &reads.files {
        if path.starts_with("ToS/contracts/") || path == RELATIONS || path == ENTITIES {
            let digest = Digest256::of_bytes(raw);
            if final_schema_digests
                .insert(path.clone(), digest)
                .is_some_and(|previous| previous != digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim schema dependency changed during source selection",
                ));
            }
        }
    }
    Ok(AssessmentClaimSources {
        records,
        native_records,
        required_source_refs,
        required_languages,
        native_summaries,
        native_inputs: native_inputs.into_values().collect(),
        native_snapshots,
        schema_digests: final_schema_digests,
        snapshots: claim_snapshots,
        source_files: reads.files,
        form_sets,
        form_paths,
        source_paths,
        public_native_identity_paths,
    })
}

fn retain_assessment_record(
    seen: &mut BTreeMap<String, JsonValue>,
    output: &mut Vec<JsonValue>,
    identity: String,
    record: JsonValue,
) -> SourceCommandResult<()> {
    if let Some(previous) = seen.get(&identity) {
        if !same_json(previous, &record)? {
            return Err(SourceCommandError::Conflict(
                "private assessment record identity resolves to different envelopes",
            ));
        }
    } else {
        seen.insert(identity, record.clone());
        output.push(record);
    }
    Ok(())
}

struct PackageState {
    records: BTreeMap<String, JsonValue>,
    forms: BTreeMap<String, JsonValue>,
    history: JsonValue,
    receipt: JsonValue,
    initial_archive_revision: Option<String>,
    archive_locations: Vec<JsonValue>,
}

fn historical_grant(raw: &[u8], context: &JsonValue) -> SourceCommandResult<Grant> {
    let value = cmd::parse(raw)?;
    let version = ConfigVersion::from_schema(cmd::text(&value, "schema_version")?)?;
    let mut config_keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "maker_type",
        "authority_ref",
        "expires_at",
        "source_context_ref",
        "source_path",
        "provenance_event_id",
        "allowed_operations",
        "allowed_claim_ids",
        "allowed_subject_refs",
        "allowed_object_refs",
        "allowed_predicates",
        "allowed_evidence_refs",
        "allowed_form_ids",
        "allowed_fields",
        "claim_selections",
    ];
    if version == ConfigVersion::V2 {
        config_keys.push("allowed_object_values");
    }
    cmd::exact_keys(&value, &config_keys)?;
    let source_path_text = cmd::text(&value, "source_path")?;
    let source_path = RelativePath::parse(source_path_text)
        .map_err(|_| SourceCommandError::Invalid("retained Claim source path"))?;
    owner_path_claim_stream(source_path_text, context)?;
    let private_prefix = cmd::text(context, "private_prefix")?;
    let tail = source_path_text
        .strip_prefix(private_prefix)
        .ok_or(SourceCommandError::Denied("retained Claim path prefix"))?;
    let scope = tail
        .split('/')
        .nth(1)
        .ok_or(SourceCommandError::Invalid("retained Claim scope"))?;
    let selections_raw = cmd::array(&value, "claim_selections")?;
    let mut selections = BTreeMap::new();
    for row in selections_raw {
        let selection = parse_selection(row, true)?;
        if selections
            .insert(selection.claim_id.clone(), selection)
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "retained Claim selection repeats",
            ));
        }
    }
    let home = format!("{private_prefix}claims/{scope}/");
    Ok(Grant {
        raw: raw.to_vec(),
        value,
        version,
        selections,
        digest: String::new(),
        context_snapshot: String::new(),
        source_path,
        home,
    })
}

fn validate_request_shape(request: &JsonValue) -> SourceCommandResult<&str> {
    let operation = cmd::text(request, "operation")?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "private Claim request version",
        ));
    }
    let commit = [
        "command_id",
        "expected_configuration",
        "expected_source",
        "expected_revision",
        "expected_dependencies",
        "expected_inputs",
    ];
    match operation {
        "describe" => cmd::exact_keys(request, &["schema_version", "operation"])?,
        "prepare-create" => {
            cmd::exact_keys(request, &["schema_version", "operation", "claims", "forms"])?
        }
        "claims.create" => {
            let mut keys = vec!["schema_version", "operation", "claims", "forms"];
            keys.extend(commit);
            cmd::exact_keys(request, &keys)?;
        }
        "prepare-revise" => cmd::exact_keys(
            request,
            &[
                "schema_version",
                "operation",
                "claim_id",
                "fields",
                "forms",
                "reason",
            ],
        )?,
        "claim.revise" => {
            let mut keys = vec![
                "schema_version",
                "operation",
                "claim_id",
                "fields",
                "forms",
                "reason",
            ];
            keys.extend(commit);
            cmd::exact_keys(request, &keys)?;
        }
        "prepare" => cmd::exact_keys(
            request,
            &[
                "schema_version",
                "operation",
                "claim_id",
                "form_id",
                "field_id",
            ],
        )?,
        "apply" => {
            let mut keys = vec!["schema_version", "operation", "claim_id", "changes"];
            keys.extend(commit);
            cmd::exact_keys(request, &keys)?;
        }
        "inspect-version" => cmd::exact_keys(
            request,
            &["schema_version", "operation", "claim_id", "source"],
        )?,
        _ => return Err(SourceCommandError::Unsupported("private Claim operation")),
    }
    if matches!(operation, "claims.create" | "claim.revise" | "apply") {
        let command = cmd::text(request, "command_id")?;
        let count = command.chars().count();
        if !(1..=256).contains(&count) {
            return Err(SourceCommandError::Invalid(
                "private Claim command identity",
            ));
        }
        if !cmd::nonblank(cmd::text(request, "expected_configuration")?)
            || !cmd::nonblank(cmd::text(request, "expected_dependencies")?)
        {
            return Err(SourceCommandError::Invalid(
                "private Claim commit preconditions",
            ));
        }
    }
    if operation == "prepare-revise" || operation == "claim.revise" {
        let reason = cmd::text(request, "reason")?;
        let stripped = tos_foundation::python_strip_unicode16_v1(reason, reason.chars().count())
            .map_err(|_| SourceCommandError::Invalid("private Claim revision reason"))?;
        if !(1..=4096).contains(&stripped.chars().count()) {
            return Err(SourceCommandError::Invalid("private Claim revision reason"));
        }
        if cmd::field(request, "fields")?
            .as_object()
            .is_none_or(|fields| fields.is_empty())
        {
            return Err(SourceCommandError::Invalid("private Claim revision fields"));
        }
        if cmd::array(request, "forms")?.is_empty() || cmd::array(request, "forms")?.len() > 32 {
            return Err(SourceCommandError::Invalid(
                "private Claim revision form selections",
            ));
        }
    }
    Ok(operation)
}

fn response_base(grant: &Grant, target_exists: bool) -> SourceCommandResult<JsonValue> {
    let mut result = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_command_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&grant.digest)),
        ("source_path", cmd::string(grant.source_path.as_str())),
        ("target_exists", JsonValue::Bool(target_exists)),
        (
            "supported_operations",
            JsonValue::Array(
                [
                    "claims.create",
                    "claim.revise",
                    "form.create",
                    "form.revise",
                ]
                .into_iter()
                .map(cmd::string)
                .collect(),
            ),
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
                    "claims.create",
                    "prepare-revise",
                    "claim.revise",
                    "prepare",
                    "apply",
                    "inspect-version",
                ]
                .into_iter()
                .map(cmd::string)
                .collect(),
            ),
        ),
        (
            "allowed_claim_ids",
            cmd::field(&grant.value, "allowed_claim_ids")?.clone(),
        ),
        (
            "allowed_form_ids",
            cmd::field(&grant.value, "allowed_form_ids")?.clone(),
        ),
        (
            "allowed_form_field_ids",
            JsonValue::Array(vec![cmd::string("claim.statement")]),
        ),
        (
            "allowed_subject_refs",
            cmd::field(&grant.value, "allowed_subject_refs")?.clone(),
        ),
        (
            "allowed_object_refs",
            cmd::field(&grant.value, "allowed_object_refs")?.clone(),
        ),
        (
            "allowed_predicates",
            cmd::field(&grant.value, "allowed_predicates")?.clone(),
        ),
        (
            "allowed_evidence_refs",
            cmd::field(&grant.value, "allowed_evidence_refs")?.clone(),
        ),
        (
            "allowed_fields",
            cmd::field(&grant.value, "allowed_fields")?.clone(),
        ),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
        ("receipt", JsonValue::Null),
        ("replayed", JsonValue::Bool(false)),
        ("replay_input_posture", JsonValue::Null),
        ("sources", JsonValue::Array(Vec::new())),
        ("source", JsonValue::Null),
        ("revision", JsonValue::Null),
        ("source_fields", JsonValue::Array(Vec::new())),
        ("materializations", JsonValue::Array(Vec::new())),
    ]);
    if let Some(values) = grant.value.object_get("allowed_object_values") {
        cmd::set(&mut result, "allowed_object_values", values.clone())?;
    }
    Ok(result)
}

fn response_state(
    mut response: JsonValue,
    files: &PrivatePackage,
    records: &BTreeMap<String, JsonValue>,
    forms: &BTreeMap<String, JsonValue>,
    grounded: &GroundedClaims,
    selected: Option<&str>,
) -> SourceCommandResult<JsonValue> {
    let refs = records
        .values()
        .map(record_ref)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    cmd::set(&mut response, "sources", JsonValue::Array(refs))?;
    if let Some(identity) = selected {
        let record = records.get(identity).ok_or(SourceCommandError::Conflict(
            "private Claim selected source is absent",
        ))?;
        cmd::set(&mut response, "source", record_ref(record)?)?;
    }
    cmd::set(
        &mut response,
        "revision",
        cmd::string(&package_revision(files)?),
    )?;
    cmd::set(
        &mut response,
        "expected_dependencies",
        cmd::string(&grounded.dependencies),
    )?;
    cmd::set(
        &mut response,
        "source_bindings",
        JsonValue::Array(grounded.bindings.clone()),
    )?;
    let fields = grounded
        .source_fields
        .iter()
        .filter(|field| {
            selected.is_none_or(|identity| {
                field.object_get("claim_id").and_then(JsonValue::as_str) == Some(identity)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    cmd::set(&mut response, "source_fields", JsonValue::Array(fields))?;
    let mut materializations = Vec::new();
    for (identity, record) in records {
        if selected.is_some_and(|chosen| chosen != identity) {
            continue;
        }
        let payload = forms.get(identity).ok_or(SourceCommandError::Conflict(
            "private Claim form package absent",
        ))?;
        materializations.extend(private_claim_materializations(record, payload)?);
    }
    cmd::set(
        &mut response,
        "materializations",
        JsonValue::Array(materializations),
    )?;
    Ok(response)
}

fn retained_form_refs(payload: &JsonValue) -> SourceCommandResult<Vec<JsonValue>> {
    optional_array(payload, "forms")?
        .iter()
        .chain(optional_array(payload, "prior_forms")?)
        .map(|form| {
            source_forms::form_reference(form)
                .map_err(|_| SourceCommandError::Invalid("retained Claim form reference"))
        })
        .collect()
}

fn validate_receipt_ref_files(
    receipt: &JsonValue,
    expected_files: &PrivatePackage,
) -> SourceCommandResult<()> {
    let expected = crate::source_revisions::file_refs(expected_files, false);
    if !same_json(cmd::field(receipt, "files")?, &expected)? {
        return Err(SourceCommandError::Conflict(
            "private Claim creation receipt file closure differs",
        ));
    }
    Ok(())
}

fn verify_creation_integrity(
    ctx: &CommandContext,
    grant: &Grant,
    owner: &OwnerTextContext,
    cut: &CorpusCutReader,
    context: &JsonValue,
    current_files: &PrivatePackage,
    state: &PackageState,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceFile>> {
    let receipt = &state.receipt;
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
            "dependencies",
            "source_bindings",
            "claims",
            "files",
            "grants_admission",
        ],
    )?;
    let request_raw = required_package_file(current_files, CREATE_REQUEST)?;
    let request = cmd::parse(request_raw)?;
    let retained_raw = required_package_file(current_files, CONFIG_FILE)?;
    let retained = cmd::parse(retained_raw)?;
    let retained_grant = historical_grant(retained_raw, context)?;
    let request_expected = encode(&request)?;
    if cmd::text(receipt, "schema_version")? != "tos_local_claim_create_receipt_v1"
        || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
        || cmd::text(&retained, "source_path")? != cmd::text(&grant.value, "source_path")?
        || !exact_list(&retained, "allowed_operations", 4)?.contains(&"claims.create")
        || request_expected != request_raw
        || cmd::text(&request, "operation")? != "claims.create"
        || cmd::text(&request, "command_id")? != cmd::text(receipt, "command_id")?
        || cmd::text(receipt, "request_digest")? != &cmd::record_digest(&request)?.to_prefixed()
        || cmd::text(receipt, "source_path")? != cmd::text(&grant.value, "source_path")?
        || cmd::text(receipt, "principal_id")? != cmd::text(&retained, "principal_id")?
        || cmd::text(receipt, "authority_ref")? != cmd::text(&retained, "authority_ref")?
        || cmd::field(receipt, "owner_configuration")?
            != cmd::field(&request, "expected_configuration")?
        || !cmd::field(&request, "expected_source")?.is_null()
        || !cmd::field(&request, "expected_revision")?.is_null()
        || cmd::field(receipt, "dependencies")? != cmd::field(&request, "expected_dependencies")?
        || cmd::field(receipt, "source_bindings")? != cmd::field(&request, "expected_inputs")?
    {
        return Err(SourceCommandError::Conflict(
            "private Claim creation receipt differs from its exact retained request",
        ));
    }
    crate::source_command::validate_instant(cmd::text(receipt, "recorded_at")?)?;
    validate_request_shape(&request)?;
    let claims = cmd::array(&request, "claims")?.to_vec();
    let ids = claims
        .iter()
        .map(|claim| cmd::text(claim, "claim_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if ids.len() != claims.len()
        || ids != state.records.keys().cloned().collect()
        || cmd::array(receipt, "claims")?.len() != claims.len()
    {
        return Err(SourceCommandError::Conflict(
            "private Claim creation subject closure changed",
        ));
    }
    let mut integrity_reads = ExactReads::default();
    let grammar = ClaimGrammar::load(
        ctx,
        owner,
        cut,
        context,
        worker,
        &mut integrity_reads,
        deadline,
        cancelled,
    )?;
    validate_allowed_predicates(&retained_grant, &grammar)?;
    for claim in &claims {
        let identity = cmd::text(claim, "claim_id")?;
        let selection =
            retained_grant
                .selections
                .get(identity)
                .ok_or(SourceCommandError::Conflict(
                    "retained Claim creation selection is absent",
                ))?;
        let route = claim_profile_route(&grammar, &retained_grant.value, selection, claim)?;
        validate_claim_scope(&retained_grant, &route, claim, &ids, true)?;
        check_claim_schema(
            ctx,
            owner,
            cut,
            context,
            &grammar,
            claim,
            &route,
            worker,
            &mut integrity_reads,
            deadline,
            cancelled,
        )?;
    }
    for (record, reference) in claims.iter().zip(cmd::array(receipt, "claims")?) {
        if !same_json(reference, &record_ref(record)?)? {
            return Err(SourceCommandError::Conflict(
                "private Claim creation receipt subject differs",
            ));
        }
    }
    let (created, _views) = creation_files(
        ctx,
        &retained_grant,
        owner,
        cut,
        context,
        worker,
        &claims,
        cmd::array(&request, "forms")?,
        deadline,
        cancelled,
    )?;
    let mut initial_files = created;
    initial_files.insert(CREATE_REQUEST.to_owned(), request_expected);
    initial_files.insert(
        CREATE_ENVIRONMENT.to_owned(),
        required_package_file(current_files, CREATE_ENVIRONMENT)?.to_vec(),
    );
    initial_files.insert(
        CREATE_PROVENANCE.to_owned(),
        required_package_file(current_files, CREATE_PROVENANCE)?.to_vec(),
    );
    validate_receipt_ref_files(receipt, &initial_files)?;
    let expected_initial_revision = match &state.initial_archive_revision {
        Some(revision) => revision.clone(),
        None => package_revision(current_files)?,
    };
    initial_files.insert(
        RECEIPT_FILE.to_owned(),
        required_package_file(current_files, RECEIPT_FILE)?.to_vec(),
    );
    if package_revision(&initial_files)? != expected_initial_revision
        || required_package_file(current_files, CONFIG_FILE)?
            != required_package_file(&initial_files, CONFIG_FILE)?
    {
        return Err(SourceCommandError::Conflict(
            "private Claim initial stream or configuration changed",
        ));
    }
    for (identity, payload) in &state.forms {
        let original = cmd::parse(required_package_file(
            &initial_files,
            &claim_form_filename(identity),
        )?)?;
        let first = optional_array(&original, "forms")?;
        let retained_refs = retained_form_refs(payload)?;
        for form in first {
            let reference = source_forms::form_reference(form)
                .map_err(|_| SourceCommandError::Invalid("initial private Claim form ref"))?;
            if !contains_same_json(&retained_refs, &reference)? {
                return Err(SourceCommandError::Conflict(
                    "private Claim initial form is no longer retained",
                ));
            }
        }
    }
    integrity_reads.into_source_files()
}

fn required_package_file<'a>(
    files: &'a PrivatePackage,
    name: &str,
) -> SourceCommandResult<&'a [u8]> {
    files
        .get(name)
        .map(Vec::as_slice)
        .ok_or(SourceCommandError::Conflict(
            "private Claim package evidence is incomplete",
        ))
}

fn package_state(
    ctx: &CommandContext,
    grant: &Grant,
    context: &JsonValue,
    files: &PrivatePackage,
    archives: &mut dyn PrivateArchiveReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PackageState> {
    if files.len() > MAX_PACKAGE_FILES
        || files.values().map(Vec::len).sum::<usize>() > MAX_PACKAGE_BYTES
    {
        return Err(SourceCommandError::Invalid(
            "private Claim package size budget",
        ));
    }
    for name in [
        CLAIM_STREAM,
        CONFIG_FILE,
        CREATE_REQUEST,
        CREATE_ENVIRONMENT,
        CREATE_PROVENANCE,
        RECEIPT_FILE,
    ] {
        required_package_file(files, name)?;
    }
    let records = parse_claims(required_package_file(files, CLAIM_STREAM)?)?;
    if records.is_empty()
        || records.len() > 32
        || records.keys().any(|identity| {
            !exact_list(&grant.value, "allowed_claim_ids", 32)
                .is_ok_and(|allowed| allowed.contains(&identity.as_str()))
        })
    {
        return Err(SourceCommandError::Denied(
            "private Claim package contains an undelegated identity",
        ));
    }
    let form_names = records
        .keys()
        .map(|identity| (claim_form_filename(identity), identity.clone()))
        .collect::<BTreeMap<_, _>>();
    if files.keys().any(|name| {
        !BASE_PACKAGE_FILES.contains(&name.as_str())
            && name != HISTORY
            && !form_names.contains_key(name)
    }) || form_names.keys().any(|name| !files.contains_key(name))
    {
        return Err(SourceCommandError::Conflict(
            "private Claim package has unbound files",
        ));
    }
    let history = match files.get(HISTORY) {
        Some(raw) => cmd::parse(raw)?,
        None => cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_revision_history_v1"),
            ),
            (
                "source_path",
                cmd::string(cmd::text(&grant.value, "source_path")?),
            ),
            ("receipts", JsonValue::Array(Vec::new())),
        ]),
    };
    cmd::exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    if cmd::text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || cmd::text(&history, "source_path")? != cmd::text(&grant.value, "source_path")?
        || cmd::array(&history, "receipts")?.len() > MAX_REVISIONS
    {
        return Err(SourceCommandError::Conflict(
            "private Claim history identity differs",
        ));
    }
    let receipt = cmd::parse(required_package_file(files, RECEIPT_FILE)?)?;
    let mut forms = BTreeMap::new();
    let mut form_ids = BTreeSet::new();
    for (name, identity) in &form_names {
        let payload = cmd::parse(required_package_file(files, name)?)?;
        super::profile::validate_form_set(
            ctx,
            worker,
            cmd::text(&grant.value, "source_path")?,
            &payload,
            deadline,
            cancelled,
        )?;
        if cmd::text(cmd::field(&payload, "subject")?, "id")? != identity
            || !same_json(
                cmd::field(&payload, "subject")?,
                &record_ref(
                    records
                        .get(identity)
                        .ok_or(SourceCommandError::Invalid("Claim form subject absent"))?,
                )?,
            )?
        {
            return Err(SourceCommandError::Conflict(
                "private Claim form subject differs",
            ));
        }
        for form in optional_array(&payload, "forms")?
            .iter()
            .chain(optional_array(&payload, "prior_forms")?)
        {
            let id = cmd::text(form, "form_id")?;
            if !exact_list(&grant.value, "allowed_form_ids", 32)?.contains(&id)
                || !form_ids.insert(id.to_owned())
            {
                return Err(SourceCommandError::Denied(
                    "private Claim form identity is undelegated or shared by siblings",
                ));
            }
        }
        forms.insert(identity.clone(), payload);
    }
    if cmd::array(&history, "receipts")?.is_empty() {
        for record in records.values() {
            if cmd::integer(record, "claim_version")? != 1 {
                return Err(SourceCommandError::Conflict(
                    "private Claim stream has revisions without retained history",
                ));
            }
        }
    }
    let mut commands = BTreeSet::new();
    let create_command = cmd::text(&receipt, "command_id")?;
    commands.insert(create_command.to_owned());
    for retained in cmd::array(&history, "receipts")? {
        if !commands.insert(cmd::text(retained, "command_id")?.to_owned()) {
            return Err(SourceCommandError::Conflict(
                "private Claim command identity is repeated",
            ));
        }
        let identity = cmd::text(cmd::field(retained, "previous_source")?, "id")?;
        let payload = forms.get(identity).ok_or(SourceCommandError::Conflict(
            "retained Claim form package is absent",
        ))?;
        let refs = retained_form_refs(payload)?;
        for reference in cmd::array(retained, "forms")? {
            if !contains_same_json(&refs, reference)? {
                return Err(SourceCommandError::Conflict(
                    "retained Claim revision form is not retained",
                ));
            }
        }
    }
    for (identity, payload) in &forms {
        for receipt in optional_array(payload, "growth_history")? {
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
            let current_record = records
                .get(identity)
                .ok_or(SourceCommandError::Invalid("Claim form source absent"))?;
            let form_source = cmd::field(receipt, "source")?;
            if !commands.insert(cmd::text(receipt, "command_id")?.to_owned())
                || cmd::text(form_source, "id")? != identity
                || cmd::integer(form_source, "version")?
                    > cmd::integer(current_record, "claim_version")?
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim form history identity is repeated or stale",
                ));
            }
            crate::source_command::validate_instant(cmd::text(receipt, "recorded_at")?)?;
            if cmd::text(receipt, "request_digest")?
                .strip_prefix("sha256:")
                .is_none_or(|hash| {
                    hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            {
                return Err(SourceCommandError::Invalid(
                    "private Claim form request digest",
                ));
            }
        }
    }
    let mut archive_locations = Vec::new();
    let mut initial_archive_revision = None;
    let mut expected_stream_digest: Option<String> = None;
    let mut history_commands = BTreeSet::new();
    for receipt in cmd::array(&history, "receipts")? {
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
        validate_request_shape(request)?;
        let command = cmd::text(receipt, "command_id")?;
        if !history_commands.insert(command.to_owned())
            || cmd::text(request, "operation")? != "claim.revise"
            || cmd::text(request, "command_id")? != command
            || cmd::text(receipt, "request_digest")? != cmd::record_digest(request)?.to_prefixed()
            || !same_json(
                cmd::field(receipt, "previous_source")?,
                cmd::field(request, "expected_source")?,
            )?
            || cmd::field(receipt, "previous_revision")?
                != cmd::field(request, "expected_revision")?
            || cmd::field(receipt, "owner_configuration")?
                != cmd::field(request, "expected_configuration")?
            || cmd::field(receipt, "dependencies")? != cmd::field(request, "expected_dependencies")?
            || cmd::field(receipt, "source_bindings")? != cmd::field(request, "expected_inputs")?
            || cmd::field(receipt, "reason")? != cmd::field(request, "reason")?
            || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim revision receipt differs from retained request",
            ));
        }
        crate::source_command::validate_instant(cmd::text(receipt, "recorded_at")?)?;
        let (package, locations) =
            read_archive(grant, context, archives, receipt, deadline, cancelled)?;
        for name in [
            CONFIG_FILE,
            CREATE_REQUEST,
            CREATE_ENVIRONMENT,
            CREATE_PROVENANCE,
            RECEIPT_FILE,
        ] {
            if package.get(name) != files.get(name) {
                return Err(SourceCommandError::Conflict(
                    "private Claim immutable creation evidence changed",
                ));
            }
        }
        let previous = parse_claims(required_package_file(&package, CLAIM_STREAM)?)?;
        let identity = cmd::text(request, "claim_id")?;
        let prior = previous.get(identity).ok_or(SourceCommandError::Conflict(
            "private Claim archived predecessor is absent",
        ))?;
        if !same_json(cmd::field(receipt, "previous_source")?, &record_ref(prior)?)?
            || package_revision(&package)? != cmd::text(receipt, "previous_revision")?
        {
            return Err(SourceCommandError::Conflict(
                "private Claim revision predecessor differs",
            ));
        }
        let archived_stream = required_package_file(&package, CLAIM_STREAM)?;
        let archived_stream_digest = Digest256::of_bytes(archived_stream).to_prefixed();
        if let Some(expected) = &expected_stream_digest
            && &archived_stream_digest != expected
        {
            return Err(SourceCommandError::Conflict(
                "private Claim archived stream chain is broken",
            ));
        }
        if expected_stream_digest.is_none() {
            initial_archive_revision = Some(package_revision(&package)?);
            for row in previous.values() {
                if cmd::integer(row, "claim_version")? != 1 {
                    return Err(SourceCommandError::Conflict(
                        "private Claim history omits its initial stream",
                    ));
                }
            }
        }
        let revised = crate::source_claims::advance_claim(
            prior,
            cmd::field(request, "fields")?,
            request.object_get("layer_transition"),
        )?;
        if !same_json(cmd::field(receipt, "source")?, &record_ref(&revised)?)?
            || cmd::integer(&revised, "claim_version")?
                != cmd::integer(cmd::field(receipt, "previous_source")?, "version")?
                    .saturating_add(1)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim revision successor differs",
            ));
        }
        let changed = cmd::field(receipt, "changed_fields")?
            .as_array()
            .ok_or(SourceCommandError::Invalid("Claim changed field list"))?;
        let mut expected_fields = cmd::field(request, "fields")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("Claim revision field patch"))?
            .iter()
            .map(|(key, _)| key.as_str().unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        expected_fields.sort();
        if changed
            .iter()
            .map(JsonValue::as_str)
            .collect::<Option<Vec<_>>>()
            .is_none_or(|values| {
                values
                    != expected_fields
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
            })
        {
            return Err(SourceCommandError::Conflict(
                "private Claim changed-field receipt differs",
            ));
        }
        let before_payload = package
            .get(&claim_form_filename(identity))
            .map(|raw| cmd::parse(raw))
            .transpose()?;
        let selections = cmd::array(request, "forms")?;
        if selections.is_empty() || selections.len() > 32 {
            return Err(SourceCommandError::Conflict(
                "private Claim revision form rebindings differ",
            ));
        }
        let mut selection_ids = BTreeSet::new();
        for selection in selections {
            cmd::exact_keys(selection, &["form_id", "field_id"])?;
            let form_id = cmd::text(selection, "form_id")?;
            if !selection_ids.insert(form_id.to_owned()) {
                return Err(SourceCommandError::Conflict(
                    "retained Claim revision repeats a form identity",
                ));
            }
        }
        if let Some(payload) = before_payload.as_ref() {
            for form in optional_array(payload, "forms")? {
                if !selection_ids.contains(cmd::text(form, "form_id")?) {
                    return Err(SourceCommandError::Conflict(
                        "private Claim revision form rebindings differ",
                    ));
                }
            }
        }
        let mut changes = Vec::new();
        for selection in selections {
            cmd::exact_keys(selection, &["form_id", "field_id"])?;
            let form_id = cmd::text(selection, "form_id")?;
            let field_id = cmd::text(selection, "field_id")?;
            if field_id != "claim.statement" {
                return Err(SourceCommandError::Denied(
                    "retained Claim form field is outside its local grammar",
                ));
            }
            changes.push(
                source_forms::prepare_form_change(
                    &revised,
                    before_payload.as_ref(),
                    cmd::text(receipt, "principal_id")?,
                    form_id,
                    field_id,
                )
                .map_err(|_| {
                    SourceCommandError::Conflict(
                        "retained Claim form change cannot be reconstructed",
                    )
                })?,
            );
        }
        let refs = changes
            .iter()
            .map(|change| {
                source_forms::form_reference(cmd::field(change, "form")?)
                    .map_err(|_| SourceCommandError::Invalid("retained Claim form reference"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        if !same_json(cmd::field(receipt, "forms")?, &JsonValue::Array(refs))? {
            return Err(SourceCommandError::Conflict(
                "retained Claim form references differ",
            ));
        }
        let _successor_forms = source_forms::apply_form_changes(
            before_payload.as_ref(),
            &record_ref(&revised)?,
            &changes,
        )
        .map_err(|_| SourceCommandError::Conflict("retained Claim form successor differs"))?;
        let successor_stream = crate::source_claims::replace_claim_row(
            required_package_file(&package, CLAIM_STREAM)?,
            &revised,
        )?;
        expected_stream_digest = Some(Digest256::of_bytes(&successor_stream).to_prefixed());
        archive_locations.push(locations);
    }
    if let Some(expected) = &expected_stream_digest
        && Digest256::of_bytes(required_package_file(files, CLAIM_STREAM)?)
            .to_prefixed()
            .as_str()
            != expected.as_str()
    {
        return Err(SourceCommandError::Conflict(
            "private Claim current stream is not its retained history head",
        ));
    }
    Ok(PackageState {
        records,
        forms,
        history,
        receipt,
        initial_archive_revision,
        archive_locations,
    })
}

/// Exact archive paths the outer owner store may select before dispatch. The
/// semantic reader later verifies each manifest, payload and complete chain.
pub(crate) fn required_archives(
    current: &PrivatePackage,
    grant: &JsonValue,
    context_config: &JsonValue,
) -> SourceCommandResult<Vec<String>> {
    let Some(raw) = current.get(HISTORY) else {
        return Ok(Vec::new());
    };
    let history = cmd::parse(raw)?;
    cmd::exact_keys(&history, &["schema_version", "source_path", "receipts"])?;
    if cmd::text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || cmd::text(&history, "source_path")? != cmd::text(grant, "source_path")?
    {
        return Err(SourceCommandError::Conflict(
            "private Claim history identity differs",
        ));
    }
    let receipts = cmd::array(&history, "receipts")?;
    if receipts.len() > MAX_REVISIONS {
        return Err(SourceCommandError::Invalid(
            "private Claim history capacity",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut refs = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let id = cmd::text(cmd::field(receipt, "previous_source")?, "id")?;
        let revision = cmd::text(receipt, "previous_revision")?;
        let reference = archive_ref(id, revision, context_config)?;
        if cmd::text(receipt, "archive_path")? != reference || !seen.insert(reference.clone()) {
            return Err(SourceCommandError::Conflict(
                "private Claim archive path is not canonical",
            ));
        }
        refs.push(reference);
    }
    Ok(refs)
}

/// Build the profile-driven basename fence before selected private identity
/// inventory reads. The profile reader validates the public entity registry.
pub(crate) fn identity_basenames(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeSet<String>> {
    let mut basenames =
        super::profile::public_identity_basenames(ctx, cut, worker, deadline, cancelled)?;
    basenames.insert(CLAIM_STREAM.to_owned());
    Ok(basenames)
}

fn request_digest(request: &JsonValue) -> SourceCommandResult<String> {
    Ok(cmd::record_digest(request)?.to_prefixed())
}

fn request_commit_matches(
    request: &JsonValue,
    grant: &Grant,
    source: Option<&JsonValue>,
    revision: Option<&str>,
    dependencies: &str,
    bindings: &JsonValue,
) -> SourceCommandResult<()> {
    let expected_source = source.cloned().unwrap_or(JsonValue::Null);
    let expected_revision = revision.map(cmd::string).unwrap_or(JsonValue::Null);
    if cmd::text(request, "command_id")?.chars().count() == 0
        || cmd::text(request, "command_id")?.chars().count() > 256
        || cmd::text(request, "expected_configuration")? != grant.digest
        || !same_json(cmd::field(request, "expected_source")?, &expected_source)?
        || !same_json(
            cmd::field(request, "expected_revision")?,
            &expected_revision,
        )?
        || cmd::text(request, "expected_dependencies")? != dependencies
        || !same_json(cmd::field(request, "expected_inputs")?, bindings)?
    {
        return Err(SourceCommandError::Conflict(
            "private Claim source, configuration or dependencies are stale",
        ));
    }
    Ok(())
}

fn ensure_operation(grant: &Grant, operation: &str) -> SourceCommandResult<()> {
    if !exact_list(&grant.value, "allowed_operations", 4)?.contains(&operation) {
        return Err(SourceCommandError::Denied(
            "private Claim operation is not delegated",
        ));
    }
    Ok(())
}

fn validate_revision_request(
    grant: &Grant,
    request: &JsonValue,
    record: &JsonValue,
) -> SourceCommandResult<()> {
    ensure_operation(grant, "claim.revise")?;
    let fields = cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "private Claim revision field patch",
        ))?;
    if fields.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private Claim revision field patch",
        ));
    }
    for (key, value) in fields {
        let key = key
            .as_str()
            .ok_or(SourceCommandError::Invalid("Claim field name"))?;
        if !exact_list(&grant.value, "allowed_fields", 8)?.contains(&key) {
            return Err(SourceCommandError::Denied(
                "private Claim field is outside delegation",
            ));
        }
        match key {
            "evidence_refs" | "counterevidence_refs" => {
                let values = value
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("Claim evidence patch"))?;
                for reference in values {
                    allowed_text(
                        &grant.value,
                        "allowed_evidence_refs",
                        reference
                            .as_str()
                            .ok_or(SourceCommandError::Invalid("Claim evidence ref"))?,
                        128,
                    )?;
                }
            }
            "object" => {
                if grant.version != ConfigVersion::V2
                    || !record
                        .object_get("object")
                        .is_some_and(|old| old.as_object().is_some())
                    || value.as_object().is_none()
                {
                    return Err(SourceCommandError::Denied(
                        "private Claim object revision is outside v2 value scope",
                    ));
                }
                let allowed = cmd::array(&grant.value, "allowed_object_values")?;
                if !contains_same_json(allowed, value)? {
                    return Err(SourceCommandError::Denied(
                        "private Claim object value is not explicitly delegated",
                    ));
                }
            }
            _ => (),
        }
    }
    let reason = cmd::text(request, "reason")?;
    let stripped = tos_foundation::python_strip_unicode16_v1(reason, reason.chars().count())
        .map_err(|_| SourceCommandError::Invalid("private Claim revision reason"))?;
    if !(1..=4096).contains(&stripped.chars().count()) {
        return Err(SourceCommandError::Invalid("private Claim revision reason"));
    }
    let forms = cmd::array(request, "forms")?;
    if forms.is_empty() || forms.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "private Claim revision form selections",
        ));
    }
    let mut ids = BTreeSet::new();
    for selection in forms {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let id = cmd::text(selection, "form_id")?;
        if !form_id(id)
            || !exact_list(&grant.value, "allowed_form_ids", 32)?.contains(&id)
            || cmd::text(selection, "field_id")? != "claim.statement"
            || !ids.insert(id.to_owned())
        {
            return Err(SourceCommandError::Denied(
                "private Claim revision form scope",
            ));
        }
    }
    if COMPOUND_PREDICATES.contains(&cmd::text(record, "predicate")?) {
        return Err(SourceCommandError::Denied(
            "private Claim relation needs its stronger owner operation",
        ));
    }
    Ok(())
}

fn add_commit_result(
    response: &mut JsonValue,
    receipt: &JsonValue,
    replayed: bool,
) -> SourceCommandResult<()> {
    cmd::set(response, "receipt", receipt.clone())?;
    cmd::set(response, "replayed", JsonValue::Bool(replayed))?;
    cmd::set(
        response,
        "replay_input_posture",
        if replayed {
            cmd::string("historical_request_current_validation")
        } else {
            JsonValue::Null
        },
    )?;
    Ok(())
}

fn parse_record_map(rows: &[JsonValue]) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    let mut records = BTreeMap::new();
    for row in rows {
        let identity = cmd::text(row, "claim_id")?.to_owned();
        if records.insert(identity, row.clone()).is_some() {
            return Err(SourceCommandError::Conflict(
                "private Claim identity repeats",
            ));
        }
    }
    Ok(records)
}

fn append_history(
    files: &mut PrivatePackage,
    grant: &Grant,
    receipt: &JsonValue,
) -> SourceCommandResult<()> {
    let mut history = match files.get(HISTORY) {
        Some(raw) => cmd::parse(raw)?,
        None => cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_revision_history_v1"),
            ),
            ("source_path", cmd::string(grant.source_path.as_str())),
            ("receipts", JsonValue::Array(Vec::new())),
        ]),
    };
    let mut receipts = cmd::array(&history, "receipts")?.to_vec();
    if receipts.len() >= MAX_REVISIONS {
        return Err(SourceCommandError::Invalid(
            "private Claim revision history capacity",
        ));
    }
    receipts.push(receipt.clone());
    cmd::set(&mut history, "receipts", JsonValue::Array(receipts))?;
    files.insert(HISTORY.to_owned(), cmd::published(&history)?);
    Ok(())
}

fn serialized_package_refs(files: &PrivatePackage) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    tos_foundation::JsonString::from_utf8(name),
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

fn private_input_reads(
    grounded: &GroundedClaims,
    other: &[SourceFile],
) -> SourceCommandResult<Vec<SourceFile>> {
    let mut values = BTreeMap::new();
    for file in grounded.reads.iter().chain(other.iter()) {
        let key = file.path.as_str().to_owned();
        if let Some(previous) = values.insert(key, file.raw.clone())
            && previous != file.raw
        {
            return Err(SourceCommandError::Conflict(
                "private Claim exact read set changed",
            ));
        }
    }
    values
        .into_iter()
        .map(|(path, raw)| {
            Ok(SourceFile {
                path: RelativePath::parse(&path)
                    .map_err(|_| SourceCommandError::Invalid("private Claim read path"))?,
                raw,
            })
        })
        .collect()
}

fn create_plan(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &mut Grant,
    request: &JsonValue,
    inventory: &PrivateIdentityInputs,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    commit: bool,
) -> SourceCommandResult<(Option<PrivatePackage>, JsonValue, Vec<SourceFile>)> {
    ensure_operation(grant, "claims.create")?;
    let claims = cmd::array(request, "claims")?;
    if claims.is_empty() || claims.len() > 32 {
        return Err(SourceCommandError::Invalid("private Claim batch size"));
    }
    let identities = claims
        .iter()
        .map(|row| cmd::text(row, "claim_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if identities != grant.selections.keys().cloned().collect() {
        return Err(SourceCommandError::Denied(
            "private Claim batch does not match exact delegated identities",
        ));
    }
    let grounded = ground_claims(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        grant,
        claims,
        inventory,
        worker,
        deadline,
        cancelled,
        true,
    )?;
    let (mut files, views) = creation_files(
        ctx,
        grant,
        owner,
        cut,
        context_config,
        worker,
        claims,
        cmd::array(request, "forms")?,
        deadline,
        cancelled,
    )?;
    if !commit {
        let mut response = response_base(grant, false)?;
        cmd::set(
            &mut response,
            "prepared_sources",
            JsonValue::Array(
                claims
                    .iter()
                    .map(record_ref)
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        )?;
        cmd::set(
            &mut response,
            "prepared_files",
            serialized_package_refs(&files),
        )?;
        cmd::set(
            &mut response,
            "prepared_materializations",
            JsonValue::Array(views),
        )?;
        cmd::set(&mut response, "expected_source", JsonValue::Null)?;
        cmd::set(&mut response, "expected_revision", JsonValue::Null)?;
        cmd::set(
            &mut response,
            "expected_dependencies",
            cmd::string(&grounded.dependencies),
        )?;
        cmd::set(
            &mut response,
            "source_bindings",
            JsonValue::Array(grounded.bindings.clone()),
        )?;
        return Ok((Some(files), response, grounded.reads));
    }
    request_commit_matches(
        request,
        grant,
        None,
        None,
        &grounded.dependencies,
        &JsonValue::Array(grounded.bindings.clone()),
    )?;
    source_serialization::capture_private_metadata(
        PrivateMetadataFamily::Claim,
        request,
        cmd::text(&grant.value, "provenance_event_id")?,
        &grant.home,
        &mut files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    // The maintained creation writer inserts the stream, configuration, then
    // claim forms in request order, followed by these three capture byproducts.
    // Package storage remains sorted; only this published receipt owns that order.
    let mut file_order = vec![
        grant.source_path.as_str().rsplit('/').next().ok_or(
            SourceCommandError::Invalid("Claim stream basename"),
        )?.to_owned(),
        CONFIG_FILE.to_owned(),
    ];
    for claim in claims {
        file_order.push(claim_form_filename(cmd::text(claim, "claim_id")?));
    }
    file_order.extend([
        "source-create-request.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
    ]);
    if file_order.len() != files.len()
        || file_order.iter().collect::<BTreeSet<_>>() != files.keys().collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Invalid("private Claim creation receipt file order"));
    }
    let unordered_refs = serialized_package_refs(&files);
    let receipt_files = JsonValue::Object(
        file_order.iter().map(|name| {
            Ok((tos_foundation::JsonString::from_utf8(name), cmd::field(&unordered_refs, name)?.clone()))
        }).collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let receipt = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_claim_create_receipt_v1"),
        ),
        ("command_id", cmd::field(request, "command_id")?.clone()),
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
        ("dependencies", cmd::string(&grounded.dependencies)),
        (
            "source_bindings",
            JsonValue::Array(grounded.bindings.clone()),
        ),
        (
            "claims",
            JsonValue::Array(
                claims
                    .iter()
                    .map(record_ref)
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        ),
        ("files", receipt_files),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    files.insert(RECEIPT_FILE.to_owned(), cmd::published(&receipt)?);
    if files.len() > MAX_PACKAGE_FILES
        || files.values().map(Vec::len).sum::<usize>() > MAX_PACKAGE_BYTES
    {
        return Err(SourceCommandError::Invalid(
            "private Claim created package budget",
        ));
    }
    let records = parse_record_map(claims)?;
    let forms = records
        .keys()
        .map(|identity| {
            let name = claim_form_filename(identity);
            let payload = cmd::parse(required_package_file(&files, &name)?)?;
            Ok((identity.clone(), payload))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    let mut response = response_base(grant, true)?;
    response = response_state(response, &files, &records, &forms, &grounded, None)?;
    add_commit_result(&mut response, &receipt, false)?;
    Ok((Some(files), response, grounded.reads))
}

fn prepare_revision(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    owner: &OwnerTextContext,
    context_config: &JsonValue,
    grant: &mut Grant,
    request: &JsonValue,
    state: &PackageState,
    current: &PrivatePackage,
    inventory: &PrivateIdentityInputs,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    JsonValue,
    PrivatePackage,
    GroundedClaims,
    Vec<JsonValue>,
    Vec<JsonValue>,
)> {
    let identity = cmd::text(request, "claim_id")?;
    let previous = state
        .records
        .get(identity)
        .ok_or(SourceCommandError::Conflict("private Claim is absent"))?;
    validate_revision_request(grant, request, previous)?;
    if cmd::array(&state.history, "receipts")?.len() >= MAX_REVISIONS {
        return Err(SourceCommandError::Invalid(
            "private Claim revision history capacity",
        ));
    }
    let revised = crate::source_claims::advance_claim(
        previous,
        cmd::field(request, "fields")?,
        request.object_get("layer_transition"),
    )?;
    let records = state
        .records
        .iter()
        .map(|(key, value)| {
            if key == identity {
                revised.clone()
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>();
    let grounded = ground_claims(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        grant,
        &records,
        inventory,
        worker,
        deadline,
        cancelled,
        false,
    )?;
    let before_forms = state
        .forms
        .get(identity)
        .ok_or(SourceCommandError::Conflict(
            "private Claim forms are absent",
        ))?;
    let prepared = prepare_claim_forms(
        ctx,
        grant,
        owner,
        cut,
        context_config,
        worker,
        &revised,
        Some(before_forms),
        cmd::array(request, "forms")?,
        true,
        deadline,
        cancelled,
    )?;
    let next_ids = cmd::array(&prepared.payload, "forms")?
        .iter()
        .map(|form| cmd::text(form, "form_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    for (other_id, other) in &state.forms {
        if other_id == identity {
            continue;
        }
        let other_ids = optional_array(other, "forms")?
            .iter()
            .chain(optional_array(other, "prior_forms")?)
            .map(|form| cmd::text(form, "form_id").map(str::to_owned))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        if next_ids.iter().any(|id| other_ids.contains(id)) {
            return Err(SourceCommandError::Conflict(
                "private Claim revision reuses sibling form identity",
            ));
        }
    }
    let mut output = current.clone();
    output.insert(
        CLAIM_STREAM.to_owned(),
        crate::source_claims::replace_claim_row(
            required_package_file(current, CLAIM_STREAM)?,
            &revised,
        )?,
    );
    output.insert(claim_form_filename(identity), cmd::published(&prepared.payload)?);
    let references = prepared.references.clone();
    let views = prepared.views.clone();
    Ok((revised, output, grounded, references, views))
}

fn initialize_grant_digest(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    owner: &OwnerTextContext,
    context: &JsonValue,
    grant: &mut Grant,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceFile>> {
    let mut reads = ExactReads::default();
    let grammar = ClaimGrammar::load(
        ctx, owner, cut, context, worker, &mut reads, deadline, cancelled,
    )?;
    validate_allowed_predicates(grant, &grammar)?;
    let _form_grammar = super::profile::form_grammar_digests(ctx, worker, deadline, cancelled)?;
    for reference in CLAIM_SHARED_SCHEMAS {
        let raw = selected_authored(
            ctx, owner, cut, context, reference, 1_048_576, &mut reads, deadline, cancelled,
        )?;
        let schema = cmd::parse(&raw)?;
        if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{reference}")
            && cmd::text(&schema, "$id")? != format!("https://treeofsophia.local/{reference}")
        {
            return Err(SourceCommandError::Conflict(
                "Claim shared schema identity differs",
            ));
        }
        require_contract_digest(worker, reference, &raw)?;
    }
    let context_schema = selected_authored(
        ctx,
        owner,
        cut,
        context,
        CONTEXT_SCHEMA_REF,
        1_048_576,
        &mut reads,
        deadline,
        cancelled,
    )?;
    require_contract_digest(worker, CONTEXT_SCHEMA_REF, &context_schema)?;
    let provenance = selected_authored(
        ctx,
        owner,
        cut,
        context,
        PROVENANCE_SCHEMA,
        1_048_576,
        &mut reads,
        deadline,
        cancelled,
    )?;
    let provenance_value = cmd::parse(&provenance)?;
    if cmd::text(&provenance_value, "$id")?
        != format!("https://tree-of-sophia.local/{PROVENANCE_SCHEMA}")
        && cmd::text(&provenance_value, "$id")?
            != format!("https://treeofsophia.local/{PROVENANCE_SCHEMA}")
    {
        return Err(SourceCommandError::Conflict(
            "private Claim provenance schema identity",
        ));
    }
    require_contract_digest(worker, PROVENANCE_SCHEMA, &provenance)?;
    // The Python reader constructors preload every declared Claim route and
    // each selected source-record route. Recheck those fixed routes here;
    // selectors cannot add a schema or type alias.
    for route in grammar.routes.values() {
        for route_row in cmd::array(&route.profile, "schemas")? {
            let root = cmd::text(route_row, "schema_ref")?;
            let mut refs = field_texts(route_row, "schema_dependencies", 128)?;
            refs.push(root.to_owned());
            for reference in refs {
                let raw = selected_authored(
                    ctx, owner, cut, context, &reference, 1_048_576, &mut reads, deadline,
                    cancelled,
                )?;
                let schema = cmd::parse(&raw)?;
                if cmd::text(&schema, "$id")? != format!("https://tree-of-sophia.local/{reference}")
                    && cmd::text(&schema, "$id")?
                        != format!("https://treeofsophia.local/{reference}")
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim relation route schema identity",
                    ));
                }
                require_contract_digest(worker, &reference, &raw)?;
            }
        }
    }
    for selection in grant.selections.values() {
        for selector in &selection.source_records {
            let type_id = cmd::text(selector, "profile_type_id")?;
            let Some(profile) = grammar.profile_by_type.get(type_id) else {
                let kind =
                    type_id
                        .strip_prefix("tos.entity.")
                        .ok_or(SourceCommandError::Unsupported(
                            "private Claim selected source profile",
                        ))?;
                if !NATIVE_CORPUS_KINDS.contains(&kind) {
                    return Err(SourceCommandError::Unsupported(
                        "private Claim selected source profile",
                    ));
                }
                let raw = selected_authored(
                    ctx,
                    owner,
                    cut,
                    context,
                    CORPUS_SCHEMA,
                    1_048_576,
                    &mut reads,
                    deadline,
                    cancelled,
                )?;
                require_contract_digest(worker, CORPUS_SCHEMA, &raw)?;
                continue;
            };
            for schema_route in cmd::array(profile, "schemas")? {
                let mut refs = field_texts(schema_route, "schema_dependencies", 128)?;
                refs.push(cmd::text(schema_route, "schema_ref")?.to_owned());
                refs.push(CORPUS_SCHEMA.to_owned());
                for reference in refs {
                    let raw = selected_authored(
                        ctx, owner, cut, context, &reference, 1_048_576, &mut reads, deadline,
                        cancelled,
                    )?;
                    let schema = cmd::parse(&raw)?;
                    if cmd::text(&schema, "$id")?
                        != format!("https://tree-of-sophia.local/{reference}")
                        && cmd::text(&schema, "$id")?
                            != format!("https://treeofsophia.local/{reference}")
                    {
                        return Err(SourceCommandError::Conflict(
                            "private Claim source profile schema identity",
                        ));
                    }
                    require_contract_digest(worker, &reference, &raw)?;
                }
            }
        }
        if selection.native_bindings.iter().any(|_| true)
            || selection
                .source_records
                .iter()
                .any(|row| cmd::field(row, "source_binding").is_ok_and(|value| !value.is_null()))
        {
            let reference = "ToS/contracts/native-text-unit-binding.schema.json";
            let raw = selected_authored(
                ctx, owner, cut, context, reference, 1_048_576, &mut reads, deadline, cancelled,
            )?;
            require_contract_digest(worker, reference, &raw)?;
        }
    }
    for selection in grant.selections.values() {
        if grammar
            .routes
            .values()
            .filter(|route| route.relation_type_id == selection.relation_type_id)
            .count()
            != 1
        {
            return Err(SourceCommandError::Unsupported(
                "private Claim relation selection is ambiguous",
            ));
        }
    }
    let configuration_grammars = configuration_grammars(
        ctx, owner, cut, context, grant, &mut reads, worker, deadline, cancelled,
    )?;
    let basis = cmd::object(vec![
        (
            "configuration_bytes",
            cmd::string(&Digest256::of_bytes(&grant.raw).to_prefixed()),
        ),
        ("context", cmd::string(&grant.context_snapshot)),
        ("grammars", configuration_grammars),
    ]);
    grant.digest = cmd::record_digest(&basis)?.to_prefixed();
    reads.into_source_files()
}

fn claim_command_receipt<'a>(
    state: &'a PackageState,
    command_id: &str,
) -> SourceCommandResult<Option<(&'a str, &'a str, &'a JsonValue)>> {
    if cmd::text(&state.receipt, "command_id")? == command_id {
        return Ok(Some(("claims.create", "", &state.receipt)));
    }
    for receipt in cmd::array(&state.history, "receipts")? {
        if cmd::text(receipt, "command_id")? == command_id {
            return Ok(Some((
                "claim.revise",
                cmd::text(cmd::field(receipt, "previous_source")?, "id")?,
                receipt,
            )));
        }
    }
    for (identity, payload) in &state.forms {
        for receipt in optional_array(payload, "growth_history")? {
            if cmd::text(receipt, "command_id")? == command_id {
                return Ok(Some(("apply", identity, receipt)));
            }
        }
    }
    Ok(None)
}

/// Prepare or execute one owner-local Claim operation. The owner transport
/// remains the only writer; this function returns bounded proposal bytes.
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
) -> SourceCommandResult<PrivateClaimPlan> {
    crate::source_creation_store::active(deadline, cancelled)?;
    let mut grant = validate_grant(ctx, owner, context_config, deadline, cancelled)?;
    let request = cmd::parse(&ctx.request_raw)?;
    let operation = validate_request_shape(&request)?;
    let before = current.cloned();

    if current.is_none() {
        if operation == "describe" {
            let reads = initialize_grant_digest(
                ctx,
                cut,
                owner,
                context_config,
                &mut grant,
                worker,
                deadline,
                cancelled,
            )?;
            return Ok(PrivateClaimPlan {
                before,
                files: None,
                response: response_base(&grant, false)?,
                archive: None,
                reads,
            });
        }
        if operation == "prepare-create" || operation == "claims.create" {
            let (files, response, reads) = create_plan(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &mut grant,
                &request,
                inventory,
                worker,
                deadline,
                cancelled,
                operation == "claims.create",
            )?;
            return Ok(PrivateClaimPlan {
                before,
                files: if operation == "claims.create" {
                    files
                } else {
                    None
                },
                response,
                archive: None,
                reads,
            });
        }
        return Err(SourceCommandError::Conflict(
            "private Claim target package is absent",
        ));
    }

    let package = current.ok_or(SourceCommandError::Conflict(
        "private Claim target package is absent",
    ))?;
    let _archive_refs = required_archives(package, &grant.value, context_config)?;
    let mut state = package_state(
        ctx,
        &grant,
        context_config,
        package,
        archive_reader,
        worker,
        deadline,
        cancelled,
    )?;
    let creation_reads = verify_creation_integrity(
        ctx,
        &grant,
        owner,
        cut,
        context_config,
        package,
        &state,
        worker,
        deadline,
        cancelled,
    )?;
    let current_records = state.records.values().cloned().collect::<Vec<_>>();
    let mut grounded = ground_claims(
        ctx,
        cut,
        software,
        components,
        owner,
        context_config,
        &mut grant,
        &current_records,
        inventory,
        worker,
        deadline,
        cancelled,
        false,
    )?;
    grounded.reads = private_input_reads(&grounded, &creation_reads)?;
    let mut response = response_base(&grant, true)?;
    let selected_identity = request.object_get("claim_id").and_then(JsonValue::as_str);
    response = response_state(
        response,
        package,
        &state.records,
        &state.forms,
        &grounded,
        selected_identity,
    )?;

    match operation {
        "describe" => Ok(PrivateClaimPlan {
            before,
            files: None,
            response,
            archive: None,
            reads: grounded.reads,
        }),
        "claims.create" => {
            let command = cmd::text(&request, "command_id")?;
            let receipt = &state.receipt;
            if cmd::text(receipt, "command_id")? != command
                || cmd::text(receipt, "request_digest")? != request_digest(&request)?
                || cmd::text(receipt, "owner_configuration")? != grant.digest
                || cmd::text(&request, "expected_configuration")? != grant.digest
                || cmd::field(receipt, "dependencies")?
                    != cmd::field(&request, "expected_dependencies")?
                || cmd::field(receipt, "source_bindings")?
                    != cmd::field(&request, "expected_inputs")?
                || !cmd::field(&request, "expected_source")?.is_null()
                || !cmd::field(&request, "expected_revision")?.is_null()
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim create replay differs from retained request",
                ));
            }
            let (prepared, _proposal, candidate_reads) = create_plan(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &mut grant,
                &request,
                inventory,
                worker,
                deadline,
                cancelled,
                false,
            )?;
            if prepared.as_ref().and_then(|files| files.get(CONFIG_FILE))
                != package.get(CONFIG_FILE)
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim replay configuration differs",
                ));
            }
            add_commit_result(&mut response, receipt, true)?;
            let reads = private_input_reads(&grounded, &candidate_reads)?;
            Ok(PrivateClaimPlan {
                before,
                files: None,
                response,
                archive: None,
                reads,
            })
        }
        "prepare-create" => Err(SourceCommandError::Conflict(
            "private Claim target package already exists",
        )),
        "inspect-version" => {
            let identity = cmd::text(&request, "claim_id")?;
            let target = cmd::field(&request, "source")?;
            let mut retained_index = None;
            for (index, receipt) in cmd::array(&state.history, "receipts")?.iter().enumerate() {
                let subject = cmd::field(receipt, "previous_source")?;
                if cmd::text(subject, "id")? == identity && same_json(subject, target)? {
                    if retained_index.replace(index).is_some() {
                        return Err(SourceCommandError::Conflict(
                            "private Claim version identity is repeated in history",
                        ));
                    }
                }
            }
            let index = retained_index.ok_or(SourceCommandError::Conflict(
                "exact private Claim version is not retained",
            ))?;
            let retained_receipt = cmd::array(&state.history, "receipts")?.get(index).ok_or(
                SourceCommandError::Conflict("Claim archive receipt is absent"),
            )?;
            let (archived, locations) = read_archive(
                &grant,
                context_config,
                archive_reader,
                retained_receipt,
                deadline,
                cancelled,
            )?;
            let archive_records = parse_claims(required_package_file(&archived, CLAIM_STREAM)?)?;
            let record = archive_records
                .get(identity)
                .ok_or(SourceCommandError::Conflict("archived Claim is absent"))?;
            cmd::set(&mut response, "record", record.clone())?;
            cmd::set(&mut response, "files", locations)?;
            cmd::set(&mut response, "inspected_source", target.clone())?;
            Ok(PrivateClaimPlan {
                before,
                files: None,
                response,
                archive: None,
                reads: grounded.reads,
            })
        }
        "prepare" => {
            let identity = cmd::text(&request, "claim_id")?;
            let record = state
                .records
                .get(identity)
                .ok_or(SourceCommandError::Conflict("private Claim is absent"))?;
            let payload = state
                .forms
                .get(identity)
                .ok_or(SourceCommandError::Conflict(
                    "private Claim forms are absent",
                ))?;
            let form = cmd::text(&request, "form_id")?;
            allowed_text(&grant.value, "allowed_form_ids", form, 32)?;
            let change = source_forms::prepare_form_change(
                record,
                Some(payload),
                cmd::text(&grant.value, "principal_id")?,
                form,
                cmd::text(&request, "field_id")?,
            )
            .map_err(|_| SourceCommandError::Invalid("private Claim form proposal"))?;
            validate_claim_form_change(&grant, &change, Some(payload))?;
            cmd::set(&mut response, "prepared_change", change)?;
            Ok(PrivateClaimPlan {
                before,
                files: None,
                response,
                archive: None,
                reads: grounded.reads,
            })
        }
        "prepare-revise" => {
            let (revised, _output, proposed, refs, views) = prepare_revision(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &mut grant,
                &request,
                &state,
                package,
                inventory,
                worker,
                deadline,
                cancelled,
            )?;
            cmd::set(&mut response, "prepared_source", record_ref(&revised)?)?;
            cmd::set(&mut response, "prepared_forms", JsonValue::Array(refs))?;
            cmd::set(
                &mut response,
                "prepared_materializations",
                JsonValue::Array(views),
            )?;
            cmd::set(
                &mut response,
                "expected_dependencies",
                cmd::string(&proposed.dependencies),
            )?;
            cmd::set(
                &mut response,
                "source_bindings",
                JsonValue::Array(proposed.bindings.clone()),
            )?;
            let reads = private_input_reads(&grounded, &proposed.reads)?;
            Ok(PrivateClaimPlan {
                before,
                files: None,
                response,
                archive: None,
                reads,
            })
        }
        "claim.revise" => {
            ensure_operation(&grant, "claim.revise")?;
            let command = cmd::text(&request, "command_id")?;
            let selected_id = cmd::text(&request, "claim_id")?;
            let prior = state
                .records
                .get(selected_id)
                .ok_or(SourceCommandError::Conflict("private Claim is absent"))?;
            validate_revision_request(&grant, &request, prior)?;
            if let Some((kind, identity, receipt)) = claim_command_receipt(&state, command)? {
                if kind != "claim.revise"
                    || identity != selected_id
                    || cmd::text(receipt, "request_digest")? != request_digest(&request)?
                    || cmd::text(receipt, "owner_configuration")? != grant.digest
                    || cmd::text(&request, "expected_configuration")? != grant.digest
                    || cmd::text(receipt, "principal_id")?
                        != cmd::text(&grant.value, "principal_id")?
                    || cmd::text(receipt, "authority_ref")?
                        != cmd::text(&grant.value, "authority_ref")?
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim command identity was reused",
                    ));
                }
                add_commit_result(&mut response, receipt, true)?;
                return Ok(PrivateClaimPlan {
                    before,
                    files: None,
                    response,
                    archive: None,
                    reads: grounded.reads,
                });
            }
            let revision = package_revision(package)?;
            let (revised, mut output, proposed, refs, _views) = prepare_revision(
                ctx,
                cut,
                software,
                components,
                owner,
                context_config,
                &mut grant,
                &request,
                &state,
                package,
                inventory,
                worker,
                deadline,
                cancelled,
            )?;
            request_commit_matches(
                &request,
                &grant,
                Some(&record_ref(prior)?),
                Some(&revision),
                &proposed.dependencies,
                &JsonValue::Array(proposed.bindings.clone()),
            )?;
            let archived = archive_package(
                &grant,
                context_config,
                &record_ref(prior)?,
                &revision,
                package,
            )?;
            let mut changed_fields = cmd::field(&request, "fields")?
                .as_object()
                .ok_or(SourceCommandError::Invalid("Claim revision fields"))?
                .iter()
                .map(|(key, _)| key.as_str().unwrap_or("").to_owned())
                .collect::<Vec<_>>();
            changed_fields.sort();
            let receipt = cmd::object(vec![
                ("command_id", cmd::field(&request, "command_id")?.clone()),
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
                ("previous_source", record_ref(prior)?),
                ("source", record_ref(&revised)?),
                ("previous_revision", cmd::string(&revision)),
                ("archive_path", cmd::string(&archived.0)),
                ("dependencies", cmd::string(&proposed.dependencies)),
                (
                    "source_bindings",
                    JsonValue::Array(proposed.bindings.clone()),
                ),
                (
                    "changed_fields",
                    JsonValue::Array(changed_fields.iter().map(|key| cmd::string(key)).collect()),
                ),
                ("forms", JsonValue::Array(refs)),
                ("grants_admission", JsonValue::Bool(false)),
                ("request", request.clone()),
            ]);
            append_history(&mut output, &grant, &receipt)?;
            let mut records = state.records.clone();
            records.insert(selected_id.to_owned(), revised);
            let mut forms = state.forms.clone();
            forms.insert(
                selected_id.to_owned(),
                cmd::parse(required_package_file(
                    &output,
                    &claim_form_filename(selected_id),
                )?)?,
            );
            let mut next_response = response_state(
                response_base(&grant, true)?,
                &output,
                &records,
                &forms,
                &proposed,
                Some(selected_id),
            )?;
            add_commit_result(&mut next_response, &receipt, false)?;
            let reads = private_input_reads(&grounded, &proposed.reads)?;
            Ok(PrivateClaimPlan {
                before,
                files: Some(output),
                response: next_response,
                archive: Some(archived),
                reads,
            })
        }
        "apply" => {
            let identity = cmd::text(&request, "claim_id")?;
            let command = cmd::text(&request, "command_id")?;
            let record = state
                .records
                .get(identity)
                .ok_or(SourceCommandError::Conflict("private Claim is absent"))?;
            let payload = state
                .forms
                .get(identity)
                .ok_or(SourceCommandError::Conflict(
                    "private Claim forms are absent",
                ))?;
            let changes = cmd::array(&request, "changes")?;
            if changes.is_empty() || changes.len() > 32 {
                return Err(SourceCommandError::Invalid("private Claim form changes"));
            }
            let mut form_ids = BTreeSet::new();
            for change in changes {
                validate_claim_form_change(&grant, change, None)?;
                let form_id = cmd::text(cmd::field(change, "form")?, "form_id")?;
                if !form_ids.insert(form_id.to_owned()) {
                    return Err(SourceCommandError::Conflict(
                        "private Claim form change repeats an identity",
                    ));
                }
            }
            let result_refs = changes
                .iter()
                .map(|change| {
                    source_forms::form_reference(cmd::field(change, "form")?)
                        .map_err(|_| SourceCommandError::Invalid("private Claim form reference"))
                })
                .collect::<SourceCommandResult<Vec<_>>>()?;
            if let Some((kind, retained_identity, receipt)) =
                claim_command_receipt(&state, command)?
            {
                if kind != "apply"
                    || retained_identity != identity
                    || cmd::text(receipt, "request_digest")? != request_digest(&request)?
                    || cmd::text(receipt, "owner_configuration")? != grant.digest
                    || cmd::text(&request, "expected_configuration")? != grant.digest
                    || cmd::text(receipt, "principal_id")?
                        != cmd::text(&grant.value, "principal_id")?
                    || cmd::text(receipt, "authority_ref")?
                        != cmd::text(&grant.value, "authority_ref")?
                    || !same_json(
                        cmd::field(receipt, "source")?,
                        cmd::field(&request, "expected_source")?,
                    )?
                    || cmd::field(receipt, "previous_revision")?
                        != cmd::field(&request, "expected_revision")?
                    || !same_json(
                        cmd::field(receipt, "results")?,
                        &JsonValue::Array(result_refs.clone()),
                    )?
                {
                    return Err(SourceCommandError::Conflict(
                        "private Claim command identity was reused",
                    ));
                }
                add_commit_result(&mut response, receipt, true)?;
                return Ok(PrivateClaimPlan {
                    before,
                    files: None,
                    response,
                    archive: None,
                    reads: grounded.reads,
                });
            }
            let revision = package_revision(package)?;
            request_commit_matches(
                &request,
                &grant,
                Some(&record_ref(record)?),
                Some(&revision),
                &grounded.dependencies,
                &JsonValue::Array(grounded.bindings.clone()),
            )?;
            for change in changes {
                validate_claim_form_change(&grant, change, Some(payload))?;
            }
            let mut sibling_ids = BTreeSet::new();
            for (sibling, sibling_forms) in &state.forms {
                if sibling == identity {
                    continue;
                }
                for forms in [
                    optional_array(sibling_forms, "forms")?,
                    optional_array(sibling_forms, "prior_forms")?,
                ] {
                    for form in forms {
                        sibling_ids.insert(cmd::text(form, "form_id")?.to_owned());
                    }
                }
            }
            if form_ids
                .iter()
                .any(|form_id| sibling_ids.contains(form_id.as_str()))
            {
                return Err(SourceCommandError::Conflict(
                    "private Claim form identity belongs to a sibling",
                ));
            }
            let subject = record_ref(record)?;
            let mut next_forms = source_forms::apply_form_changes(Some(payload), &subject, changes)
                .map_err(|_| SourceCommandError::Conflict("private Claim form successor"))?;
            super::profile::validate_form_set(
                ctx,
                worker,
                grant.source_path.as_str(),
                &next_forms,
                deadline,
                cancelled,
            )?;
            let views = private_claim_materializations(record, &next_forms)?;
            for change in changes {
                let form = cmd::field(change, "form")?;
                if cmd::text(cmd::field(form, "content")?, "kind")? == "source-copy" {
                    let id = cmd::text(form, "form_id")?;
                    let mut ready = false;
                    for view in &views {
                        if cmd::text(view, "state")? == "ready"
                            && cmd::text(cmd::field(view, "form")?, "id")? == id
                        {
                            ready = true;
                            break;
                        }
                    }
                    if !ready {
                        return Err(SourceCommandError::Invalid(
                            "private Claim source-copy form loses mandatory context",
                        ));
                    }
                }
            }
            let receipt = cmd::object(vec![
                ("command_id", cmd::field(&request, "command_id")?.clone()),
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
                ("source", subject),
                (
                    "previous_revision",
                    cmd::field(&request, "expected_revision")?.clone(),
                ),
                ("results", JsonValue::Array(result_refs)),
            ]);
            let mut growth = optional_array(&next_forms, "growth_history")?.to_vec();
            growth.push(receipt.clone());
            cmd::set(&mut next_forms, "growth_history", JsonValue::Array(growth))?;
            super::profile::validate_form_set(
                ctx,
                worker,
                grant.source_path.as_str(),
                &next_forms,
                deadline,
                cancelled,
            )?;
            let mut output = package.clone();
            output.insert(claim_form_filename(identity), cmd::published(&next_forms)?);
            let mut forms = state.forms.clone();
            forms.insert(identity.to_owned(), next_forms);
            let mut next_response = response_state(
                response_base(&grant, true)?,
                &output,
                &state.records,
                &forms,
                &grounded,
                Some(identity),
            )?;
            add_commit_result(&mut next_response, &receipt, false)?;
            Ok(PrivateClaimPlan {
                before,
                files: Some(output),
                response: next_response,
                archive: None,
                reads: grounded.reads,
            })
        }
        _ => Err(SourceCommandError::Unsupported("private Claim operation")),
    }
}
