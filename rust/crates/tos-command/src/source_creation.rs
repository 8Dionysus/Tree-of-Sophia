//! Maintained initial source package preparation, never source assessment.
//! Source/software custody and package absence are separately established.

use crate::source_command::{self as cmd, *};
use crate::{source_claims as claims, source_forms as forms, source_revisions as revisions};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RULE_INPUTS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_witness_human_forms.py",
    "scripts/build_source_witness_catalog.py",
    "scripts/source_record_profiles.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/source_witness_bibliographic_graph_common.py",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
const CONTRACTS: &[&str] = &[
    "ToS/contracts/historical-record.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/historical-claim.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    ENTITIES,
    RELATIONS,
];
const LINKS: &[&str] = &[
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationFamily {
    HistoricalV1,
    HistoricalV2,
    PublicProfile,
    CorpusV1,
    CorpusV2,
    Sign,
}
impl CreationFamily {
    fn parse(schema: &str) -> SourceCommandResult<Self> {
        Ok(match schema {
            "tos_local_historical_create_owner_v1" => Self::HistoricalV1,
            "tos_local_historical_create_owner_v2" => Self::HistoricalV2,
            "tos_local_profile_create_owner_v1" => Self::PublicProfile,
            "tos_local_corpus_create_owner_v1" => Self::CorpusV1,
            "tos_local_corpus_create_owner_v2" => Self::CorpusV2,
            "tos_local_sign_promote_owner_v1" => Self::Sign,
            _ => return Err(SourceCommandError::Denied("source create owner schema")),
        })
    }
    pub fn handler_id(self) -> &'static str {
        match self {
            Self::HistoricalV1 | Self::HistoricalV2 => "historical-source-create",
            Self::PublicProfile => "public-profile-create",
            Self::CorpusV1 | Self::CorpusV2 => "native-corpus-create",
            Self::Sign => "sign-promotion",
        }
    }
    fn historical(self) -> bool {
        matches!(self, Self::HistoricalV1 | Self::HistoricalV2)
    }
    fn corpus(self) -> bool {
        matches!(self, Self::CorpusV1 | Self::CorpusV2)
    }
    fn operation(self) -> &'static str {
        if self.historical() {
            "historical.create"
        } else if self == Self::Sign {
            "sign.promote"
        } else {
            "source.create"
        }
    }
}

/// Private construction retains exact request/configuration, selected current
/// source/software observations and pending files. It is not a commit grant.
pub struct PreparedCreation {
    context: CommandContext,
    family: CreationFamily,
    home: RelativePath,
    subject: JsonValue,
    dependencies: String,
    files: BTreeMap<String, Vec<u8>>,
    components: SoftwareComponentSelectionV1,
}
/// Actual handler-produced buffer package and in-process observation. The
/// private constructor cannot authenticate execution truth or confer a grant.
pub struct SerializedCreation {
    pub(crate) prepared: PreparedCreation,
    pub(crate) command: PreparedCommand,
}
impl SerializedCreation {
    pub fn command(&self) -> &PreparedCommand {
        &self.command
    }
    pub fn prepared(&self) -> &PreparedCreation {
        &self.prepared
    }
    pub(crate) fn published_result(&self, replayed: bool) -> SourceCommandResult<JsonValue> {
        creation_result(
            &self.prepared,
            true,
            cmd::field(&self.command.response, "receipt")?.clone(),
            replayed,
        )
    }
}
impl PreparedCreation {
    pub fn family(&self) -> CreationFamily {
        self.family
    }
    pub fn home(&self) -> &RelativePath {
        &self.home
    }
    pub fn context(&self) -> &CommandContext {
        &self.context
    }
    pub fn source(&self) -> &JsonValue {
        &self.subject
    }
    pub fn dependencies(&self) -> &str {
        &self.dependencies
    }
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    pub(crate) fn components(&self) -> &SoftwareComponentSelectionV1 {
        &self.components
    }
    pub fn preview(&self) -> SourceCommandResult<JsonValue> {
        let mut value = creation_result(self, false, JsonValue::Null, false)?;
        cmd::set(&mut value, "prepared_source", self.subject.clone())?;
        cmd::set(
            &mut value,
            "expected_dependencies",
            cmd::string(&self.dependencies),
        )?;
        cmd::set(&mut value, "prepared_files", file_refs(&self.files))?;
        cmd::set(
            &mut value,
            "capture_at_apply",
            JsonValue::Array(if self.family == CreationFamily::HistoricalV1 {
                vec![]
            } else {
                [
                    "source-create-request.json",
                    "source-create-environment.json",
                    "source-create-provenance.jsonl",
                ]
                .into_iter()
                .map(cmd::string)
                .collect()
            }),
        )?;
        Ok(value)
    }

