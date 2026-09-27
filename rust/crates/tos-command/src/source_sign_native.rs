//! Exact native TextUnit assessment inputs for the maintained Sign v2/v3 consumer.
//! This reader returns local assessment Record envelopes, never content exports,
//! source admission, publication permission or an authenticated provider claim.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, python_strip_unicode16_v1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

// NativeTextBindingResolver's fixed source budgets, distinct from the tighter
// retained native derivation operation profile below.
const MAX_METADATA_FILE_BYTES: usize = 1_048_576;
const MAX_INPUT_FILES: usize = 128;
const MAX_METADATA_BYTES: usize = 8_388_608;
const MAX_CONTENT_BYTES: usize = 8_388_608;
const MAX_DERIVED_TEXT_BYTES: usize = 131_072;
const MAX_EDITS: usize = 128;
const ASSESSMENT_SCHEMA: &str = "native-text-unit-assessment-subject.schema.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeReadKind {
    Metadata,
    Schema,
    Support,
    Content,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeReadScope {
    MetadataOnly,
    PublicContent,
    ExactOwnerLocal,
}

/// Implemented by the Sign consumer's selected protected source-root transport.
/// These methods observe byte custody and current transport, never issue a grant.
/// Sign v2/v3 has no OwnerLocalSourceContext; a true owner_local role refuses.
pub trait SignNativeRead {
    fn read(
        &mut self,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>>;
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()>;
    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool>;
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeInput {
    pub reference: String,
    pub kind: NativeReadKind,
    pub category: &'static str,
    pub raw_sha256: Digest256,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedSignBinding {
    pub packet: JsonValue,
    pub layer: JsonValue,
    pub summary: JsonValue,
    pub inputs: Vec<NativeInput>,
    pub input_snapshot: String,
    pub schema_digests: BTreeMap<String, Digest256>,
}
pub(crate) struct ResolvedSignBindingBatch {
    pub summaries: Vec<JsonValue>,
    pub inputs: Vec<NativeInput>,
    pub input_snapshot: String,
    pub schema_digests: BTreeMap<String, Digest256>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSignNative {
    /// Maintained Record.from_payload inputs: id/version/payload/origin_id.
    pub records: [JsonValue; 2],
    pub summary: JsonValue,
    pub inputs: Vec<NativeInput>,
    pub input_snapshot: String,
    pub schema_digests: BTreeMap<String, Digest256>,
}
struct Cached {
    raw: Vec<u8>,
    kind: NativeReadKind,
}
struct Native<'a, R: SignNativeRead + ?Sized> {
    reader: &'a mut R,
    worker: &'a mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    cache: BTreeMap<(String, &'static str), Cached>,
    schemas: BTreeMap<String, Digest256>,
    remaining_metadata: usize,
    remaining_content: usize,
}
fn path(name: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(name)
        .map_err(|_| SourceCommandError::Invalid("unsafe native source reference"))
}
fn split(name: &str) -> SourceCommandResult<(&str, &str)> {
    name.rsplit_once('/').ok_or(SourceCommandError::Invalid(
        "native source reference parent",
    ))
}
fn texts(value: &JsonValue, key: &str, max: usize) -> SourceCommandResult<Vec<String>> {
    let values = cmd::array(value, key)?;
    if values.len() > max {
        return Err(SourceCommandError::Invalid(
            "native metadata reference count",
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(String::from)
                .ok_or(SourceCommandError::Invalid("native metadata reference"))
        })
        .collect()
}
impl<R: SignNativeRead + ?Sized> Native<'_, R> {
    fn tick(&self) -> SourceCommandResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported("native read cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(SourceCommandError::Unsupported("native read deadline"));
        }
        Ok(())
    }
    fn route(&self, name: &str, kind: NativeReadKind) -> SourceCommandResult<()> {
        path(name)?;
        let home = match kind {
            NativeReadKind::Schema => "ToS/contracts/",
            NativeReadKind::Support => "ToS/",
            _ => "ToS/source-witnesses/",
        };
        if !name.starts_with(home)
            || name.split('/').any(|part| {
                part == "catalog"
                    || kind != NativeReadKind::Content
                        && matches!(part, "payload" | "local-content")
            })
        {
            return Err(SourceCommandError::Denied(
                "native reference leaves its selected source home",
            ));
        }
        if name.starts_with("ToS/source-witnesses/owner-local/") || self.reader.owner_local(name)? {
            return Err(SourceCommandError::Unsupported(
                "Sign v2/v3 excludes owner-local transport context",
            ));
        }
        Ok(())
    }
    fn read(
        &mut self,
        name: &str,
        expected: Option<&str>,
        support: bool,
        schema: bool,
    ) -> SourceCommandResult<Vec<u8>> {
        let kind = if schema {
            NativeReadKind::Schema
        } else if support {
            NativeReadKind::Support
        } else {
            NativeReadKind::Metadata
        };
        self.raw(name, expected, kind)
    }
    fn raw(
        &mut self,
        name: &str,
        expected: Option<&str>,
        kind: NativeReadKind,
    ) -> SourceCommandResult<Vec<u8>> {
        self.tick()?;
        self.route(name, kind)?;
        let category = if kind == NativeReadKind::Content {
            "content"
        } else {
            "metadata"
        };
        let key = (name.to_string(), category);
        if !self.cache.contains_key(&key) {
            let remaining = if category == "content" {
                self.remaining_content
            } else {
                self.remaining_metadata.min(MAX_METADATA_FILE_BYTES)
            };
            if self.cache.len() >= MAX_INPUT_FILES {
                return Err(SourceCommandError::Invalid(
                    "native input dependency count budget",
                ));
            }
            let raw = self
                .reader
                .read(name, kind, remaining, self.deadline, self.cancelled)?;
            self.tick()?;
            if raw.len() > remaining {
                return Err(SourceCommandError::Invalid("native input byte budget"));
            }
            if category == "content" {
                self.remaining_content -= raw.len();
            } else {
                self.remaining_metadata -= raw.len();
            }
            self.cache.insert(key.clone(), Cached { raw, kind });
        }
        let input = self
            .cache
            .get_mut(&key)
            .ok_or(SourceCommandError::Invalid("native cached input"))?;
        // A support record may later also be a grammar; preserve the stronger
        // selected role while the opaque snapshot remains metadata-category.
        if kind == NativeReadKind::Schema
            || kind == NativeReadKind::Support && input.kind == NativeReadKind::Metadata
        {
            input.kind = kind;
        }
        let digest = Digest256::of_bytes(&input.raw);
        if expected.is_some_and(|expected| expected != digest.to_hex()) {
            return Err(SourceCommandError::Conflict(
                "native exact input SHA256 differs",
            ));
        }
        if kind == NativeReadKind::Schema {
            self.schemas.insert(name.into(), digest);
        }
        Ok(input.raw.clone())
    }
    fn record(
        &mut self,
        name: &str,
        expected: Option<&str>,
    ) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
        let raw = self.read(name, expected, false, false)?;
        let value = cmd::parse(&raw)?;
        if value.as_object().is_none() {
            return Err(SourceCommandError::Invalid("native JSON metadata object"));
        }
        Ok((value, raw))
    }
    fn validate(&mut self, value: &JsonValue, basename: &str) -> SourceCommandResult<()> {
        let name = format!("ToS/contracts/{basename}");
        let grammar_raw = self.read(&name, None, false, true)?;
        let grammar = cmd::parse(&grammar_raw)?;
        if cmd::text(&grammar, "$id")? != format!("https://tree-of-sophia.local/{name}") {
            return Err(SourceCommandError::Invalid(
                "native grammar exact source identity",
            ));
        }
        let allowed = if basename == ASSESSMENT_SCHEMA {
            for dependency in [
                "native-text-unit-binding.schema.json",
                "source-text-unit-packet-v1.schema.json",
            ] {
                let dependency_name = format!("ToS/contracts/{dependency}");
                let raw = self.read(&dependency_name, None, false, true)?;
                if self.worker.contract_digest(&dependency_name) != Some(Digest256::of_bytes(&raw))
                {
                    return Err(SourceCommandError::Conflict(
                        "native assessment dependency and schema worker differ",
                    ));
                }
            }
            vec![
                "https://tree-of-sophia.local/ToS/contracts/native-text-unit-binding.schema.json",
                "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
            ]
        } else {
            vec![]
        };
        schema_refs(&grammar, &allowed, 0)?;
        if self.worker.contract_digest(&name) != Some(Digest256::of_bytes(&grammar_raw)) {
            return Err(SourceCommandError::Conflict(
                "native schema worker and selected grammar differ",
            ));
        }
        match self.worker.check(
            &name,
            &cmd::canonical(value)?,
            &name,
            self.deadline,
            self.cancelled,
        ) {
            Ok(true) => Ok(()),
            Ok(false) => Err(SourceCommandError::Invalid(
                "native value violates exact source grammar",
            )),
            Err(reason) => Err(SourceCommandError::SchemaExecution {
                path: name.clone(),
                root: name,
                reason,
            }),
        }
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
    fn source_scope(
        &mut self,
        binding: &JsonValue,
        scope: &JsonValue,
        layer: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
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
            self.validate(&record, "corpus-record.schema.json")?;
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
        self.validate(&manifest, "source-item-manifest.schema.json")?;
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
                "native predecessor lineage cycle or depth",
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
        let configuration = match maker
            .object_get("configuration_ref")
            .filter(|v| **v != JsonValue::Null)
        {
            Some(value) => {
                let raw = self.read(
                    value.as_str().ok_or(SourceCommandError::Invalid(
                        "native maker configuration reference",
                    ))?,
                    Some(cmd::text(maker, "configuration_digest")?),
                    true,
                    false,
                )?;
                optional_configuration(&raw)
            }
            None => None,
        };
        let mut previous = Vec::new();
        for target in cmd::array(derivation, "input_layers")? {
            let name = cmd::text(target, "record_ref")?;
            let (value, raw) = self.record(name, Some(cmd::text(target, "record_sha256")?))?;
            self.validate(&value, "source-text-layer.schema.json")?;
            if cmd::field(&value, "layer_id")? != cmd::field(target, "layer_id")?
                || cmd::field(cmd::field(&value, "representation")?, "content_sha256")?
                    != cmd::field(target, "content_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "native predecessor identity or content digest differs",
                ));
            }
            self.layer_dependencies(&value, &raw, name, visiting)?;
            previous.push(value);
        }
        if let Some(configuration) = configuration.filter(retained_configuration) {
            self.derived_metadata(layer, &configuration, &previous, name)?;
        }
        visiting.remove(id);
        Ok(())
    }

    fn derived_metadata(
        &mut self,
        layer: &JsonValue,
        config: &JsonValue,
        previous: &[JsonValue],
        name: &str,
    ) -> SourceCommandResult<()> {
        let operations = cmd::array(config, "allowed_operations")?;
        if operations.len() != 1 {
            return Err(SourceCommandError::Invalid(
                "retained native derivation exact operation",
            ));
        }
        let operation = operations[0].as_str().ok_or(SourceCommandError::Invalid(
            "retained native derivation operation",
        ))?;
        let version = cmd::text(config, "schema_version")?;
        let observed = matches!(
            (operation, version),
            (
                "text-layer.record-owner-ocr",
                "tos_local_text_layer_record_owner_ocr_v1"
            ) | (
                "text-layer.record-owner-page-ocr",
                "tos_local_text_layer_record_owner_page_ocr_v1"
            )
        );
        let ordinary = match operation {
            "text-layer.correct" => "correction",
            "text-layer.normalize" => "unicode_normalization",
            "text-layer.record-transcription" => "manual_transcription",
            "text-layer.record-ocr" => "ocr",
            _ if observed => "ocr",
            _ => {
                return Err(SourceCommandError::Invalid(
                    "retained native derivation operation profile",
                ));
            }
        };
        if matches!(
            version,
            "tos_local_text_layer_record_owner_ocr_v1"
                | "tos_local_text_layer_record_owner_page_ocr_v1"
        ) != observed
        {
            return Err(SourceCommandError::Invalid(
                "retained native OCR configuration profile differs",
            ));
        }
        let rep = cmd::field(layer, "representation")?;
        let derivation = cmd::field(layer, "derivation")?;
        let maker = cmd::field(derivation, "maker")?;
        let scope = cmd::field(layer, "source_binding")?;
        let exact_scope = layer_scope(scope)?;
        let policy_ref = cmd::field(layer, "editorial_policy")?;
        let (policy, _) = self.record(
            cmd::text(policy_ref, "policy_ref")?,
            Some(cmd::text(policy_ref, "policy_sha256")?),
        )?;
        let method = cmd::text(derivation, "method")?;
        let role = if operation == "text-layer.normalize" {
            "normalized_text"
        } else if operation == "text-layer.record-ocr" || observed {
            "raw_ocr"
        } else if method == "model_transcription" {
            "machine_transcription"
        } else {
            "diplomatic_transcription"
        };
        let base = split(cmd::text(config, "source_path")?)?.0;
        let identities = cmd::field(config, "identities")?;
        let maker_identity = cmd::object(
            ["maker_type", "agent_ref", "method", "version"]
                .iter()
                .map(|key| Ok((*key, cmd::field(maker, key)?.clone())))
                .collect::<SourceCommandResult<Vec<_>>>()?,
        );
        if cmd::text(config, "source_path")? != name
            || !cmd::same(cmd::field(config, "source_scope")?, &exact_scope)?
            || cmd::field(identities, "layer_id")? != cmd::field(layer, "layer_id")?
            || cmd::field(identities, "provenance_event_id")?
                != cmd::field(layer, "provenance_event_ref")?
            || if operation == "text-layer.record-transcription" {
                !["manual_transcription", "model_transcription"].contains(&method)
            } else {
                method != ordinary
            }
            || !cmd::same(cmd::field(config, "policy")?, &policy)?
            || cmd::text(&policy, "schema_version")? != "tos_native_text_layer_derivation_policy_v1"
            || cmd::text(&policy, "operation")? != operation
            || cmd::text(&policy, "method")? != method
            || cmd::field(&policy, "provider_execution_verified")? != &JsonValue::Bool(observed)
            || cmd::text(&policy, "inherited_quality")? != "not-transferred"
            || cmd::text(layer, "layer_role")? != role
            || cmd::field(config, "language")? != cmd::field(rep, "language")?
            || cmd::text(rep, "content_ref")? != format!("{base}/content.txt")
            || cmd::field(rep, "character_normalization")?
                != cmd::field(&policy, "unicode_normalization")?
            || cmd::text(maker, "configuration_ref")?
                != format!("{base}/source-create-owner-configuration.json")
            || !cmd::same(&maker_identity, cmd::field(config, "maker")?)?
            || !cmd::same(
                cmd::field(rep, "rights_record_refs")?,
                cmd::field(
                    cmd::field(config, "derivation_access")?,
                    "rights_record_refs",
                )?,
            )?
        {
            return Err(SourceCommandError::Conflict(
                "derived native layer differs from retained exact configuration",
            ));
        }
        if ["text-layer.correct", "text-layer.normalize"].contains(&operation) {
            let binding = cmd::field(cmd::field(config, "input")?, "binding")?;
            let target = cmd::field(binding, "text_layer")?;
            if previous.len() != 1 {
                return Err(SourceCommandError::Invalid(
                    "derived native layer requires exact single predecessor",
                ));
            }
            let prior = &previous[0];
            let mut expected = cmd::object(
                ["layer_id", "record_ref", "record_sha256"]
                    .iter()
                    .map(|key| Ok((*key, cmd::field(target, key)?.clone())))
                    .collect::<SourceCommandResult<Vec<_>>>()?,
            );
            cmd::set(
                &mut expected,
                "content_sha256",
                cmd::field(cmd::field(prior, "representation")?, "content_sha256")?.clone(),
            )?;
            if !cmd::same(
                cmd::field(derivation, "input_layers")?,
                &JsonValue::Array(vec![expected]),
            )? || cmd::field(prior, "layer_version")? != cmd::field(target, "layer_version")?
                || !cmd::same(cmd::field(prior, "source_binding")?, scope)?
                || cmd::field(cmd::field(prior, "representation")?, "language")?
                    != cmd::field(rep, "language")?
                || cmd::field(layer, "supersedes_layer_ref")? != cmd::field(prior, "layer_id")?
                || cmd::integer(layer, "layer_version")?
                    != cmd::integer(prior, "layer_version")?.checked_add(1).ok_or(
                        SourceCommandError::Invalid("native layer version exhaustion"),
                    )?
                || !cmd::same(
                    cmd::field(binding, "source_record_refs")?,
                    cmd::field(config, "source_record_refs")?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "derived native predecessor version lineage or source scope differs",
                ));
            }
            if operation == "text-layer.correct"
                && (cmd::text(prior, "layer_role")? == "normalized_text"
                    || cmd::text(
                        cmd::field(prior, "representation")?,
                        "character_normalization",
                    )? != "none")
            {
                return Err(SourceCommandError::Denied(
                    "source-near native correction cannot erase predecessor normalization",
                ));
            }
        } else {
            let target = cmd::field(cmd::field(config, "input")?, "anchor")?;
            let expected = cmd::object(vec![
                ("anchor_id", cmd::field(target, "anchor_id")?.clone()),
                (
                    "anchor_record_ref",
                    cmd::field(target, "record_ref")?.clone(),
                ),
                (
                    "anchor_record_sha256",
                    cmd::field(target, "record_sha256")?.clone(),
                ),
            ]);
            let material = cmd::field(config, "material")?;
            if !previous.is_empty()
                || cmd::integer(layer, "layer_version")? != 1
                || cmd::field(layer, "supersedes_layer_ref")? != &JsonValue::Null
                || !cmd::same(
                    cmd::field(scope, "anchors")?,
                    &JsonValue::Array(vec![expected]),
                )?
                || !observed && cmd::text(material, "provider_execution")? != "not_observed"
                || cmd::field(rep, "content_sha256")? != cmd::field(material, "content_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "supplied native result differs from exact source and byte declaration",
                ));
            }
            if observed {
                return Err(SourceCommandError::Unsupported(
                    "retained owner OCR requires genuine verify_owner_ocr cryptographic backend and excluded owner-local context",
                ));
            }
        }
        Ok(())
    }

    fn source_anchors(&mut self, layer: &JsonValue, scope: &JsonValue) -> SourceCommandResult<()> {
        for target in cmd::array(cmd::field(layer, "source_binding")?, "anchors")? {
            let name = cmd::text(target, "anchor_record_ref")?;
            let (anchor, raw) =
                self.record(name, Some(cmd::text(target, "anchor_record_sha256")?))?;
            self.validate(&anchor, "source-anchor-v2.schema.json")?;
            self.metadata(&raw, name, "anchor")?;
            let endpoint = cmd::field(&anchor, "target")?;
            if cmd::field(&anchor, "anchor_id")? != cmd::field(target, "anchor_id")?
                || cmd::field(endpoint, "item_id")? != cmd::field(scope, "item_ref")?
                || cmd::field(endpoint, "file_id")? != cmd::field(scope, "file_ref")?
                || cmd::field(endpoint, "file_sha256")? != cmd::field(scope, "file_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "native layer source anchor closure differs",
                ));
            }
        }
        Ok(())
    }
    fn rights_records(
        &mut self,
        layer: &JsonValue,
        scope: &JsonValue,
        manifest: &JsonValue,
    ) -> SourceCommandResult<Vec<JsonValue>> {
        let rep = cmd::field(layer, "representation")?;
        let refs = cmd::array(rep, "rights_record_refs")?;
        if refs.is_empty()
            || !refs
                .iter()
                .any(|row| row.object_get("ref") == manifest.object_get("rights_ref"))
        {
            return Err(SourceCommandError::Conflict(
                "native layer omits exact item rights closure",
            ));
        }
        let mut relevant = vec![
            cmd::text(layer, "layer_id")?,
            cmd::text(rep, "content_file_id")?,
        ];
        for (key, value) in scope
            .as_object()
            .ok_or(SourceCommandError::Invalid("native source scope object"))?
        {
            if key.as_str().is_some_and(|key| key.ends_with("_ref")) {
                relevant.push(
                    value
                        .as_str()
                        .ok_or(SourceCommandError::Invalid("native source scope identity"))?,
                );
            }
        }
        let mut records = Vec::new();
        for target in refs {
            let name = cmd::text(target, "ref")?;
            let (record, _) = self.record(name, Some(cmd::text(target, "sha256")?))?;
            self.validate(&record, "rights-record.schema.json")?;
            let scope_refs = texts(&record, "scope_refs", 4096)?;
            if !relevant.iter().any(|id| scope_refs.iter().any(|r| r == id))
                || name == cmd::text(manifest, "rights_ref")?
                    && ![cmd::text(scope, "item_ref")?, cmd::text(scope, "file_ref")?]
                        .iter()
                        .all(|id| scope_refs.iter().any(|r| r == id))
            {
                return Err(SourceCommandError::Denied(
                    "native rights record addresses different source scope",
                ));
            }
            records.push(record);
        }
        Ok(records)
    }
    fn publication_support(&mut self, layer: &JsonValue) -> SourceCommandResult<()> {
        for target in cmd::array(
            cmd::field(layer, "representation")?,
            "publication_authority_refs",
        )? {
            self.read(
                cmd::text(target, "ref")?,
                Some(cmd::text(target, "sha256")?),
                true,
                false,
            )?;
        }
        Ok(())
    }
    fn local_research_rights(
        &self,
        layer: &JsonValue,
        records: &[JsonValue],
    ) -> SourceCommandResult<()> {
        let exact = [
            cmd::text(layer, "layer_id")?,
            cmd::text(cmd::field(layer, "representation")?, "content_file_id")?,
        ];
        let mut decisions = Vec::new();
        for record in records {
            if ["superseded", "legal_review_requested"]
                .contains(&cmd::text(record, "review_status")?)
                || ["permission_denied", "conflicting_evidence"]
                    .contains(&cmd::text(record, "assessment_status")?)
            {
                return Err(SourceCommandError::Denied(
                    "native predecessor rights inactive or denied",
                ));
            }
            let mut scoped = Vec::new();
            for part in record
                .object_get("layer_assessments")
                .and_then(JsonValue::as_array)
                .unwrap_or(&[])
            {
                if intersects(part, &exact)? {
                    scoped.push(part);
                }
            }
            if !scoped.is_empty() {
                decisions.extend(scoped);
            } else if intersects(record, &exact)? {
                decisions.push(record);
            }
        }
        if decisions.is_empty() {
            decisions.extend(records);
        }
        for decision in decisions {
            if !["local_research_only", "allowed"]
                .contains(&cmd::text(decision, "derivative_posture")?)
                || ["permission_denied", "conflicting_evidence"]
                    .contains(&cmd::text(decision, "assessment_status")?)
                || ["superseded", "legal_review_requested"]
                    .contains(&cmd::text(decision, "review_status")?)
            {
                return Err(SourceCommandError::Denied(
                    "native predecessor lacks unconditional current local research rights route",
                ));
            }
        }
        Ok(())
    }
    fn public_rights(&self, layer: &JsonValue, records: &[JsonValue]) -> SourceCommandResult<()> {
        let exact = [
            cmd::text(layer, "layer_id")?,
            cmd::text(cmd::field(layer, "representation")?, "content_file_id")?,
        ];
        let mut applicable = Vec::new();
        for record in records {
            if intersects(record, &exact)? {
                applicable.push(record);
            }
        }
        if applicable.is_empty() {
            applicable.extend(records);
        }
        for record in applicable {
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
                    "native public content declaration conflicts with recorded current rights gate",
                ));
            }
        }
        Ok(())
    }
    fn resolve_layer_metadata(&mut self, binding: &JsonValue) -> SourceCommandResult<JsonValue> {
        self.validate(binding, "native-text-layer-binding.schema.json")?;
        let target = cmd::field(binding, "text_layer")?;
        let name = cmd::text(target, "record_ref")?;
        let (layer, raw) = self.record(name, Some(cmd::text(target, "record_sha256")?))?;
        self.validate(&layer, "source-text-layer.schema.json")?;
        if cmd::field(&layer, "layer_id")? != cmd::field(target, "layer_id")?
            || cmd::field(&layer, "layer_version")? != cmd::field(target, "layer_version")?
        {
            return Err(SourceCommandError::Conflict(
                "native predecessor layer identity or version differs",
            ));
        }
        self.layer_dependencies(&layer, &raw, name, &mut BTreeSet::new())?;
        let scope = layer_scope(cmd::field(&layer, "source_binding")?)?;
        let manifest = self.source_scope(binding, &scope, &layer)?;
        let rep = cmd::field(&layer, "representation")?;
        if !["text/plain", "text/plain; charset=utf-8"].contains(&cmd::text(rep, "media_type")?)
            || cmd::text(rep, "content_file_id")?
                != format!("tos.file.sha256.{}", cmd::text(rep, "content_sha256")?)
        {
            return Err(SourceCommandError::Invalid(
                "native predecessor needs exact UTF-8 representation file",
            ));
        }
        self.source_anchors(&layer, &scope)?;
        let rights = self.rights_records(&layer, &scope, &manifest)?;
        self.local_research_rights(&layer, &rights)?;
        self.publication_support(&layer)?;
        self.route(cmd::text(rep, "content_ref")?, NativeReadKind::Content)?;
        self.snapshot()?;
        Ok(layer)
    }
    fn derived_content(&mut self, layer: &JsonValue, text: &str) -> SourceCommandResult<()> {
        let maker = cmd::field(cmd::field(layer, "derivation")?, "maker")?;
        let Some(name) = maker
            .object_get("configuration_ref")
            .filter(|v| **v != JsonValue::Null)
        else {
            return Ok(());
        };
        let raw = self.read(
            name.as_str().ok_or(SourceCommandError::Invalid(
                "native derived configuration path",
            ))?,
            Some(cmd::text(maker, "configuration_digest")?),
            true,
            false,
        )?;
        let Some(config) = optional_configuration(&raw).filter(retained_configuration) else {
            return Ok(());
        };
        let rep = cmd::field(layer, "representation")?;
        let scope = cmd::field(rep, "text_scope")?;
        if text.len() > MAX_DERIVED_TEXT_BYTES
            || cmd::integer(scope, "start")? != 0
            || cmd::integer(scope, "end")? != text.chars().count() as u64
        {
            return Err(SourceCommandError::Invalid(
                "derived native content exceeds whole representation profile",
            ));
        }
        let operation = cmd::array(&config, "allowed_operations")?
            .first()
            .and_then(JsonValue::as_str)
            .ok_or(SourceCommandError::Invalid(
                "retained derived native content operation",
            ))?;
        if !["text-layer.correct", "text-layer.normalize"].contains(&operation) {
            if text.len() as u64 != cmd::integer(cmd::field(&config, "material")?, "byte_size")? {
                return Err(SourceCommandError::Conflict(
                    "native supplied result size differs from retained material",
                ));
            }
            return Ok(());
        }
        let binding = cmd::field(cmd::field(&config, "input")?, "binding")?;
        // This call verifies the independent predecessor rights before any
        // predecessor representation read; no content verdict is inherited.
        let previous = self.resolve_layer_metadata(binding)?;
        let prior = cmd::field(&previous, "representation")?;
        let declared_size = cmd::integer(cmd::field(&config, "source_access")?, "byte_size")?;
        if declared_size > MAX_DERIVED_TEXT_BYTES as u64 {
            return Err(SourceCommandError::Invalid(
                "native predecessor transformation byte profile",
            ));
        }
        let raw = self.raw(
            cmd::text(prior, "content_ref")?,
            Some(cmd::text(prior, "content_sha256")?),
            NativeReadKind::Content,
        )?;
        if raw.len() as u64 != declared_size {
            return Err(SourceCommandError::Conflict(
                "native predecessor size differs from retained input",
            ));
        }
        let input = std::str::from_utf8(&raw).map_err(|_| {
            SourceCommandError::Invalid("native predecessor content is not exact UTF-8")
        })?;
        let changes = cmd::field(cmd::field(layer, "derivation")?, "change_payload")?;
        let operations = cmd::array(changes, "operations")?;
        if cmd::text(changes, "kind")? != "explicit_operations"
            || operations.is_empty()
            || operations.len() > MAX_EDITS
        {
            return Err(SourceCommandError::Invalid(
                "native derived changes leave exact explicit operation profile",
            ));
        }
        // VAL owns the exact ordered Unicode-codepoint edit replay. This seam
        // has no normalization, rendering, provider or admission shortcut.
        tos_validation::text_rules::replay_source_text_layer_edits(
            input,
            text,
            operations,
            self.deadline,
            self.cancelled,
        )
        .map_err(|refusal| match refusal {
            tos_validation::item_rules::ItemRefusal::Deadline => {
                SourceCommandError::Unsupported("native edit replay deadline")
            }
            tos_validation::item_rules::ItemRefusal::Budget => {
                SourceCommandError::Unsupported("native edit replay backend budget")
            }
            tos_validation::item_rules::ItemRefusal::Unsupported(_) => {
                SourceCommandError::Unsupported("native edit replay scalar or backend profile")
            }
            tos_validation::item_rules::ItemRefusal::Source(_) => SourceCommandError::Invalid(
                "native derived edit evidence does not replay against exact predecessor bytes",
            ),
        })
    }
    fn snapshot(&mut self) -> SourceCommandResult<String> {
        self.tick()?;
        self.reader.verify_current(self.deadline, self.cancelled)?;
        let mut metadata = MAX_METADATA_BYTES;
        let mut content = MAX_CONTENT_BYTES;
        let mut value = Vec::new();
        for ((name, category), input) in &self.cache {
            self.route(name, input.kind)?;
            let remaining = if *category == "content" {
                content
            } else {
                metadata.min(MAX_METADATA_FILE_BYTES)
            };
            let raw =
                self.reader
                    .read(name, input.kind, remaining, self.deadline, self.cancelled)?;
            if raw.len() > remaining || raw != input.raw {
                return Err(SourceCommandError::Conflict(
                    "native dependency changed during current reread",
                ));
            }
            if *category == "content" {
                content -= raw.len();
            } else {
                metadata -= raw.len();
            }
            value.push(JsonValue::Array(vec![
                cmd::string(name),
                cmd::string(category),
                cmd::string(&Digest256::of_bytes(&raw).to_hex()),
            ]));
        }
        self.reader.verify_current(self.deadline, self.cancelled)?;
        self.tick()?;
        ascii_snapshot(&JsonValue::Array(value))
    }
    fn resolve(
        &mut self,
        binding: &JsonValue,
        scope: NativeReadScope,
    ) -> SourceCommandResult<(JsonValue, JsonValue, JsonValue)> {
        self.validate(binding, "native-text-unit-binding.schema.json")?;
        let packet_path = cmd::text(binding, "packet_ref")?;
        let (packet, packet_raw) =
            self.record(packet_path, Some(cmd::text(binding, "packet_sha256")?))?;
        self.validate(&packet, "source-text-unit-packet-v1.schema.json")?;
        if cmd::text(&packet, "content_posture")? != "source_bound"
            || cmd::field(&packet, "packet_id")? != cmd::field(binding, "packet_id")?
            || cmd::field(&packet, "packet_version")? != cmd::field(binding, "packet_version")?
        {
            return Err(SourceCommandError::Conflict(
                "native packet identity version or source posture differs",
            ));
        }
        let layer_binding = cmd::field(binding, "text_layer")?;
        let layer_path = cmd::text(layer_binding, "record_ref")?;
        let packet_layer = cmd::field(&packet, "source_layer")?;
        if cmd::text(packet_layer, "text_layer_ref")? != layer_path {
            return Err(SourceCommandError::Conflict(
                "native packet addresses another text layer",
            ));
        }
        let (layer, layer_raw) =
            self.record(layer_path, Some(cmd::text(layer_binding, "record_sha256")?))?;
        self.validate(&layer, "source-text-layer.schema.json")?;
        if cmd::field(&layer, "layer_id")? != cmd::field(layer_binding, "layer_id")?
            || cmd::field(&layer, "layer_version")? != cmd::field(layer_binding, "layer_version")?
        {
            return Err(SourceCommandError::Conflict(
                "native text layer identity or version differs",
            ));
        }
        self.metadata(&packet_raw, packet_path, "unit")?;
        self.layer_dependencies(&layer, &layer_raw, layer_path, &mut BTreeSet::new())?;
        let source_scope = cmd::field(&packet, "source_scope")?;
        let manifest = self.source_scope(binding, source_scope, &layer)?;
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
        let text_scope = cmd::field(rep, "text_scope")?;
        for anchor in cmd::array(&packet, "anchors")? {
            let selector = cmd::field(anchor, "selector")?;
            if cmd::field(cmd::field(anchor, "source_return")?, "locator_ref")?
                != cmd::field(rep, "content_ref")?
                || !(cmd::integer(text_scope, "start")? <= cmd::integer(selector, "start")?
                    && cmd::integer(selector, "start")? <= cmd::integer(selector, "end")?
                    && cmd::integer(selector, "end")? <= cmd::integer(text_scope, "end")?)
            {
                return Err(SourceCommandError::Conflict(
                    "native unit anchor leaves exact representation scope",
                ));
            }
        }
        self.source_anchors(&layer, source_scope)?;
        let rights = cmd::field(&packet, "rights_and_visibility")?;
        let rights_refs = texts(rights, "rights_record_refs", MAX_INPUT_FILES)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let declared = cmd::array(rep, "rights_record_refs")?
            .iter()
            .map(|row| cmd::text(row, "ref").map(String::from))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        if declared.is_empty() || rights_refs != declared {
            return Err(SourceCommandError::Conflict(
                "native packet and layer rights closure differs",
            ));
        }
        let rights_records = self.rights_records(&layer, source_scope, &manifest)?;
        self.publication_support(&layer)?;
        let units = cmd::array(&packet, "units")?
            .iter()
            .filter(|row| row.object_get("unit_id") == binding.object_get("unit_id"))
            .collect::<Vec<_>>();
        let segments = cmd::array(&packet, "segmentations")?
            .iter()
            .filter(|row| {
                row.object_get("segmentation_id") == binding.object_get("segmentation_id")
            })
            .collect::<Vec<_>>();
        if units.len() != 1 || segments.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "native unit or segmentation not unique in packet",
            ));
        }
        let unit = units[0];
        let segment = segments[0];
        if cmd::field(unit, "unit_version")? != cmd::field(binding, "unit_version")?
            || cmd::field(unit, "ordered_anchor_refs")?
                != cmd::field(binding, "ordered_anchor_refs")?
            || cmd::text(unit, "surface_posture")? != "source_bearing"
            || cmd::field(segment, "segmentation_version")?
                != cmd::field(binding, "segmentation_version")?
            || !cmd::array(segment, "ordered_unit_refs")?.contains(cmd::field(unit, "unit_id")?)
        {
            return Err(SourceCommandError::Conflict(
                "native unit membership version or ordered anchors differs",
            ));
        }
        let public = cmd::text(rep, "content_visibility")? == "public"
            && cmd::field(rep, "publication_authorized")? == &JsonValue::Bool(true)
            && cmd::text(rights, "packet_visibility")? == "public"
            && cmd::text(rights, "effective_visibility")? == "public"
            && cmd::field(rights, "publication_authorized")? == &JsonValue::Bool(true)
            && cmd::field(rights, "private_source_used")? == &JsonValue::Bool(false);
        if public {
            self.public_rights(&layer, &rights_records)?;
        }
        let content_path = cmd::text(rep, "content_ref")?;
        self.route(content_path, NativeReadKind::Content)?;
        let verified = scope != NativeReadScope::MetadataOnly;
        if verified {
            if !public && scope != NativeReadScope::ExactOwnerLocal {
                return Err(SourceCommandError::Denied(
                    "exact nonpublic native content needs explicit selected read scope",
                ));
            }
            // All topology, source, rights and transport-route checks above
            // precede this first content read. Metadata alone cannot reach it.
            let raw = self.raw(
                content_path,
                Some(cmd::text(rep, "content_sha256")?),
                NativeReadKind::Content,
            )?;
            let text = std::str::from_utf8(&raw).map_err(|_| {
                SourceCommandError::Invalid("native representation is not exact UTF-8")
            })?;
            let selected = codepoint_span(
                text,
                cmd::integer(text_scope, "start")?,
                cmd::integer(text_scope, "end")?,
            )?;
            check_normalization(selected, normalization)?;
            use tos_validation::text_rules::{self as rules, TextRuleContext, TextRuleState};
            let generation = Digest256::of_bytes(&packet_raw).to_prefixed();
            let report = rules::inspect_source_text_unit_v1(
                &packet_raw,
                &raw,
                &TextRuleContext {
                    packet_path: packet_path.into(),
                    frozen_text_locator: content_path.into(),
                    schema_checked: true,
                    requested_profiles: vec![rules::TEXT_UNIT_PROFILE.into()],
                    interval_generation: generation.clone(),
                    reverse_generation: generation,
                },
            );
            if report.packet_digest != Some(Digest256::of_bytes(&packet_raw).to_hex())
                || report.rule_id != rules::TEXT_UNIT_RULE_ID
            {
                return Err(SourceCommandError::Conflict(
                    "native content predicate exact packet binding differs",
                ));
            }
            if !report.unsupported_profiles.is_empty() {
                return Err(SourceCommandError::Unsupported(
                    "native content predicate profile unsupported",
                ));
            }
            match report.state {
                TextRuleState::Checked
                    if report.issues.is_empty() && report.unsupported_profiles.is_empty() =>
                {
                    ()
                }
                TextRuleState::Unsupported => {
                    return Err(SourceCommandError::Unsupported(
                        "native content predicate profile unsupported",
                    ));
                }
                TextRuleState::BudgetExceeded => {
                    return Err(SourceCommandError::Unsupported(
                        "native content predicate backend budget",
                    ));
                }
                _ => {
                    return Err(SourceCommandError::Invalid(
                        "native unit anchors or coverage do not resolve exact bytes",
                    ));
                }
            }
            self.derived_content(&layer, text)?;
        }
        self.snapshot()?;
        let summary = cmd::object(vec![
            ("metadata_verified", JsonValue::Bool(true)),
            ("content_verified", JsonValue::Bool(verified)),
            ("original_payload_verified", JsonValue::Bool(false)),
            ("unit_id", cmd::field(unit, "unit_id")?.clone()),
            ("unit_version", cmd::field(unit, "unit_version")?.clone()),
            ("unit_kind", cmd::field(unit, "unit_kind")?.clone()),
            (
                "segmentation_id",
                cmd::field(segment, "segmentation_id")?.clone(),
            ),
            (
                "segmentation_version",
                cmd::field(segment, "segmentation_version")?.clone(),
            ),
            ("layer_id", cmd::field(&layer, "layer_id")?.clone()),
            (
                "layer_version",
                cmd::field(&layer, "layer_version")?.clone(),
            ),
            ("language", cmd::field(rep, "language")?.clone()),
            (
                "effective_visibility",
                cmd::field(rights, "effective_visibility")?.clone(),
            ),
            ("public_content_declared", JsonValue::Bool(public)),
            (
                "public_content_available",
                JsonValue::Bool(verified && public),
            ),
            (
                "native_status",
                cmd::object(vec![
                    (
                        "unit_boundary_posture",
                        cmd::field(unit, "boundary_posture")?.clone(),
                    ),
                    (
                        "segmentation_status",
                        cmd::field(segment, "status")?.clone(),
                    ),
                    (
                        "layer_review_status",
                        cmd::field(cmd::field(&layer, "admission")?, "review_status")?.clone(),
                    ),
                ]),
            ),
            ("assessment_applied", JsonValue::Bool(false)),
        ]);
        Ok((packet, layer, summary))
    }
}

