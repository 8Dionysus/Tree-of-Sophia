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
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-validation/src/assessment.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-command/src/source_private_claim.rs",
    "rust/crates/tos-command/src/source_private_owner_store.rs",
    "rust/crates/tos-command/src/source_text_owner.rs",
    "rust/crates/tos-command/src/source_corpus_index_projection.rs",
    "rust/crates/tos-compiler/src/source_bibliographic.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
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
    Artifact,
}
impl CreationFamily {
    pub(crate) fn parse(schema: &str) -> SourceCommandResult<Self> {
        Ok(match schema {
            "tos_local_historical_create_owner_v1" => Self::HistoricalV1,
            "tos_local_historical_create_owner_v2" => Self::HistoricalV2,
            "tos_local_profile_create_owner_v1" => Self::PublicProfile,
            "tos_local_corpus_create_owner_v1" => Self::CorpusV1,
            "tos_local_corpus_create_owner_v2" => Self::CorpusV2,
            "tos_local_sign_promote_owner_v1" => Self::Sign,
            "tos_local_artifact_create_owner_v1" => Self::Artifact,
            _ => return Err(SourceCommandError::Denied("source create owner schema")),
        })
    }
    pub fn handler_id(self) -> &'static str {
        match self {
            Self::HistoricalV1 | Self::HistoricalV2 => "historical-source-create",
            Self::PublicProfile => "public-profile-create",
            Self::CorpusV1 | Self::CorpusV2 => "native-corpus-create",
            Self::Sign => "sign-promotion",
            Self::Artifact => "native-artifact-create",
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

/// Concrete controlled generation identity, distinct from schema SourceRevision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedCreationBasis {
    pub domain: String,
    pub digest: Digest256,
    pub generation: u64,
    pub epoch: u64,
    pub definition: Digest256,
}
impl ManagedCreationBasis {
    pub(crate) fn from_generation(
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
    ) -> Self {
        Self {
            domain: generation.cohort().domain().into(),
            digest: generation.digest(),
            generation: generation.commit_seq(),
            epoch: generation.epoch(),
            definition: generation.agent_definition_digest(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedCreationObservation {
    pub metadata: tos_source_store::MemberMetadata,
    pub dependencies: Option<Vec<RelativePath>>,
    pub custody_revision: u64,
    pub commit_seq: u64,
}
/// Private-issued complete Agent input; the context revision binds schemas only.
pub struct ManagedCreationInput {
    pub(crate) context: CommandContext,
    pub(crate) basis: ManagedCreationBasis,
    pub(crate) observations: BTreeMap<String, ManagedCreationObservation>,
    pub(crate) components: SoftwareComponentSelectionV1,
    pub(crate) inventory: Option<crate::source_cohort::ManagedAgentInventory>,
}
pub fn select_managed_agent_creation_input(
    coordinator: &mut crate::durable_adapter::DurablePgCoordinator,
    store: &tos_segment_store::SegmentStore,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
    context: &CommandContext,
    schema_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ManagedCreationInput> {
    let mut selected = context.clone();
    selected
        .files
        .retain(|file| !file.path.as_str().starts_with("ToS/"));
    // Existing v1 check is used only for separately selected software/schema
    // context here; no generation-authored instance passes its equality gate.
    selected.check_from_selected_captures(schema_cut, software, components, deadline, cancelled)?;
    let mut total = selected
        .files
        .iter()
        .map(|file| file.raw.len())
        .sum::<usize>();
    let mut observations = BTreeMap::new();
    // Schema resources retain their independently selected v1 meaning. Only
    // these resources and addressed request dependencies are read as bodies.
    let mut paths = schema_cut
        .current()
        .members()
        .filter(|member| {
            member.path.as_str().starts_with("ToS/contracts/")
                || [ENTITIES, RELATIONS].contains(&member.path.as_str())
        })
        .map(|member| member.path.clone())
        .collect::<BTreeSet<_>>();
    let request = cmd::parse(&context.request_raw)?;
    let record = cmd::field(&request, "record")?;
    if cmd::text(record, "record_type")? != "agent" {
        return Err(SourceCommandError::Unsupported(
            "managed creation consumer is initial native Agent only",
        ));
    }
    for reference in cmd::array(record, "source_refs")? {
        let name = reference
            .as_str()
            .ok_or(SourceCommandError::Invalid("Agent source reference"))?;
        if name.starts_with("ToS/") {
            let path = relative(name)?;
            // Literal source references are not invented endpoint constraints.
            // Existing addressed members are read; absence is bound by the
            // complete inventory generation used below and at registration.
            if generation
                .member(coordinator, store, &path, deadline, cancelled)?
                .is_some()
            {
                paths.insert(path);
            }
        }
    }
    while let Some(path) = paths.pop_first() {
        if observations.contains_key(path.as_str()) {
            continue;
        }
        if path
            .as_str()
            .split('/')
            .any(|part| ["owner-local", "payload", "local-content"].contains(&part))
        {
            return Err(SourceCommandError::Unsupported(
                "managed Agent addressed input requires private/native reader",
            ));
        }
        let member = generation
            .member(coordinator, store, &path, deadline, cancelled)?
            .ok_or(SourceCommandError::Conflict(
                "managed schema resource absent",
            ))?;
        if selected.files.len() >= SELECTED_SOURCE_MAX_FILES || member.size_bytes > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "managed Agent addressed input budget",
            ));
        }
        total = total
            .checked_add(
                usize::try_from(member.size_bytes)
                    .map_err(|_| SourceCommandError::Invalid("managed member size"))?,
            )
            .filter(|bytes| *bytes <= SELECTED_SOURCE_MAX_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "managed Agent addressed input byte budget",
            ))?;
        let observed = generation.read_current_member(
            coordinator,
            store,
            &path,
            8_388_608,
            deadline,
            cancelled,
        )?;
        if observed.metadata != member || observed.current_generation != generation.commit_seq() {
            return Err(SourceCommandError::Conflict(
                "managed Agent selected observation differs",
            ));
        }
        if path.as_str().starts_with("ToS/contracts/")
            || [ENTITIES, RELATIONS].contains(&path.as_str())
        {
            let expected = schema_cut
                .read_member(
                    schema_cut.current().revision(),
                    &path,
                    8_388_608,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Conflict("independent schema resource unavailable")
                })?;
            if observed.raw != expected.raw {
                return Err(SourceCommandError::Conflict(
                    "managed resource differs from independent schema cut",
                ));
            }
        }
        for dependency in observed.dependency_claims.as_deref().unwrap_or(&[]) {
            if generation
                .member(coordinator, store, dependency, deadline, cancelled)?
                .is_none()
            {
                return Err(SourceCommandError::Conflict(
                    "managed addressed dependency absent",
                ));
            }
            if !observations.contains_key(dependency.as_str()) {
                paths.insert(dependency.clone());
            }
        }
        observations.insert(
            path.as_str().into(),
            ManagedCreationObservation {
                metadata: observed.metadata,
                dependencies: observed.dependency_claims,
                custody_revision: observed.custody_revision,
                commit_seq: observed.commit_seq,
            },
        );
        selected.files.push(SourceFile {
            path,
            raw: observed.raw,
        });
    }
    selected.files.sort_by(|a, b| a.path.cmp(&b.path));
    selected.check()?;
    let inventory = coordinator
        .select_agent_inventory(generation, &selected, deadline, cancelled)
        .map_err(crate::source_current_cut::durable)?;
    Ok(ManagedCreationInput {
        context: selected,
        basis: ManagedCreationBasis::from_generation(generation),
        observations,
        components: components.clone(),
        inventory: Some(inventory),
    })
}

/// The managed wrapper shares the actual creation payload, never a v1 command.
pub struct ManagedPreparedCreation {
    prepared: PreparedCreation,
}
pub struct ManagedSerializedCreation {
    pub(crate) prepared: PreparedCreation,
    pub(crate) plan: cmd::CommandPlan,
    pub(crate) basis: ManagedCreationBasis,
}
/// Borrowed actual payload shared by the two concrete registration consumers.
#[derive(Clone, Copy)]
pub(crate) enum CreationPackage<'a> {
    V1(&'a SerializedCreation),
    Managed(&'a ManagedSerializedCreation),
}
impl<'a> CreationPackage<'a> {
    pub(crate) fn prepared(self) -> &'a PreparedCreation {
        match self {
            Self::V1(p) => &p.prepared,
            Self::Managed(p) => &p.prepared,
        }
    }
    pub(crate) fn reads(self) -> &'a [SourceDependency] {
        match self {
            Self::V1(p) => &p.command.reads,
            Self::Managed(p) => &p.plan.reads,
        }
    }
    pub(crate) fn changes(self) -> &'a [SourceChange] {
        match self {
            Self::V1(p) => &p.command.changes,
            Self::Managed(p) => &p.plan.changes,
        }
    }
    pub(crate) fn handler(self) -> &'a str {
        match self {
            Self::V1(p) => &p.command.handler_id,
            Self::Managed(p) => &p.plan.handler_id,
        }
    }
    pub(crate) fn operation(self) -> &'a str {
        match self {
            Self::V1(p) => &p.command.operation,
            Self::Managed(p) => &p.plan.operation,
        }
    }
    pub(crate) fn configuration_digest(self) -> Digest256 {
        match self {
            Self::V1(p) => p.command.configuration_raw_sha256,
            Self::Managed(p) => p.plan.configuration_raw_sha256,
        }
    }
    pub(crate) fn v1_revision(self) -> Option<tos_foundation::SourceRevision> {
        match self {
            Self::V1(p) => Some(p.command.base_revision),
            Self::Managed(_) => None,
        }
    }
    pub(crate) fn observations(self) -> Option<&'a BTreeMap<String, ManagedCreationObservation>> {
        self.prepared().managed_observations.as_ref()
    }
    pub(crate) fn inventory(self) -> Option<&'a crate::source_cohort::ManagedAgentInventory> {
        self.prepared().managed_inventory.as_ref()
    }
    pub(crate) fn managed_basis(self) -> Option<&'a ManagedCreationBasis> {
        match self {
            Self::V1(_) => None,
            Self::Managed(p) => Some(&p.basis),
        }
    }
}