    /// Observe native serialization in this process, validate its event and
    /// issue the maintained byte receipt. No caller event/digest is admitted.
    pub fn serialize(
        mut self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<SerializedCreation> {
        if components != &self.components || software.selection() != self.components.capture() {
            return Err(SourceCommandError::Conflict(
                "creation software capture selection changed before serialization",
            ));
        }
        let request = cmd::parse(&cmd::canonical(&cmd::parse(&self.context.request_raw)?)?)?;
        let config = cmd::parse(&self.context.configuration_raw)?;
        if cmd::text(&request, "operation")? != self.family.operation()
            || cmd::text(&request, "command_id")?.is_empty()
            || cmd::text(&request, "command_id")?.chars().count() > 256
        {
            return Err(SourceCommandError::Invalid(
                "creation commit command identity/operation",
            ));
        }
        let configuration = cmd::record_digest(&config)?.to_prefixed();
        if cmd::text(&request, "expected_configuration")? != configuration
            || cmd::field(&request, "expected_source")? != &JsonValue::Null
            || cmd::field(&request, "expected_revision")? != &JsonValue::Null
            || cmd::text(&request, "expected_dependencies")? != self.dependencies
        {
            return Err(SourceCommandError::Conflict(
                "creation configuration/dependencies/absence changed",
            ));
        }
        if self.family != CreationFamily::HistoricalV1 {
            crate::source_serialization::capture_creation(
                &request,
                cmd::text(&config, "provenance_event_id")?,
                self.home.as_str(),
                &mut self.files,
                software,
                components,
                deadline,
                cancelled,
            )?;
            let raw = self.files.get("source-create-provenance.jsonl").unwrap();
            let event = cmd::parse(raw)?;
            revisions::schema(
                worker,
                deadline,
                cancelled,
                &self.context,
                &["ToS/contracts/provenance-event-v2.schema.json".into()],
                "ToS/contracts/provenance-event-v2.schema.json",
                &event,
            )?;
            let event_value: serde_json::Value = serde_json::from_slice(raw)
                .map_err(|_| SourceCommandError::Invalid("native provenance decoded JSON"))?;
            if !tos_validation::provenance_rules::semantic_issues(&event_value, 128, deadline)
                .map_err(|_| SourceCommandError::Invalid("native provenance semantic execution"))?
                .is_empty()
            {
                return Err(SourceCommandError::Invalid(
                    "native provenance violates existing event rules",
                ));
            }
            // Every package entity must name the exact just-serialized bytes;
            // software inputs were independently secure-read inside capture.
            let entities = cmd::field(&event, "entities")?;
            let mut observed = BTreeSet::new();
            for group in ["inputs", "outputs", "byproducts"] {
                for entity in cmd::array(entities, group)? {
                    let location = cmd::text(entity, "entity_ref")?;
                    let name = location
                        .strip_prefix(&format!("{}/", self.home.as_str()))
                        .ok_or(SourceCommandError::Conflict(
                            "native event package entity home",
                        ))?;
                    let bytes = self.files.get(name).ok_or(SourceCommandError::Conflict(
                        "native event package entity absent",
                    ))?;
                    if !observed.insert(name.to_owned())
                        || cmd::integer(entity, "byte_size")? != bytes.len() as u64
                        || cmd::text(entity, "sha256")? != Digest256::of_bytes(bytes).to_hex()
                        || cmd::field(entity, "fixity_verified")? != &JsonValue::Bool(false)
                    {
                        return Err(SourceCommandError::Conflict(
                            "native event package entity byte binding",
                        ));
                    }
                }
            }
            if observed
                != self
                    .files
                    .keys()
                    .filter(|n| n.as_str() != "source-create-provenance.jsonl")
                    .cloned()
                    .collect()
            {
                return Err(SourceCommandError::Conflict(
                    "native serialization event omits output bytes",
                ));
            }
        }
        let refs = file_refs(&self.files);
        let receipt = cmd::object(vec![
            (
                "schema_version",
                cmd::string(if self.family.historical() {
                    "tos_local_historical_create_receipt_v1"
                } else {
                    "tos_local_source_create_receipt_v1"
                }),
            ),
            ("command_id", cmd::field(&request, "command_id")?.clone()),
            (
                "request_digest",
                cmd::string(&cmd::record_digest(&request)?.to_prefixed()),
            ),
            ("principal_id", cmd::field(&config, "principal_id")?.clone()),
            (
                "authority_ref",
                cmd::field(&config, "authority_ref")?.clone(),
            ),
            ("owner_configuration", cmd::string(&configuration)),
            (
                "recorded_at",
                cmd::string(&crate::source_serialization::instant()?),
            ),
            ("source_path", cmd::field(&config, "source_path")?.clone()),
            ("source", self.subject.clone()),
            ("dependencies", cmd::string(&self.dependencies)),
            ("files", refs),
            ("grants_admission", JsonValue::Bool(false)),
        ]);
        let mut raw = cmd::canonical(&receipt)?;
        raw.push(b'\n');
        self.files.insert("source-create-receipt.json".into(), raw);
        if self.files.len() > 40
            || self.files.values().any(|raw| raw.len() > 8_388_608)
            || self
                .files
                .values()
                .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
                .is_none_or(|n| n > 33_554_432)
        {
            return Err(SourceCommandError::Invalid(
                "creation serialized package count/byte budget",
            ));
        }
        let changes = self
            .files
            .iter()
            .map(|(name, raw)| {
                Ok(SourceChange {
                    path: relative(&format!("{}/{name}", self.home.as_str()))?,
                    before: None,
                    after: Some(raw.clone()),
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let command = self.context.plan(
            self.family.handler_id(),
            cmd::object(vec![
                ("receipt", receipt),
                ("replayed", JsonValue::Bool(false)),
                ("grants_admission", JsonValue::Bool(false)),
            ]),
            changes,
            false,
        )?;
        Ok(SerializedCreation {
            prepared: self,
            command,
        })
    }
}

fn creation_result(
    prepared: &PreparedCreation,
    target_exists: bool,
    receipt: JsonValue,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let config = cmd::parse(&prepared.context.configuration_raw)?;
    let family = prepared.family;
    let mut result = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.historical() {
                "tos_local_historical_create_result_v1"
            } else {
                "tos_local_source_create_result_v1"
            }),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&cmd::record_digest(&config)?.to_prefixed()),
        ),
        ("source_path", cmd::field(&config, "source_path")?.clone()),
        ("record_id", cmd::field(&config, "record_id")?.clone()),
        ("target_exists", JsonValue::Bool(target_exists)),
        (
            "supported_operations",
            JsonValue::Array(vec![cmd::string(family.operation())]),
        ),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare", "prepare-create", family.operation()]
                    .into_iter()
                    .map(cmd::string)
                    .collect(),
            ),
        ),
        (
            "allowed_operations",
            cmd::field(&config, "allowed_operations")?.clone(),
        ),
        (
            "allowed_form_ids",
            cmd::field(&config, "allowed_form_ids")?.clone(),
        ),
        ("expected_source", JsonValue::Null),
        ("expected_revision", JsonValue::Null),
        (
            "creation_provenance_event_id",
            config
                .object_get("provenance_event_id")
                .cloned()
                .unwrap_or(JsonValue::Null),
        ),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    if family.historical() {
        cmd::set(
            &mut result,
            "allowed_claim_ids",
            cmd::field(&config, "allowed_claim_ids")?.clone(),
        )?;
        cmd::set(
            &mut result,
            "record_schema_ref",
            cmd::string("ToS/contracts/historical-record.schema.json"),
        )?;
        cmd::set(
            &mut result,
            "claim_schema_ref",
            cmd::string("ToS/contracts/historical-claim.schema.json"),
        )?;
    } else if family.corpus() {
        let kind = cmd::text(&config, "record_type")?;
        cmd::set(&mut result, "record_type", cmd::string(kind))?;
        cmd::set(
            &mut result,
            "source_profile",
            cmd::object(vec![
                ("record_type", cmd::string(kind)),
                ("id_prefix", cmd::string(&format!("tos.{kind}."))),
                ("source_basename", cmd::string(&format!("{kind}.json"))),
                (
                    "schema_ref",
                    cmd::string("ToS/contracts/corpus-record.schema.json"),
                ),
                ("schema_version", cmd::string("tos_corpus_record_v1")),
                ("source_scope", cmd::string("public_metadata_only")),
            ]),
        )?;
    } else {
        let profile_id = cmd::text(&config, "profile_type_id")?;
        let registry = json(&prepared.context, ENTITIES)?;
        let entries: Vec<_> = cmd::array(&registry, "types")?
            .iter()
            .filter(|entry| {
                entry.object_get("type_id").and_then(JsonValue::as_str) == Some(profile_id)
            })
            .collect();
        if entries.len() != 1 {
            return Err(SourceCommandError::Denied(
                "creation result profile not unique",
            ));
        }
        cmd::set(&mut result, "profile_type_id", cmd::string(profile_id))?;
        cmd::set(
            &mut result,
            "source_profile",
            cmd::field(entries[0], "source_record_profile")?.clone(),
        )?;
    }
    Ok(result)
}