fn layer_scope(source: &JsonValue) -> SourceCommandResult<JsonValue> {
    let mut fields = ["work_ref", "expression_ref", "edition_ref", "item_ref"]
        .iter()
        .map(|key| Ok((*key, cmd::field(source, key)?.clone())))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    fields.push(("file_ref", cmd::field(source, "source_file_ref")?.clone()));
    fields.push((
        "file_sha256",
        cmd::field(source, "source_file_sha256")?.clone(),
    ));
    Ok(cmd::object(fields))
}
fn intersects(record: &JsonValue, exact: &[&str]) -> SourceCommandResult<bool> {
    Ok(cmd::array(record, "scope_refs")?
        .iter()
        .any(|r| r.as_str().is_some_and(|id| exact.contains(&id))))
}
fn optional_configuration(raw: &[u8]) -> Option<JsonValue> {
    if raw.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return None;
    }
    cmd::parse(raw).ok().filter(|v| v.as_object().is_some())
}
fn retained_configuration(value: &JsonValue) -> bool {
    matches!(
        cmd::text(value, "schema_version").ok(),
        Some(
            "tos_local_text_layer_derive_owner_v1"
                | "tos_local_text_layer_record_owner_ocr_v1"
                | "tos_local_text_layer_record_owner_page_ocr_v1"
        )
    )
}
fn schema_refs(value: &JsonValue, allowed: &[&str], depth: usize) -> SourceCommandResult<()> {
    if depth > 128 {
        return Err(SourceCommandError::Invalid("native grammar nesting budget"));
    }
    match value {
        JsonValue::Object(fields) => {
            for (key, child) in fields {
                if [Some("$ref"), Some("$dynamicRef"), Some("$recursiveRef")]
                    .contains(&key.as_str())
                    && !child
                        .as_str()
                        .is_some_and(|r| r.starts_with('#') || allowed.contains(&r))
                {
                    return Err(SourceCommandError::Unsupported(
                        "native grammar selects undeclared external resource",
                    ));
                }
                schema_refs(child, allowed, depth + 1)?;
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                schema_refs(child, allowed, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
fn ascii_snapshot(value: &JsonValue) -> SourceCommandResult<String> {
    let raw = cmd::canonical(value)?;
    let raw = std::str::from_utf8(&raw)
        .map_err(|_| SourceCommandError::Invalid("native snapshot UTF-8"))?;
    let mut ascii = String::new();
    for ch in raw.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]).iter() {
                use std::fmt::Write;
                write!(&mut ascii, "\\u{unit:04x}")
                    .map_err(|_| SourceCommandError::Invalid("native snapshot emission"))?;
            }
        }
    }
    Ok(Digest256::of_bytes(ascii.as_bytes()).to_prefixed())
}

fn codepoint_span(text: &str, start: u64, end: u64) -> SourceCommandResult<&str> {
    let start = usize::try_from(start)
        .map_err(|_| SourceCommandError::Invalid("native content codepoint start"))?;
    let end = usize::try_from(end)
        .map_err(|_| SourceCommandError::Invalid("native content codepoint end"))?;
    let points = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    if start > end || end >= points.len() {
        return Err(SourceCommandError::Invalid(
            "native text scope leaves exact representation",
        ));
    }
    Ok(&text[points[start]..points[end]])
}
fn check_normalization(text: &str, form: &str) -> SourceCommandResult<()> {
    use unicode_normalization::UnicodeNormalization;
    let same = match form {
        "none" => true,
        "NFC" => text.nfc().eq(text.chars()),
        "NFD" => text.nfd().eq(text.chars()),
        "NFKC" => text.nfkc().eq(text.chars()),
        "NFKD" => text.nfkd().eq(text.chars()),
        _ => {
            return Err(SourceCommandError::Unsupported(
                "native Unicode normalization declaration",
            ));
        }
    };
    if !same {
        return Err(SourceCommandError::Invalid(
            "native text contradicts declared Unicode normalization",
        ));
    }
    Ok(())
}

/// Construct the exact maintained native unit/layer assessment inputs. The
/// caller owns current subject scope/maker/language checks and Sign admission.
pub fn resolve_assessment<R: SignNativeRead + ?Sized>(
    reader: &mut R,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    origin: &str,
    scope: NativeReadScope,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedSignNative> {
    if python_strip_unicode16_v1(origin, MAX_METADATA_FILE_BYTES)
        .map_err(|_| SourceCommandError::Invalid("native assessment origin budget"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native assessment requires explicit evidence origin",
        ));
    }
    let (mut native, packet, layer, summary) =
        selected_binding(reader, worker, binding, scope, deadline, cancelled)?;
    let layer_ref = cmd::object(vec![
        ("id", cmd::field(&layer, "layer_id")?.clone()),
        ("version", cmd::field(&layer, "layer_version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(&layer)?.to_prefixed()),
        ),
    ]);
    // _validate_schema_resource before taking input_snapshot; dependencies are
    // already part of resolve's frozen schema reads and are rechecked below.
    let assessment_name = format!("ToS/contracts/{ASSESSMENT_SCHEMA}");
    let grammar = cmd::parse(&native.read(&assessment_name, None, false, true)?)?;
    if cmd::text(&grammar, "$id")? != format!("https://tree-of-sophia.local/{assessment_name}") {
        return Err(SourceCommandError::Invalid(
            "native assessment schema owner identity",
        ));
    }
    let input_snapshot = native.snapshot()?;
    let payload = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_text_unit_assessment_subject_v1"),
        ),
        ("native_binding", binding.clone()),
        ("packet", packet),
        ("text_layer", layer_ref.clone()),
        (
            "content_verified",
            cmd::field(&summary, "content_verified")?.clone(),
        ),
        ("input_snapshot", cmd::string(&input_snapshot)),
    ]);
    native.validate(&payload, ASSESSMENT_SCHEMA)?;
    if native.snapshot()? != input_snapshot {
        return Err(SourceCommandError::Conflict(
            "native assessment view changed during assembly",
        ));
    }
    let records = [
        cmd::object(vec![
            ("id", cmd::field(binding, "unit_id")?.clone()),
            ("version", cmd::field(binding, "unit_version")?.clone()),
            ("payload", payload),
            ("origin_id", cmd::string(origin)),
        ]),
        cmd::object(vec![
            ("id", cmd::field(&layer_ref, "id")?.clone()),
            ("version", cmd::field(&layer_ref, "version")?.clone()),
            ("payload", layer),
            ("origin_id", cmd::string(origin)),
        ]),
    ];
    for record in &records {
        if cmd::canonical(cmd::field(record, "payload")?)?.len() > MAX_METADATA_FILE_BYTES {
            return Err(SourceCommandError::Invalid(
                "native assessment one-record byte budget",
            ));
        }
    }
    let inputs = selected_inputs(&native);
    Ok(ResolvedSignNative {
        records,
        summary,
        inputs,
        input_snapshot,
        schema_digests: native.schemas,
    })
}