impl ManagedPreparedCreation {
    pub fn preview(&self) -> SourceCommandResult<JsonValue> {
        self.prepared.preview()
    }
    pub fn serialize(
        self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<ManagedSerializedCreation> {
        let (prepared, plan) = self
            .prepared
            .serialize_content(software, components, worker, deadline, cancelled, None)?;
        let basis = prepared
            .managed_basis
            .clone()
            .ok_or(SourceCommandError::Conflict("managed package basis absent"))?;
        Ok(ManagedSerializedCreation {
            prepared,
            plan,
            basis,
        })
    }
}
impl ManagedSerializedCreation {
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        self.prepared.files()
    }
    pub fn reads(&self) -> &[SourceDependency] {
        &self.plan.reads
    }
}
/// Private construction retains exact request/configuration, selected current
/// source/software observations and pending files. It is not a commit grant.
pub struct PreparedCreation {
    context: CommandContext,
    managed_basis: Option<ManagedCreationBasis>,
    managed_observations: Option<BTreeMap<String, ManagedCreationObservation>>,
    managed_inventory: Option<crate::source_cohort::ManagedAgentInventory>,
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
    pub(crate) fn selected_components(&self) -> &SoftwareComponentSelectionV1 {
        &self.components
    }
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
        self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<SerializedCreation> {
        if self.managed_basis.is_some() {
            return Err(SourceCommandError::Conflict(
                "managed payload cannot become v1 command",
            ));
        }
        let (prepared, plan) =
            self.serialize_content(software, components, worker, deadline, cancelled, None)?;
        let command = plan.into_v1(prepared.context.base_revision);
        Ok(SerializedCreation { prepared, command })
    }
    /// Cold reconstruction uses exact retained serialization evidence. The
    /// independently selected filesystem reader owns these bytes; callers
    /// cannot supply an alternative receipt or provenance event.
    pub(crate) fn serialize_retained(
        self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        retained: &BTreeMap<String, Vec<u8>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<SerializedCreation> {
        if self.managed_basis.is_some() {
            return Err(SourceCommandError::Conflict(
                "managed creation retained route",
            ));
        }
        let (prepared, plan) = self.serialize_content(
            software,
            components,
            worker,
            deadline,
            cancelled,
            Some(retained),
        )?;
        let command = plan.into_v1(prepared.context.base_revision);
        Ok(SerializedCreation { prepared, command })
    }
    fn serialize_content(
        mut self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
        retained: Option<&BTreeMap<String, Vec<u8>>>,
    ) -> SourceCommandResult<(PreparedCreation, cmd::CommandPlan)> {
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
            if let Some(original) = retained {
                crate::source_serialization::restore_creation_capture(
                    &request,
                    cmd::text(&config, "provenance_event_id")?,
                    self.home.as_str(),
                    &mut self.files,
                    original,
                    software,
                    components,
                    deadline,
                    cancelled,
                )?;
            } else {
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
            }
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
                        || cmd::integer(entity, "size_bytes")? != bytes.len() as u64
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
        let recorded_at = if let Some(original) = retained {
            let original_receipt = cmd::parse(original.get("source-create-receipt.json").ok_or(
                SourceCommandError::Conflict("retained creation receipt absent"),
            )?)?;
            let instant = cmd::text(&original_receipt, "recorded_at")?.to_owned();
            cmd::validate_instant(&instant)?;
            instant
        } else {
            crate::source_serialization::instant()?
        };
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
            ("recorded_at", cmd::string(&recorded_at)),
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
        if retained.is_some_and(|original| original != &self.files) {
            return Err(SourceCommandError::Conflict(
                "retained creation package differs from reprepare",
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
        let command = self.context.plan_content(
            self.family.handler_id(),
            cmd::object(vec![
                ("receipt", receipt),
                ("replayed", JsonValue::Bool(false)),
                ("grants_admission", JsonValue::Bool(false)),
            ]),
            changes,
            false,
        )?;
        Ok((self, command))
    }
}

fn creation_result(
    prepared: &PreparedCreation,
    target_exists: bool,
    receipt: JsonValue,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    creation_information(
        &prepared.context,
        prepared.family,
        target_exists,
        receipt,
        replayed,
    )
}
fn creation_information(
    context: &CommandContext,
    family: CreationFamily,
    target_exists: bool,
    receipt: JsonValue,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let config = cmd::parse(&context.configuration_raw)?;
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
    } else if family == CreationFamily::Artifact {
        cmd::set(&mut result, "record_type", cmd::string("artifact"))?;
        cmd::set(
            &mut result,
            "source_profile",
            cmd::object(vec![
                ("record_type", cmd::string("artifact")),
                ("identity_field", cmd::string("artifact_id")),
                ("id_prefix", cmd::string("tos.artifact.")),
                ("source_basename", cmd::string("artifact-witness.json")),
                (
                    "schema_ref",
                    cmd::string(crate::source_artifact_native::SCHEMA),
                ),
                (
                    "schema_version",
                    cmd::string("tos_artifact_source_witness_v2"),
                ),
                ("source_scope", cmd::string("public_metadata_only")),
            ]),
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
        let registry = json(context, ENTITIES)?;
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
    } else if family == CreationFamily::Artifact {
        keys.push("source_bindings");
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
    if family == CreationFamily::Artifact {
        crate::source_artifact_native::configuration(&config)?;
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
    if family == CreationFamily::Artifact {
        return crate::source_artifact_native::initial(
            ctx, config, record, worker, deadline, cancelled,
        );
    }
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

/// Read the maintained initial source metadata without constructing a
/// package, provenance event, receipt or publication proposal.
pub(crate) fn initial_information_from_captures(
    context: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    target_exists: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let mut context = context.clone();
    let software_files: Vec<_> = context
        .files
        .iter()
        .filter(|f| !f.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect();
    context.files = claims::complete_authored_inputs(&context, cut, deadline, cancelled)?;
    context.files.extend(software_files);
    context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    let (family, config, _) = configuration(&context)?;
    revisions::validate_source_profile_registry(worker, deadline, cancelled, &context)?;
    let request = cmd::parse(&context.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    cmd::exact_keys(
        &request,
        if operation == "describe" {
            &["schema_version", "operation"]
        } else {
            &["schema_version", "operation", "record"]
        },
    )?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !["describe", "prepare"].contains(&operation)
    {
        return Err(SourceCommandError::Invalid("creation information request"));
    }
    let mut result = creation_information(&context, family, target_exists, JsonValue::Null, false)?;
    if operation == "prepare" {
        if !contains(&config, "allowed_operations", family.operation())? {
            return Err(SourceCommandError::Denied(
                "creation operation not delegated",
            ));
        }
        let record = cmd::field(&request, "record")?;
        initial(
            &context, cut, worker, &config, family, record, deadline, cancelled,
        )?;
        cmd::set(
            &mut result,
            "prepared_source",
            forms::metadata_subject(record)?,
        )?;
        cmd::set(
            &mut result,
            "source_fields",
            JsonValue::Array(
                forms::metadata_fields(record)?
                    .into_iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        )?;
    }
    Ok(result)
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
        context, cut, software, components, worker, deadline, cancelled, None, None,
    )
}

pub fn prepare_managed_agent_creation(
    input: &ManagedCreationInput,
    schema_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ManagedPreparedCreation> {
    if schema_cut.current().revision() != input.context.base_revision
        || worker.source_revision() != input.context.base_revision
        || components != &input.components
        || software.selection() != input.components.capture()
    {
        return Err(SourceCommandError::Conflict(
            "managed creation schema/software selection differs",
        ));
    }
    Ok(ManagedPreparedCreation {
        prepared: prepare_creation(
            &input.context,
            schema_cut,
            software,
            components,
            worker,
            deadline,
            cancelled,
            None,
            Some(input),
        )?,
    })
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
    crate::source_sign::require_assessment_profile(assessment_worker)?;
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
        None,
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
    managed: Option<&ManagedCreationInput>,
) -> SourceCommandResult<PreparedCreation> {
    if managed.is_none() {
        context.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    }
    let mut ctx = context.clone();
    let software_files = ctx
        .files
        .iter()
        .filter(|f| !f.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    if managed.is_none() {
        ctx.files = claims::complete_authored_inputs(context, cut, deadline, cancelled)?;
        ctx.files.extend(software_files);
        ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    } else {
        ctx.check()?;
    }
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
    if family == CreationFamily::Artifact {
        keys.push("source_bindings");
        if !cmd::same(
            cmd::field(&request, "source_bindings")?,
            cmd::field(&config, "source_bindings")?,
        )? {
            return Err(SourceCommandError::Denied(
                "Artifact request input bindings differ from grant",
            ));
        }
    }
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
    if managed.is_some() && (!family.corpus() || cmd::text(record, "record_type")? != "agent") {
        return Err(SourceCommandError::Unsupported(
            "managed creation consumer is initial native Agent only",
        ));
    }
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
    let occupied = if managed.is_some() {
        ctx.files.iter().any(|member| {
            member.path.as_str() == home.as_str()
                || member
                    .path
                    .as_str()
                    .starts_with(&format!("{}/", home.as_str()))
        })
    } else {
        cut.current().members().any(|member| {
            member.path.as_str() == home.as_str()
                || member
                    .path
                    .as_str()
                    .starts_with(&format!("{}/", home.as_str()))
        })
    };
    if occupied {
        return Err(SourceCommandError::Conflict(
            "creation source home already occupied",
        ));
    }
    let indexed = managed.and_then(|input| input.inventory.as_ref());
    let mut inventory = if indexed.is_some() {
        None
    } else {
        Some(if managed.is_some() {
            claims::maintained_agent_inventory_from_managed(
                &ctx, false, worker, deadline, cancelled,
            )?
        } else {
            claims::maintained_inventory_from_cut(&ctx, cut, worker, deadline, cancelled)?
        })
    };
    if inventory.as_ref().is_some_and(|inventory| {
        inventory.objects.contains_key(
            cmd::text(
                record,
                if family == CreationFamily::Artifact {
                    "artifact_id"
                } else {
                    "record_id"
                },
            )
            .unwrap_or(""),
        )
    }) {
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
        && inventory.as_ref().is_some_and(|inventory| {
            inventory
                .events
                .object_get(cmd::text(&config, "provenance_event_id").unwrap_or(""))
                .is_some()
        })
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
    if let Some(inventory) = &inventory {
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
            if name.ends_with("/source-claims.jsonl") || name.ends_with("/historical-claims.jsonl")
            {
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
            inventory.as_ref().ok_or(SourceCommandError::Invalid(
                "historical inventory unavailable",
            ))?,
            worker,
            deadline,
            cancelled,
            &mut evidence,
        )?;
        files.insert("historical-claims.jsonl".into(), raw);
    }
    let dependencies = if let Some(indexed) = indexed {
        if !profile_resources.is_empty() {
            return Err(SourceCommandError::Unsupported(
                "Agent projection resource route changed",
            ));
        }
        indexed.dependencies.clone()
    } else {
        let mut inventory = inventory.take().ok_or(SourceCommandError::Invalid(
            "creation inventory unavailable",
        ))?;
        for name in profile_resources
            .into_iter()
            .filter(|_| family != CreationFamily::Artifact)
        {
            cmd::set(
                &mut inventory.record_inputs,
                &name,
                cmd::string(&Digest256::of_bytes(selected(&ctx, &name)?).to_hex()),
            )?;
        }
        let snapshot = creation_dependency_snapshot(
            &ctx,
            family,
            CreationInventoryEncoding {
                records: inventory.records,
                claims: JsonValue::Array(inventory.claims.into_values().collect()),
                source_profiles: inventory.record_inputs,
                native_identity_snapshot: inventory.native_identity_snapshot.take().ok_or(
                    SourceCommandError::Invalid(
                        "creation native identity inventory was not selected",
                    ),
                )?,
                native_text_snapshot: inventory.native_text_snapshot.take(),
                claim_profile_inputs: inventory.claim_profile_inputs,
                events: inventory.events,
                anchors: inventory.anchors,
            },
            evidence,
            form_inputs,
        )?;
        cmd::record_digest(&snapshot)?.to_prefixed()
    };
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(SourceCommandError::Denied(
            "creation deadline or cancellation",
        ));
    }
    Ok(PreparedCreation {
        context: ctx,
        managed_basis: managed.map(|input| input.basis.clone()),
        managed_observations: managed.map(|input| input.observations.clone()),
        managed_inventory: managed.and_then(|input| input.inventory.clone()),
        family,
        home,
        subject,
        dependencies,
        files,
        components: components.clone(),
    })
}

// Two concrete consumers share the maintained snapshot envelope. The indexed
// Agent codec streams its four large fields; it never digests the NULL slots.
struct CreationInventoryEncoding {
    records: JsonValue,
    claims: JsonValue,
    source_profiles: JsonValue,
    native_identity_snapshot: String,
    native_text_snapshot: Option<String>,
    claim_profile_inputs: JsonValue,
    events: JsonValue,
    anchors: JsonValue,
}
fn creation_dependency_snapshot(
    ctx: &CommandContext,
    family: CreationFamily,
    encoding: CreationInventoryEncoding,
    evidence: Vec<JsonValue>,
    form_inputs: JsonValue,
) -> SourceCommandResult<JsonValue> {
    if family == CreationFamily::Artifact {
        let config = cmd::parse(&ctx.configuration_raw)?;
        let implementation = [
            "rust/crates/tos-command/src/source_creation.rs",
            "rust/crates/tos-command/src/source_command.rs",
            "rust/crates/tos-command/src/source_native_cli.rs",
            "rust/crates/tos-command/src/source_legacy_historical_claim.rs",
            "rust/crates/tos-command/src/source_forms.rs",
            "rust/crates/tos-validation/src/assessment.rs",
            "rust/crates/tos-compiler/src/source_witness_catalog.rs",
            "rust/crates/tos-command/src/source_private_profile.rs",
            "rust/crates/tos-command/src/source_private_claim.rs",
            "rust/crates/tos-command/src/source_private_owner_store.rs",
            "rust/crates/tos-command/src/source_text_owner.rs",
            "rust/crates/tos-command/src/source_corpus_index_projection.rs",
            "rust/crates/tos-compiler/src/source_bibliographic.rs",
            "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ];
        return Ok(cmd::object(vec![
            ("records", encoding.records),
            ("claims", encoding.claims),
            ("events", encoding.events),
            ("source_profiles", encoding.source_profiles),
            ("source_claim_profiles", encoding.claim_profile_inputs),
            (
                "native_semantic_identity_snapshot",
                cmd::string(&encoding.native_identity_snapshot),
            ),
            (
                "native_text_binding_snapshot",
                encoding
                    .native_text_snapshot
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "inputs",
                cmd::object(vec![
                    ("bindings", cmd::field(&config, "source_bindings")?.clone()),
                    (
                        "schemas",
                        claims::raw_digests(
                            ctx,
                            &[
                                "ToS/contracts/rights-record.schema.json",
                                "ToS/contracts/material-discovery-record.schema.json",
                            ],
                            true,
                        )?,
                    ),
                ]),
            ),
            ("forms", form_inputs),
            (
                "contracts",
                claims::raw_digests(
                    ctx,
                    &[
                        crate::source_artifact_native::SCHEMA,
                        "ToS/contracts/provenance-event-v2.schema.json",
                    ],
                    true,
                )?,
            ),
            (
                "implementation",
                claims::raw_digests(ctx, &implementation, true)?,
            ),
        ]));
    }
    let provenance_contract = if family == CreationFamily::HistoricalV1 {
        cmd::object(vec![])
    } else {
        claims::raw_digests(
            &ctx,
            &["ToS/contracts/provenance-event-v2.schema.json"],
            true,
        )?
    };
    let mut snapshot = cmd::object(vec![
        ("records", encoding.records),
        ("claims", encoding.claims),
        ("source_profiles", encoding.source_profiles),
        (
            "native_semantic_identity_snapshot",
            cmd::string(&encoding.native_identity_snapshot),
        ),
        (
            "native_text_binding_snapshot",
            encoding
                .native_text_snapshot
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("source_claim_profiles", encoding.claim_profile_inputs),
        ("provenance_contract", provenance_contract),
        ("events", encoding.events),
        ("anchors", encoding.anchors),
        ("evidence", JsonValue::Array(evidence)),
        ("forms", form_inputs),
        ("contracts", claims::raw_digests(&ctx, CONTRACTS, true)?),
        (
            "implementation",
            claims::raw_digests(&ctx, RULE_INPUTS, true)?,
        ),
    ]);
    if family == CreationFamily::Sign {
        let promotion_implementation = "rust/crates/tos-command/src/source_assessment_journal.rs";
        cmd::set(
            &mut snapshot,
            "promotion_implementation",
            cmd::string(
                &Digest256::of_bytes(selected(&ctx, promotion_implementation)?).to_prefixed(),
            ),
        )?;
    }
    Ok(snapshot)
}

pub(crate) fn agent_dependency_template(
    ctx: &CommandContext,
    source_profiles: JsonValue,
) -> SourceCommandResult<JsonValue> {
    creation_dependency_snapshot(
        ctx,
        CreationFamily::CorpusV2,
        CreationInventoryEncoding {
            records: JsonValue::Null,
            claims: JsonValue::Array(vec![]),
            source_profiles,
            native_identity_snapshot: revisions::python_ascii_digest(&cmd::object(vec![]))?,
            native_text_snapshot: None,
            claim_profile_inputs: cmd::object(vec![]),
            events: JsonValue::Null,
            anchors: JsonValue::Null,
        },
        vec![],
        JsonValue::Null,
    )
}

/// Only the durable committed-attempt reader supplies this original input and
/// immutable package. Reconstruct mechanics, never register or authorize a write.
pub(crate) fn reprepare_managed_agent_creation(
    input: ManagedCreationInput,
    schema_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    original_files: BTreeMap<String, Vec<u8>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ManagedSerializedCreation> {
    let prepared = prepare_managed_agent_creation(
        &input, schema_cut, software, components, worker, deadline, cancelled,
    )?
    .prepared;
    if !prepared.family.corpus() {
        return Err(SourceCommandError::Unsupported(
            "managed recovery is Agent-only",
        ));
    }
    let basis = input.basis.clone();
    drop(input);
    let (prepared, plan) = prepared.serialize_content(
        software,
        components,
        worker,
        deadline,
        cancelled,
        Some(&original_files),
    )?;
    Ok(ManagedSerializedCreation {
        basis,
        prepared,
        plan,
    })
}