fn file_refs(files: &BTreeMap<String, Vec<u8>>) -> JsonValue {
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

fn relative(s: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(s).map_err(|_| SourceCommandError::Invalid("creation source path"))
}
fn selected<'a>(ctx: &'a CommandContext, name: &str) -> SourceCommandResult<&'a [u8]> {
    ctx.file(&relative(name)?)?
        .ok_or(SourceCommandError::Unsupported(
            "required selected creation bytes absent",
        ))
}
fn json(ctx: &CommandContext, name: &str) -> SourceCommandResult<JsonValue> {
    cmd::parse(selected(ctx, name)?)
}
fn contains(v: &JsonValue, key: &str, item: &str) -> SourceCommandResult<bool> {
    Ok(cmd::array(v, key)?.iter().any(|v| v.as_str() == Some(item)))
}
fn bounded_ids(v: &JsonValue, key: &str, prefix: Option<&str>) -> SourceCommandResult<()> {
    let rows = cmd::array(v, key)?;
    let mut seen = BTreeSet::new();
    if rows.len() > 32 {
        return Err(SourceCommandError::Invalid("creation delegated ID count"));
    }
    for row in rows {
        let id = row
            .as_str()
            .ok_or(SourceCommandError::Invalid("creation delegated ID"))?;
        if !seen.insert(id) || prefix.is_some_and(|p| !revisions::valid_id(id, p, p == "tos.form."))
        {
            return Err(SourceCommandError::Invalid("creation delegated ID scope"));
        }
    }
    Ok(())
}
fn configuration(
    ctx: &CommandContext,
) -> SourceCommandResult<(CreationFamily, JsonValue, RelativePath)> {
    ctx.check()?;
    let config = cmd::parse(&ctx.configuration_raw)?;
    let family = CreationFamily::parse(cmd::text(&config, "schema_version")?)?;
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
        "maker_type",
    ];
    if family.historical() {
        keys.push("allowed_claim_ids");
    } else if family.corpus() {
        keys.push("record_type");
    } else {
        keys.push("profile_type_id");
    }
    if family != CreationFamily::HistoricalV1 {
        keys.push("provenance_event_id");
    }
    if family == CreationFamily::Sign {
        keys.extend([
            "promotion_assessment_owner_config",
            "promotion_candidate_id",
        ]);
    }
    cmd::exact_keys(&config, &keys)?;
    if cmd::integer(&config, "uid")? != ctx.effective_uid
        || tos_foundation::python_strip_unicode16_v1(cmd::text(&config, "principal_id")?, 1_048_576)
            .map_err(|_| SourceCommandError::Invalid("creation principal Unicode budget"))?
            .is_empty()
        || tos_foundation::python_strip_unicode16_v1(
            cmd::text(&config, "authority_ref")?,
            1_048_576,
        )
        .map_err(|_| SourceCommandError::Invalid("creation authority Unicode budget"))?
        .is_empty()
    {
        return Err(SourceCommandError::Denied(
            "creation current account/principal",
        ));
    }
    cmd::validate_expiry(cmd::text(&config, "expires_at")?, &ctx.recorded_at)?;
    if !["human", "software", "model"].contains(&cmd::text(&config, "maker_type")?) {
        return Err(SourceCommandError::Denied("creation declared maker kind"));
    }
    bounded_ids(&config, "allowed_form_ids", Some("tos.form."))?;
    bounded_ids(&config, "allowed_operations", None)?;
    if cmd::array(&config, "allowed_operations")?
        .iter()
        .any(|v| v.as_str() != Some(family.operation()))
    {
        return Err(SourceCommandError::Denied("creation operation scope"));
    }
    if family.historical() {
        bounded_ids(&config, "allowed_claim_ids", Some("tos.claim."))?;
    }
    if family != CreationFamily::HistoricalV1
        && !revisions::valid_id(
            cmd::text(&config, "provenance_event_id")?,
            "tos.event.",
            false,
        )
    {
        return Err(SourceCommandError::Invalid("creation provenance identity"));
    }
    let source = cmd::text(&config, "source_path")?;
    relative(source)?;
    let parts: Vec<_> = source.split('/').collect();
    if parts.len() < 5
        || !source.starts_with("ToS/source-witnesses/")
        || !source.ends_with(".json")
        || source.ends_with(".human-forms.json")
        || parts
            .iter()
            .any(|p| matches!(*p, "owner-local" | "payload" | "local-content" | "catalog"))
    {
        return Err(SourceCommandError::Denied(
            "creation exact public source home",
        ));
    }
    let home = relative(source.rsplit_once('/').unwrap().0)?;
    Ok((family, config, home))
}