fn selected_binding<'a, R: SignNativeRead + ?Sized>(
    reader: &'a mut R,
    worker: &'a mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    scope: NativeReadScope,
    deadline: Instant,
    cancelled: &'a AtomicBool,
) -> SourceCommandResult<(Native<'a, R>, JsonValue, JsonValue, JsonValue)> {
    let mut native = selected_native(reader, worker, deadline, cancelled)?;
    let (packet, layer, summary) = native.resolve(binding, scope)?;
    Ok((native, packet, layer, summary))
}
fn selected_native<'a, R: SignNativeRead + ?Sized>(
    reader: &'a mut R,
    worker: &'a mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &'a AtomicBool,
) -> SourceCommandResult<Native<'a, R>> {
    reader.verify_current(deadline, cancelled)?;
    Ok(Native {
        reader,
        worker,
        deadline,
        cancelled,
        cache: BTreeMap::new(),
        schemas: BTreeMap::new(),
        remaining_metadata: MAX_METADATA_BYTES,
        remaining_content: MAX_CONTENT_BYTES,
    })
}

/// The maintained SourceRecordProfiles owns one resolver closure across every
/// loaded binding. Reuse the actual resolver cache, quotas and snapshot law.
pub(crate) fn resolve_bindings<R: SignNativeRead + ?Sized>(
    reader: &mut R,
    worker: &mut CutWorkerSchemaExecutor,
    bindings: &[JsonValue],
    scope: NativeReadScope,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedSignBindingBatch> {
    if bindings.is_empty() {
        return Err(SourceCommandError::Invalid(
            "native binding batch requires a used resolver",
        ));
    }
    let mut native = selected_native(reader, worker, deadline, cancelled)?;
    let mut summaries = Vec::new();
    for binding in bindings {
        let (_, _, summary) = native.resolve(binding, scope)?;
        summaries.push(summary);
    }
    let input_snapshot = native.snapshot()?;
    Ok(ResolvedSignBindingBatch {
        summaries,
        inputs: selected_inputs(&native),
        input_snapshot,
        schema_digests: native.schemas,
    })
}

fn selected_inputs<R: SignNativeRead + ?Sized>(native: &Native<'_, R>) -> Vec<NativeInput> {
    native
        .cache
        .iter()
        .map(|((reference, category), input)| NativeInput {
            reference: reference.clone(),
            kind: input.kind,
            category: *category,
            raw_sha256: Digest256::of_bytes(&input.raw),
        })
        .collect()
}

/// The already-used source profile consumer resolves native bindings without
/// adding the assessment-view grammar or changing its owner snapshot inputs.
pub(crate) fn resolve_binding<R: SignNativeRead + ?Sized>(
    reader: &mut R,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    scope: NativeReadScope,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedSignBinding> {
    let (mut native, packet, layer, summary) =
        selected_binding(reader, worker, binding, scope, deadline, cancelled)?;
    let input_snapshot = native.snapshot()?;
    let inputs = selected_inputs(&native);
    Ok(ResolvedSignBinding {
        packet,
        layer,
        summary,
        inputs,
        input_snapshot,
        schema_digests: native.schemas,
    })
}