fn initial(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    config: &JsonValue,
    family: CreationFamily,
    record: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<String>> {
    let mut profile_resources = Vec::new();
    if cmd::text(record, "record_id")? != cmd::text(config, "record_id")?
        || cmd::integer(record, "record_version")? != 1
        || record
            .object_get("supersedes_ref")
            .is_some_and(|v| v != &JsonValue::Null)
        || cmd::text(record, "identity_status")? != "provisional"
        || cmd::text(record, "same_as_posture")? != "no_equivalence_claim"
    {
        return Err(SourceCommandError::Denied(
            "creation delegated provisional initial identity",
        ));
    }
    let kind = cmd::text(record, "record_type")?;
    let path = cmd::text(config, "source_path")?;
    if family.corpus() {
        let allowed = if family == CreationFamily::CorpusV2 {
            &["agent", "place", "organization", "work", "collection"][..]
        } else {
            &["agent", "place", "organization", "work"][..]
        };
        if !allowed.contains(&kind)
            || cmd::text(config, "record_type")? != kind
            || !path.ends_with(&format!("/{kind}.json"))
            || !revisions::valid_id(
                cmd::text(record, "record_id")?,
                &format!("tos.{kind}."),
                false,
            )
            || kind == "collection" && !path.starts_with("ToS/source-witnesses/collections/")
            || kind == "work" && path.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")
        {
            return Err(SourceCommandError::Denied(
                "creation corpus type/path grant or stronger Nietzsche home",
            ));
        }
        for link in LINKS {
            if record.object_get(link).is_some() {
                if (kind == "work" && *link == "expression_claim_refs"
                    || kind == "collection" && *link == "membership_claim_refs")
                    && cmd::array(record, link)?.is_empty()
                {
                    continue;
                }
                return Err(SourceCommandError::Denied(
                    "initial standalone identity cannot assert links",
                ));
            }
        }
        if ["work", "collection"].contains(&kind) {
            let allowed = [
                "schema_version",
                "record_type",
                "record_id",
                "record_version",
                "preferred_label",
                "variant_labels",
                "field_languages",
                "identity_status",
                "source_refs",
                "external_identifiers",
                "same_as_posture",
                "notes",
                "supersedes_ref",
                if kind == "work" {
                    "expression_claim_refs"
                } else {
                    "membership_claim_refs"
                },
            ];
            if record
                .as_object()
                .ok_or(SourceCommandError::Invalid("creation record object"))?
                .iter()
                .any(|(k, _)| !allowed.contains(&k.as_str().unwrap_or("")))
            {
                return Err(SourceCommandError::Denied(
                    "Work/Collection creation descriptive fields only",
                ));
            }
            cmd::array(
                record,
                if kind == "work" {
                    "expression_claim_refs"
                } else {
                    "membership_claim_refs"
                },
            )?;
        }
        for section in ["variant_labels", "external_identifiers"] {
            if let Some(rows) = record.object_get(section) {
                for row in rows
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("creation identity assertions"))?
                {
                    if cmd::text(row, "status")? != "unverified" {
                        return Err(SourceCommandError::Denied(
                            "initial identity has accepted attribution",
                        ));
                    }
                }
            }
        }
        revisions::schema(
            worker,
            deadline,
            cancelled,
            ctx,
            &["ToS/contracts/corpus-record.schema.json".into()],
            "ToS/contracts/corpus-record.schema.json",
            record,
        )?;
    } else if family.historical() {
        if !["historical-event", "historical-process", "historical-state"].contains(&kind)
            || !path.ends_with(&format!("/{kind}.json"))
            || !revisions::valid_id(
                cmd::text(record, "record_id")?,
                &format!("tos.{kind}."),
                false,
            )
            || !["public", "public_metadata_only"].contains(&cmd::text(record, "visibility")?)
        {
            return Err(SourceCommandError::Denied(
                "historical creation typed public identity",
            ));
        }
        revisions::schema(
            worker,
            deadline,
            cancelled,
            ctx,
            &[
                "ToS/contracts/corpus-record.schema.json".into(),
                "ToS/contracts/historical-record.schema.json".into(),
            ],
            "ToS/contracts/historical-record.schema.json",
            record,
        )?;
    } else {
        let (profile, resources, _, _) =
            revisions::public_profile(Some(cut), worker, deadline, cancelled, ctx, config, record)?;
        profile_resources = resources;
        if profile.object_get("creation_gate").is_some() && family != CreationFamily::Sign {
            return Err(SourceCommandError::Denied(
                "profile requires explicit Sign promotion gate",
            ));
        }
        if family == CreationFamily::Sign
            && cmd::text(&profile, "creation_gate")? != "sign-promotion-v1"
        {
            return Err(SourceCommandError::Denied(
                "Sign requires exact declared creation gate",
            ));
        }
        // Metadata-only revision validation cannot substitute for creation's
        // mandatory exact public representation read. Filled by the same
        // native resolver's content path, not an admission boolean.
        if profile.object_get("native_binding_adapter").is_some() {
            return Err(SourceCommandError::Unsupported(
                "creation exact native content verification pending resolver content route",
            ));
        }
    }
    Ok(profile_resources)
}

fn historical_claims(
    ctx: &CommandContext,
    config: &JsonValue,
    record: &JsonValue,
    rows: &[JsonValue],
    inventory: &claims::MaintainedInventory,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    evidence: &mut Vec<JsonValue>,
) -> SourceCommandResult<Vec<u8>> {
    let registry = json(ctx, RELATIONS)?;
    let entities = json(ctx, ENTITIES)?;
    let types = cmd::array(&entities, "types")?;
    let mut objects = inventory.objects.clone();
    objects.insert(
        cmd::text(record, "record_id")?.into(),
        cmd::object(vec![
            ("record_type", cmd::field(record, "record_type")?.clone()),
            (
                "source_record_ref",
                cmd::field(config, "source_path")?.clone(),
            ),
            (
                "record_sha256",
                cmd::string(&cmd::record_digest(record)?.to_hex()),
            ),
        ]),
    );
    let mut ids = inventory.claims.keys().cloned().collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    for claim in rows {
        let id = cmd::text(claim, "claim_id")?;
        let maker = cmd::field(claim, "maker")?;
        if !contains(config, "allowed_claim_ids", id)?
            || cmd::text(claim, "subject_ref")? != cmd::text(record, "record_id")?
            || cmd::text(maker, "agent_ref")? != cmd::text(config, "principal_id")?
            || cmd::text(maker, "maker_type")? != cmd::text(config, "maker_type")?
            || !["public", "public_metadata_only"].contains(&cmd::text(claim, "visibility")?)
            || cmd::integer(claim, "claim_version")? != 1
            || claim
                .object_get("supersedes_claim_ref")
                .is_some_and(|v| v != &JsonValue::Null)
            || claim
                .object_get("assessment_refs")
                .is_some_and(|v| v.as_array().is_none_or(|v| !v.is_empty()))
        {
            return Err(SourceCommandError::Denied("initial historical Claim scope"));
        }
        if !ids.insert(id.into()) {
            return Err(SourceCommandError::Conflict(
                "historical Claim identity exists or repeats",
            ));
        }
        revisions::schema(
            worker,
            deadline,
            cancelled,
            ctx,
            &[
                "ToS/contracts/historical-claim.schema.json".into(),
                "ToS/contracts/claim-packet.schema.json".into(),
                "ToS/contracts/knowledge-assessment.schema.json".into(),
            ],
            "ToS/contracts/historical-claim.schema.json",
            claim,
        )?;
        let predicate = cmd::text(claim, "predicate")?;
        if ![
            "historical_participant",
            "historical_place",
            "historical_work",
            "historical_dating",
        ]
        .contains(&predicate)
        {
            return Err(SourceCommandError::Denied("historical predicate owner"));
        }
        let relations = cmd::array(&registry, "relations")?
            .iter()
            .filter(|r| {
                cmd::array(r, "source_mappings").is_ok_and(|ms| {
                    ms.iter().any(|m| {
                        cmd::text(m, "source_graph").ok() == Some("source-claims")
                            && cmd::text(m, "scope").ok() == Some("claim-predicate")
                            && cmd::text(m, "source_predicate_id").ok() == Some(predicate)
                    })
                })
            })
            .collect::<Vec<_>>();
        if relations.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "historical predicate registry mapping",
            ));
        }
        for (field, scope) in [
            ("subject_ref", "domain_type_ids"),
            ("object", "range_type_ids"),
        ] {
            let type_id =
                if field == "object" && predicate == "historical_dating" {
                    if let Some(anchor) = claim
                        .object_get("object")
                        .and_then(|v| v.object_get("relative"))
                        .and_then(|v| v.object_get("anchor_ref"))
                        .and_then(JsonValue::as_str)
                    {
                        if !objects.get(anchor).is_some_and(|r| {
                            cmd::text(r, "record_type").is_ok_and(|k| {
                                ["historical-event", "historical-process", "historical-state"]
                                    .contains(&k)
                            })
                        }) {
                            return Err(SourceCommandError::Invalid(
                                "historical date anchor unresolved",
                            ));
                        }
                    }
                    "tos.entity.temporal-assertion"
                } else {
                    let object = objects.get(cmd::text(claim, field)?).ok_or(
                        SourceCommandError::Invalid("historical endpoint unresolved"),
                    )?;
                    let kind = cmd::text(object, "record_type")?;
                    let mapped = types
                        .iter()
                        .filter(|t| {
                            cmd::array(t, "source_mappings").is_ok_and(|ms| {
                                ms.iter().any(|m| {
                                    cmd::text(m, "source_graph").ok() == Some("source-claims")
                                        && cmd::text(m, "source_kind_id").ok() == Some(kind)
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
            let ancestry = claims::ancestry(types, type_id)?;
            if !cmd::array(relations[0], scope)?
                .iter()
                .any(|v| v.as_str().is_some_and(|id| ancestry.contains(id)))
            {
                return Err(SourceCommandError::Denied(
                    "historical registry domain/range",
                ));
            }
        }
        if claim
            .object_get("qualifiers")
            .and_then(|v| v.object_get("display_fields"))
            .and_then(|v| v.object_get("schema_version"))
            .and_then(JsonValue::as_str)
            == Some("tos_claim_display_fields_v1")
        {
            revisions::schema(
                worker,
                deadline,
                cancelled,
                ctx,
                &[
                    "ToS/contracts/corpus-record.schema.json".into(),
                    "ToS/contracts/claim-display-fields.schema.json".into(),
                ],
                "ToS/contracts/claim-display-fields.schema.json",
                cmd::field(claim, "qualifiers")?,
            )?;
        }
        let event = cmd::text(claim, "provenance_event_ref")?;
        if let Some(new) = config.object_get("provenance_event_id") {
            if new.as_str() != Some(event) {
                return Err(SourceCommandError::Denied(
                    "historical Claim delegated creation event",
                ));
            }
        }
        if inventory.events.object_get(event).is_none()
            && config
                .object_get("provenance_event_id")
                .and_then(JsonValue::as_str)
                != Some(event)
        {
            return Err(SourceCommandError::Invalid(
                "historical Claim event unresolved",
            ));
        }
        for key in ["evidence_refs", "counterevidence_refs"] {
            let Some(refs) = claim.object_get(key) else {
                continue;
            };
            for reference in refs
                .as_array()
                .ok_or(SourceCommandError::Invalid("historical evidence array"))?
            {
                let name = reference
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("historical evidence ref"))?;
                if name.starts_with("ToS/")
                    && name
                        .split('/')
                        .any(|part| matches!(part, "payload" | "local-content"))
                {
                    return Err(SourceCommandError::Denied(
                        "historical evidence addresses private content",
                    ));
                }
                evidence.push(claims::maintained_evidence(
                    ctx,
                    name,
                    &objects,
                    &inventory.anchors,
                    &inventory.events,
                )?);
            }
        }
        output.extend(cmd::canonical(claim)?);
        output.push(b'\n');
    }
    for claim in rows {
        if let Some(refs) = claim.object_get("alternative_claim_refs") {
            if refs
                .as_array()
                .ok_or(SourceCommandError::Invalid("historical alternative Claims"))?
                .iter()
                .any(|v| v.as_str().is_none_or(|id| !ids.contains(id)))
            {
                return Err(SourceCommandError::Invalid(
                    "historical alternative Claim unresolved",
                ));
            }
        }
    }
    Ok(output)
}

pub fn prepare_source_creation_from_captures(
    context: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCreation> {
    prepare_creation(
        context, cut, software, components, worker, deadline, cancelled, None,
    )
}

/// Sign has a distinct current owner read, using the actual protected journal
/// and native content inputs rather than a caller-issued promotion verdict.
pub fn prepare_sign_promotion_from_captures(
    configuration_path: &std::path::Path,
    context: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    local_worker: &mut CutWorkerSchemaExecutor,
    assessment_worker: &mut CutWorkerSchemaExecutor,
    limits: tos_validation::assessment::AssessmentLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCreation> {
    context.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let (family, config, home) = configuration(context)?;
    if family != CreationFamily::Sign || !contains(&config, "allowed_operations", "sign.promote")? {
        return Err(SourceCommandError::Denied(
            "current Sign operation not delegated",
        ));
    }
    let request = cmd::parse(&context.request_raw)?;
    if !matches!(
        cmd::text(&request, "operation")?,
        "prepare-create" | "sign.promote"
    ) {
        return Err(SourceCommandError::Invalid("Sign preparation operation"));
    }
    if cut.current().members().any(|member| {
        member.path.as_str() == home.as_str()
            || member
                .path
                .as_str()
                .starts_with(&format!("{}/", home.as_str()))
    }) {
        return Err(SourceCommandError::Conflict(
            "Sign source home already occupied",
        ));
    }
    let mut selected = crate::source_sign::SignPromotionRead::select(
        configuration_path,
        context,
        cut,
        limits.deadline,
        cancelled,
    )?;
    let basis =
        selected.current_basis(context, local_worker, assessment_worker, limits, cancelled)?;
    prepare_creation(
        context,
        cut,
        software,
        components,
        local_worker,
        limits.deadline,
        cancelled,
        Some(&basis),
    )
}

fn prepare_creation(
    context: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    promotion: Option<&JsonValue>,
) -> SourceCommandResult<PreparedCreation> {
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let mut ctx = context.clone();
    let software_files = ctx
        .files
        .iter()
        .filter(|f| !f.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    ctx.files = claims::complete_authored_inputs(context, cut, deadline, cancelled)?;
    ctx.files.extend(software_files);
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let (family, config, home) = configuration(&ctx)?;
    revisions::validate_source_profile_registry(worker, deadline, cancelled, &ctx)?;
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&ctx.request_raw)?)?)?;
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Invalid(
            "creation package preparation operation",
        ));
    }
    let mut keys = vec!["schema_version", "operation", "record", "forms"];
    if family.historical() {
        keys.push("claims");
    }
    if operation == family.operation() {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid("creation command schema"));
    }
    if !contains(&config, "allowed_operations", family.operation())? {
        return Err(SourceCommandError::Denied(
            "creation operation not delegated",
        ));
    }
    let record = cmd::field(&request, "record")?;
    if family == CreationFamily::Sign {
        let basis = promotion.ok_or(SourceCommandError::Unsupported(
            "Sign requires actual protected current assessment reader",
        ))?;
        if cmd::text(&config, "profile_type_id")? != "tos.entity.sign"
            || !revisions::valid_id(
                cmd::text(&config, "promotion_candidate_id")?,
                "tos.claim.",
                false,
            )
            || !cmd::same(cmd::field(record, "promotion_basis")?, basis)?
        {
            return Err(SourceCommandError::Conflict(
                "Sign description must retain exact current promotion basis",
            ));
        }
    } else if promotion.is_some() {
        return Err(SourceCommandError::Denied(
            "Sign reader cannot delegate another creation family",
        ));
    }
    let profile_resources = initial(
        &ctx, cut, worker, &config, family, record, deadline, cancelled,
    )?;
    if cut.current().members().any(|m| {
        m.path.as_str() == home.as_str()
            || m.path.as_str().starts_with(&format!("{}/", home.as_str()))
    }) {
        return Err(SourceCommandError::Conflict(
            "creation source home already occupied",
        ));
    }
    let mut inventory = claims::maintained_inventory(&ctx, worker, deadline, cancelled)?;
    if inventory
        .objects
        .contains_key(cmd::text(record, "record_id")?)
    {
        return Err(SourceCommandError::Conflict(
            "source identity exists in authored inventory",
        ));
    }
    if family == CreationFamily::Sign {
        for file in &ctx.files {
            if file.path.as_str().starts_with("ToS/source-witnesses/")
                && file.path.as_str().ends_with("/sign.json")
            {
                let prior = cmd::parse(&file.raw)?;
                if prior
                    .object_get("promotion_basis")
                    .and_then(|b| b.object_get("candidate"))
                    .and_then(|r| r.object_get("id"))
                    == config.object_get("promotion_candidate_id")
                {
                    return Err(SourceCommandError::Conflict(
                        "Sign candidate already issued in selected source cohort",
                    ));
                }
            }
        }
    }
    if family != CreationFamily::HistoricalV1
        && inventory
            .events
            .object_get(cmd::text(&config, "provenance_event_id")?)
            .is_some()
    {
        return Err(SourceCommandError::Conflict(
            "creation event identity exists",
        ));
    }
    let selections = cmd::array(&request, "forms")?;
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "creation form selections budget",
        ));
    }
    let initial_claims = request
        .object_get("claims")
        .map(|v| {
            v.as_array()
                .ok_or(SourceCommandError::Invalid("creation claims array"))
        })
        .transpose()?
        .unwrap_or(&[]);
    if initial_claims.len() > 32 || !family.historical() && !initial_claims.is_empty() {
        return Err(SourceCommandError::Denied(
            "initial Claims belong only to historical grant",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut changes = Vec::new();
    for selection in selections {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        let id = cmd::text(selection, "form_id")?;
        if !contains(&config, "allowed_form_ids", id)? || !seen.insert(id) {
            return Err(SourceCommandError::Denied(
                "creation form identity grant or repeated ID",
            ));
        }
        changes.push(forms::prepare_form_change(
            record,
            None,
            cmd::text(&config, "principal_id")?,
            id,
            cmd::text(selection, "field_id")?,
        )?);
    }
    let subject = forms::metadata_subject(record)?;
    let set = forms::apply_form_changes(None, &subject, &changes)?;
    let views = forms::materialize_source_forms(record, &set)?;
    if views
        .iter()
        .any(|v| cmd::text(v, "state").ok() != Some("ready"))
        || !views
            .iter()
            .any(|v| cmd::text(v, "role").ok() == Some("name"))
    {
        return Err(SourceCommandError::Invalid(
            "creation requires ready source-copy name form",
        ));
    }
    revisions::schema(
        worker,
        deadline,
        cancelled,
        &ctx,
        &[
            "ToS/contracts/human-form.schema.json".into(),
            "ToS/contracts/human-form-set.schema.json".into(),
            "ToS/contracts/human-form-template.schema.json".into(),
        ],
        "ToS/contracts/human-form-set.schema.json",
        &set,
    )?;
    let mut form_inputs = cmd::object(vec![]);
    let mut form_paths = BTreeSet::new();
    for entry in inventory.objects.values() {
        let name = cmd::text(entry, "source_record_ref")?;
        form_paths.insert(format!(
            "{}.human-forms.json",
            name.strip_suffix(".json")
                .ok_or(SourceCommandError::Invalid("catalog form record path"))?
        ));
    }
    for entry in inventory.claims.values() {
        let name = cmd::text(entry, "source_claim_file_ref")?;
        if name.ends_with("/source-claims.jsonl") || name.ends_with("/historical-claims.jsonl") {
            let (h, f) = name.rsplit_once('/').unwrap();
            form_paths.insert(format!(
                "{h}/{}.{}.human-forms.json",
                f.strip_suffix(".jsonl").unwrap(),
                Digest256::of_bytes(cmd::text(entry, "claim_id")?.as_bytes()).to_hex()
            ));
        }
    }
    for name in form_paths {
        let Some(raw) = ctx.file(&relative(&name)?)? else {
            continue;
        };
        let prior = cmd::parse(raw)?;
        forms::apply_form_changes(Some(&prior), cmd::field(&prior, "subject")?, &[])?;
        revisions::schema(
            worker,
            deadline,
            cancelled,
            &ctx,
            &["ToS/contracts/human-form-set.schema.json".into()],
            "ToS/contracts/human-form-set.schema.json",
            &prior,
        )?;
        for section in ["forms", "prior_forms"] {
            for form in cmd::array(&prior, section)? {
                if seen.contains(cmd::text(form, "form_id")?) {
                    return Err(SourceCommandError::Conflict(
                        "form identity already allocated on another subject",
                    ));
                }
            }
        }
        cmd::set(
            &mut form_inputs,
            &name,
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        )?;
    }
    let source_path = cmd::text(&config, "source_path")?;
    let filename = source_path.rsplit('/').next().unwrap();
    let mut files = BTreeMap::from([
        (filename.to_string(), cmd::published(record)?),
        (
            format!(
                "{}.human-forms.json",
                filename.strip_suffix(".json").unwrap()
            ),
            cmd::published(&set)?,
        ),
    ]);
    let mut evidence = Vec::new();
    if family.historical() {
        let raw = historical_claims(
            &ctx,
            &config,
            record,
            initial_claims,
            &inventory,
            worker,
            deadline,
            cancelled,
            &mut evidence,
        )?;
        files.insert("historical-claims.jsonl".into(), raw);
    }
    for name in profile_resources {
        cmd::set(
            &mut inventory.record_inputs,
            &name,
            cmd::string(&Digest256::of_bytes(selected(&ctx, &name)?).to_hex()),
        )?;
    }
    let source_profiles = inventory.record_inputs;
    let empty_native = Digest256::of_bytes(b"{}").to_prefixed();
    let provenance_contract = if family == CreationFamily::HistoricalV1 {
        cmd::object(vec![])
    } else {
        claims::raw_digests(
            &ctx,
            &["ToS/contracts/provenance-event-v2.schema.json"],
            true,
        )?
    };
    let snapshot = cmd::object(vec![
        ("records", inventory.records),
        (
            "claims",
            JsonValue::Array(inventory.claims.into_values().collect()),
        ),
        ("source_profiles", source_profiles),
        (
            "native_semantic_identity_snapshot",
            cmd::string(&empty_native),
        ),
        ("native_text_binding_snapshot", JsonValue::Null),
        ("source_claim_profiles", inventory.claim_profile_inputs),
        ("provenance_contract", provenance_contract),
        ("events", inventory.events),
        ("anchors", inventory.anchors),
        ("evidence", JsonValue::Array(evidence)),
        ("forms", form_inputs),
        ("contracts", claims::raw_digests(&ctx, CONTRACTS, true)?),
        (
            "implementation",
            claims::raw_digests(&ctx, RULE_INPUTS, true)?,
        ),
    ]);
    let dependencies = cmd::record_digest(&snapshot)?.to_prefixed();
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(SourceCommandError::Denied(
            "creation deadline or cancellation",
        ));
    }
    Ok(PreparedCreation {
        context: ctx,
        family,
        home,
        subject,
        dependencies,
        files,
        components: components.clone(),
    })
}
