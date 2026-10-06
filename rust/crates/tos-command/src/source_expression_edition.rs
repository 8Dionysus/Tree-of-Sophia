//! One protected Expression/Edition growth operation. Shared transport moves exact
//! bytes; this owner alone constructs the exact typed authorization.

use super::work_expression::{
    allowed_forms, checked_current_descriptor, digest_map, raw_hex, relative, selected_forms, slug,
};
use super::work_transaction::PublicationSnapshot;
use super::{CreationFilesystem, active, walk, work_transaction};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};

const CONFIG: &str = "tos_local_expression_edition_owner_v1";
const REQUEST: &str = "tos_local_expression_edition_command_v1";
const OPERATION: &str = "expression.edition.create";
const RECOVERY: &str = "expression.edition.recover";
const AUTHORIZATION: &str = "tos_expression_edition_authorization_v1";
const SCOPE_KEYS: &[&str] = &[
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "edition_id",
    "edition_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_expression_form_ids",
    "allowed_edition_form_ids",
    "allowed_claim_form_ids",
];

struct ExpressionEditionOwner {
    configuration: JsonValue,
    request: JsonValue,
    expression_path: RelativePath,
    work_path: RelativePath,
    edition_path: RelativePath,
    claim_path: RelativePath,
    expression_id: String,
    work_id: String,
    edition_id: String,
    claim_id: String,
    configuration_digest: String,
}

struct ExpressionEditionGrant {
    configuration: JsonValue,
    expression_path: RelativePath,
    work_path: RelativePath,
    edition_path: RelativePath,
    claim_path: RelativePath,
    expression_forms: BTreeSet<String>,
    claim_forms: BTreeSet<String>,
    edition_forms: BTreeSet<String>,
}
impl ExpressionEditionGrant {
    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        required_operation: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        fs.current_context(ctx, deadline, cancelled)?;
        Self::from_configuration(
            fs,
            cmd::parse(&ctx.configuration_raw)?,
            required_operation,
            deadline,
            cancelled,
        )
    }
    fn from_configuration(
        fs: &CreationFilesystem,
        config: JsonValue,
        required_operation: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;

        let mut keys = SCOPE_KEYS.to_vec();
        keys.extend([
            "schema_version",
            "uid",
            "principal_id",
            "maker_type",
            "source_root",
            "authority_ref",
            "expires_at",
            "allowed_operations",
        ]);
        cmd::exact_keys(&config, &keys)?;
        if cmd::text(&config, "schema_version")? != CONFIG
            || cmd::integer(&config, "uid")? != u64::from(fs.uid)
            || Path::new(cmd::text(&config, "source_root")?) != fs.root_path
            || !cmd::nonblank(cmd::text(&config, "principal_id")?)
            || !cmd::nonblank(cmd::text(&config, "authority_ref")?)
            || !matches!(
                cmd::text(&config, "maker_type")?,
                "human" | "software" | "model"
            )
        {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition delegation root/account/schema",
            ));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let operations = cmd::array(&config, "allowed_operations")?;
        let mut allowed = BTreeSet::new();
        if operations.len() > 2 {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition operation count",
            ));
        }
        for operation in operations {
            let operation = operation
                .as_str()
                .ok_or(SourceCommandError::Invalid("ExpressionEdition operation"))?;
            if !matches!(operation, OPERATION | RECOVERY) || !allowed.insert(operation) {
                return Err(SourceCommandError::Denied(
                    "ExpressionEdition operation grant",
                ));
            }
        }
        if required_operation.is_some_and(|operation| !allowed.contains(operation)) {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition growth not delegated",
            ));
        }
        for (key, prefix) in [
            ("work_id", "tos.work."),
            ("expression_id", "tos.expression."),
            ("edition_id", "tos.edition."),
            ("claim_id", "tos.claim."),
            ("provenance_event_id", "tos.event."),
        ] {
            if !slug(cmd::text(&config, key)?, prefix) {
                return Err(SourceCommandError::Denied(
                    "ExpressionEdition typed identity grant",
                ));
            }
        }
        let work_path = relative(cmd::text(&config, "work_source_path")?)?;
        let expression_path = relative(cmd::text(&config, "expression_source_path")?)?;
        let edition_path = relative(cmd::text(&config, "edition_source_path")?)?;
        let parts = work_path.as_str().split('/').collect::<Vec<_>>();
        let expression_parts = expression_path.as_str().split('/').collect::<Vec<_>>();
        let edition_parts = edition_path.as_str().split('/').collect::<Vec<_>>();
        let work_home = work_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied(
                "ExpressionEdition typed source home absent",
            ))?
            .0;
        let expression_home = expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied(
                "ExpressionEdition typed source home absent",
            ))?
            .0;
        let edition_home = edition_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied(
                "ExpressionEdition typed source home absent",
            ))?
            .0;
        if parts.len() < 5
            || parts[..3] != ["ToS", "source-witnesses", "works"]
            || parts.last() != Some(&"work.json")
            || expression_parts.last() != Some(&"expression.json")
            || expression_parts.len() < 3
            || expression_home.rsplit_once('/').map(|x| x.0)
                != Some(format!("{work_home}/expressions").as_str())
            || !expression_home
                .rsplit_once('/')
                .is_some_and(|x| slug(x.1, ""))
            || edition_parts.last() != Some(&"edition.json")
            || edition_parts.len() < 3
            || edition_home.rsplit_once('/').map(|x| x.0)
                != Some(format!("{expression_home}/editions").as_str())
            || !edition_home.rsplit_once('/').is_some_and(|x| slug(x.1, ""))
        {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition exact Work/Expression/Edition homes",
            ));
        }
        let claim_path = relative(&format!("{edition_home}/source-claims.jsonl"))?;
        let expression_forms = allowed_forms(&config, "allowed_expression_form_ids")?;
        let edition_forms = allowed_forms(&config, "allowed_edition_form_ids")?;
        let claim_forms = allowed_forms(&config, "allowed_claim_form_ids")?;
        if !expression_forms.is_disjoint(&edition_forms)
            || !expression_forms.is_disjoint(&claim_forms)
            || !edition_forms.is_disjoint(&claim_forms)
        {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition distinct typed form identities",
            ));
        }

        Ok(Self {
            configuration: config,
            expression_path,
            work_path,
            edition_path,
            claim_path,
            expression_forms,
            claim_forms,
            edition_forms,
        })
    }
}
/// Validate only this exact family grant after protected transport selection.
/// The engine independently rereads current context before any operation.
pub fn check_expression_edition_configuration(
    fs: &CreationFilesystem,
    configuration: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    ExpressionEditionGrant::from_configuration(fs, configuration.clone(), None, deadline, cancelled)
        .map(|_| ())
}

impl ExpressionEditionOwner {
    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        proposal: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let grant = ExpressionEditionGrant::select(fs, ctx, Some(OPERATION), deadline, cancelled)?;
        Self::from_grant(grant, &ctx.request_raw, proposal, None)
    }
    fn from_grant(
        grant: ExpressionEditionGrant,
        request_raw: &[u8],
        proposal: bool,
        historical_configuration_digest: Option<&str>,
    ) -> SourceCommandResult<Self> {
        let ExpressionEditionGrant {
            configuration: config,
            expression_path,
            work_path,
            edition_path,
            claim_path,
            expression_forms,
            claim_forms,
            edition_forms,
        } = grant;
        let request = cmd::parse(request_raw)?;
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "record",
            "edition_forms",
            "claim",
            "forms",
            "claim_forms",
            "reason",
        ];
        if !proposal {
            request_keys.extend([
                "command_id",
                "fields",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "expected_publication",
            ]);
        }
        cmd::exact_keys(&request, &request_keys)?;
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")?
                != if proposal {
                    "prepare-create"
                } else {
                    OPERATION
                }
            || cmd::canonical(&request)?.len() > 1_048_576
        {
            return Err(SourceCommandError::Invalid(
                "ExpressionEdition request schema/operation/budget",
            ));
        }
        let reason = cmd::text(&request, "reason")?;
        if !cmd::nonblank(reason)
            || tos_foundation::python_strip_unicode16_v1(reason, reason.len())
                .map_err(|_| SourceCommandError::Invalid("bounded reason whitespace profile"))?
                .chars()
                .count()
                > 4096
        {
            return Err(SourceCommandError::Invalid("ExpressionEdition reason"));
        }
        let configuration_digest = match historical_configuration_digest {
            Some(digest) if work_transaction::is_hash(digest) => digest.to_owned(),
            Some(_) => {
                return Err(SourceCommandError::Invalid(
                    "ExpressionEdition retained owner digest",
                ));
            }
            None => cmd::record_digest(&config)?.to_prefixed(),
        };
        if !proposal {
            let id = cmd::text(&request, "command_id")?;
            if id.is_empty() || id.chars().count() > 256 {
                return Err(SourceCommandError::Invalid(
                    "ExpressionEdition command identity",
                ));
            }
            for key in [
                "expected_configuration",
                "expected_revision",
                "expected_dependencies",
            ] {
                if !work_transaction::is_hash(cmd::text(&request, key)?) {
                    return Err(SourceCommandError::Invalid(
                        "ExpressionEdition expected digest",
                    ));
                }
            }
            if cmd::field(&request, "expected_publication")? != &JsonValue::Null
                && !cmd::field(&request, "expected_publication")?
                    .as_str()
                    .is_some_and(work_transaction::is_hash)
            {
                return Err(SourceCommandError::Invalid(
                    "ExpressionEdition expected publication",
                ));
            }
            cmd::exact_keys(cmd::field(&request, "fields")?, &["embodiment_claim_refs"])?;
            if cmd::text(&request, "expected_configuration")? != configuration_digest {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition current owner digest differs",
                ));
            }
        }
        let record = cmd::field(&request, "record")?;
        let claim = cmd::field(&request, "claim")?;
        let work_id = cmd::text(&config, "work_id")?.to_owned();
        let expression_id = cmd::text(&config, "expression_id")?.to_owned();
        let edition_id = cmd::text(&config, "edition_id")?.to_owned();
        let claim_id = cmd::text(&config, "claim_id")?.to_owned();
        let maker = cmd::object(vec![
            ("maker_type", cmd::field(&config, "maker_type")?.clone()),
            ("agent_ref", cmd::field(&config, "principal_id")?.clone()),
        ]);
        if cmd::text(record, "record_type")? != "edition"
            || cmd::text(record, "record_id")? != edition_id
            || cmd::array(record, "embodies_expression_refs")? != [cmd::string(&expression_id)]
            || cmd::text(claim, "claim_id")? != claim_id
            || cmd::text(claim, "subject_ref")? != expression_id
            || cmd::text(claim, "object")? != edition_id
            || cmd::text(claim, "predicate")? != "embodied_by"
            || cmd::text(claim, "provenance_event_ref")?
                != cmd::text(&config, "provenance_event_id")?
            || !cmd::same(cmd::field(claim, "maker")?, &maker)?
        {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition packet endpoint/maker/event grant",
            ));
        }
        selected_forms(&request, "edition_forms", &edition_forms)?;
        selected_forms(&request, "forms", &expression_forms)?;
        selected_forms(&request, "claim_forms", &claim_forms)?;
        Ok(Self {
            configuration: config,
            request,
            expression_path,
            work_path,
            edition_path,
            claim_path,
            expression_id,
            work_id,
            edition_id,
            claim_id,
            configuration_digest,
        })
    }

    fn scope(&self) -> SourceCommandResult<JsonValue> {
        Ok(JsonValue::Object(
            self.configuration
                .as_object()
                .ok_or(SourceCommandError::Invalid("ExpressionEdition scope"))?
                .iter()
                .filter(|(key, _)| key.as_str().is_some_and(|key| SCOPE_KEYS.contains(&key)))
                .cloned()
                .collect(),
        ))
    }

    fn authorization(&self, dependencies: JsonValue) -> SourceCommandResult<JsonValue> {
        Ok(cmd::object(vec![
            ("schema_version", cmd::string(AUTHORIZATION)),
            ("scope", self.scope()?),
            (
                "principal_id",
                cmd::field(&self.configuration, "principal_id")?.clone(),
            ),
            (
                "maker_type",
                cmd::field(&self.configuration, "maker_type")?.clone(),
            ),
            (
                "authority_ref",
                cmd::field(&self.configuration, "authority_ref")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::string(&self.configuration_digest),
            ),
            (
                "command_id",
                cmd::field(&self.request, "command_id")?.clone(),
            ),
            (
                "request_digest",
                cmd::string(&cmd::record_digest(&self.request)?.to_prefixed()),
            ),
            ("dependency_bindings", dependencies),
        ]))
    }
}

impl ExpressionEditionOwner {
    fn transaction_id(&self) -> SourceCommandResult<String> {
        Ok(cmd::record_digest(&cmd::object(vec![
            ("operation", cmd::string(OPERATION)),
            (
                "command_id",
                cmd::field(&self.request, "command_id")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::field(&self.request, "expected_configuration")?.clone(),
            ),
            (
                "request_digest",
                cmd::string(&cmd::record_digest(&self.request)?.to_prefixed()),
            ),
        ]))?
        .to_prefixed())
    }

    fn plan(
        &self,
        authorization: JsonValue,
        before: &BTreeMap<String, Vec<u8>>,
        parent: BTreeMap<String, Vec<u8>>,
        child: BTreeMap<String, Vec<u8>>,
        new_directories: Vec<RelativePath>,
    ) -> SourceCommandResult<work_transaction::WorkPlan> {
        let expression_home = self
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("ExpressionEdition parent home"))?
            .0;
        let claim_home = self
            .edition_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("ExpressionEdition Claim home"))?
            .0;
        let parent_names = [
            "expression.json",
            "expression.human-forms.json",
            "source-revision-history.json",
        ];
        let claim_forms = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(self.claim_id.as_bytes()).to_hex()
        );
        let child_names = [
            "edition.json",
            "edition.human-forms.json",
            "source-claims.jsonl",
            claim_forms.as_str(),
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "expression-edition-receipt.json",
        ];
        if parent.len() != parent_names.len()
            || parent_names.iter().any(|name| !parent.contains_key(*name))
            || before
                .keys()
                .any(|name| !parent_names.contains(&name.as_str()))
            || child.len() != child_names.len()
            || child_names.iter().any(|name| !child.contains_key(*name))
            || new_directories.last() != Some(&relative(claim_home)?)
            || new_directories.len() > 2
            || new_directories.len() == 2
                && new_directories[0].as_str() != claim_home.rsplit_once('/').unwrap().0
        {
            return Err(SourceCommandError::Invalid(
                "ExpressionEdition exact selected package shape",
            ));
        }
        let mut files = Vec::with_capacity(parent.len() + child.len());
        for (name, raw) in parent {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{expression_home}/{name}"))?,
                before: before.get(&name).cloned(),
                after: Some(raw),
            });
        }
        for (name, raw) in child {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{claim_home}/{name}"))?,
                before: None,
                after: Some(raw),
            });
        }
        Ok(work_transaction::WorkPlan {
            transaction_id: self.transaction_id()?,
            authorization,
            item_path_profile: None,
            files,
            new_directories,
            source_readset: None,
            source_successor: None,
        })
    }
}

use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};

use super::expression_responsibility::{
    ExpressionResponsibilityCatalogRows, catalog_claim, catalog_lines, catalog_record,
    catalog_rows, source_dependency,
};

use super::work_expression::{
    WorkApplicationGuard, WorkControlRead, after_cut_budget, complete_current_cut,
    original_prior_publication, physical_current, same_selected_plan, selected_reads_current,
    selected_sides, software_current, work_dependencies_current,
};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

fn item_error(reason: ItemRefusal) -> SourceCommandError {
    SourceCommandError::SchemaExecution {
        path: OPERATION.to_owned(),
        root: "selected ExpressionEdition growth mechanics".to_owned(),
        reason,
    }
}
fn serde_value(value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("ExpressionEdition JSON value conversion"))
}
fn foundation_value(value: &serde_json::Value) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        &serde_json::to_vec(value)
            .map_err(|_| SourceCommandError::Invalid("ExpressionEdition JSON value conversion"))?,
    )
}
struct ExpressionEditionCatalog {
    digests: BTreeMap<String, String>,
    retained_transactions: BTreeMap<String, String>,
    native_reads: Vec<tos_validation::PredicateRead>,
    native_bytes: u64,
    native_state: usize,
    source_bytes: u64,
}
fn legacy_topology_binding(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    entry: &JsonValue,
    claim: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    retained: &mut Option<JsonValue>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    const EVENT: &str = "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31";
    const PATH: &str = "ToS/source-witnesses/relations/provenance.jsonl";
    if cmd::text(claim, "claim_type")? != "bibliographic"
        || cmd::text(claim, "assertion_layer")? != "bibliographic_assertion"
        || cmd::text(claim, "provenance_event_ref")? != EVENT
    {
        return Err(SourceCommandError::Denied(
            "ExpressionEdition existing topology evidence route",
        ));
    }
    if retained.is_none() {
        let raw = source_dependency(fs, cut, PATH, digests, total, deadline, cancelled)?;
        let mut rows =
            catalog_lines(&raw).filter(|line| !crate::source_claims::python_bytes_blank(line));
        let row = cmd::parse(rows.next().ok_or(SourceCommandError::Conflict(
            "ExpressionEdition legacy batch absent",
        ))?)?;
        if rows.next().is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition legacy batch is not singular",
            ));
        }
        *retained = Some(row);
    }
    let event = retained.as_ref().unwrap();
    let source = cmd::text(entry, "source_claim_file_ref")?;
    if cmd::text(event, "event_id")? != EVENT
        || !cmd::array(event, "outputs")?.iter().any(|output| {
            output.object_get("ref").and_then(JsonValue::as_str) == Some(source)
                && output.object_get("sha256").and_then(JsonValue::as_str)
                    == digests.get(source).map(String::as_str)
        })
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition legacy topology batch changed",
        ));
    }
    Ok(())
}

fn current_catalog(
    fs: &CreationFilesystem,
    owner: &ExpressionEditionOwner,
    before: &BTreeMap<String, Vec<u8>>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    publication_token: Option<&str>,
    mut check_publication: impl FnMut() -> SourceCommandResult<()>,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionCatalog> {
    check_publication()?;
    let deadline = limits.deadline;
    let ExpressionResponsibilityCatalogRows {
        records,
        claims,
        mut digests,
        source_bytes: mut total,
    } = catalog_rows(fs, publication_token, deadline, cancelled)?;
    let expression_raw = before
        .get("expression.json")
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition predecessor absent",
        ))?;
    let expression = cmd::parse(expression_raw)?;
    let history = tos_validation::native_compound::inspect_record_history(
        before,
        expression_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let parent = records
        .get(&owner.expression_id)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition catalog parent absent",
        ))?;
    if cmd::text(&expression, "record_type")? != "expression"
        || cmd::text(&expression, "record_id")? != owner.expression_id
        || cmd::text(&expression, "work_ref")? != owner.work_id
        || cmd::text(parent, "source_record_ref")? != owner.expression_path.as_str()
        || cmd::text(parent, "record_sha256")? != cmd::record_digest(&expression)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition exact parent/Work binding stale",
        ));
    }
    for identity in [&owner.edition_id, &owner.claim_id] {
        if records.contains_key(identity) || claims.contains_key(identity) {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition new identity occupied",
            ));
        }
    }
    if claims.values().any(|entry| {
        entry.object_get("provenance_event_ref")
            == owner.configuration.object_get("provenance_event_id")
    }) {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition new event occupied",
        ));
    }
    let work_entry = records
        .get(&owner.work_id)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition Work absent",
        ))?;
    if cmd::text(work_entry, "record_type")? != "work"
        || cmd::text(work_entry, "source_record_ref")? != owner.work_path.as_str()
    {
        return Err(SourceCommandError::Denied(
            "ExpressionEdition exact Work catalog binding",
        ));
    }
    let work = catalog_record(
        fs,
        cut,
        work_entry,
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    let origins = claims
        .values()
        .filter(|entry| {
            entry.object_get("predicate").and_then(JsonValue::as_str) == Some("has_expression")
                && entry.object_get("object").and_then(JsonValue::as_str)
                    == Some(owner.expression_id.as_str())
        })
        .collect::<Vec<_>>();
    if origins.len() != 1
        || cmd::text(origins[0], "subject_ref")? != owner.work_id
        || cmd::array(&work, "expression_claim_refs")?
            .iter()
            .filter(|id| {
                id.as_str()
                    == origins[0]
                        .object_get("claim_id")
                        .and_then(JsonValue::as_str)
            })
            .count()
            != 1
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition unique declared Work origin absent",
        ));
    }
    let origin = catalog_claim(
        fs,
        cut,
        origins[0],
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    let mut legacy = None;
    let origin_path = cmd::text(origins[0], "source_claim_file_ref")?;
    if origin_path.ends_with("/source-claims.jsonl") {
        let mut remaining = limits;
        remaining.max_total_bytes = remaining.max_total_bytes.checked_sub(total as u64).ok_or(
            SourceCommandError::Invalid("ExpressionEdition source byte budget"),
        )?;
        let observation = tos_validation::native_compound::verify_work_expression_reads_from_cut(
            cut,
            worker,
            origin_path,
            &serde_value(&origin)?,
            remaining,
            cancelled,
        )
        .map_err(item_error)?;
        let (actual, plan, _, _) = work_transaction::inspect_committed(
            fs,
            &observation.transaction_id,
            deadline,
            cancelled,
        )?;
        if observation.transport != tos_validation::native_compound::NativeTransportState::Committed
            || actual != observation.manifest_sha256
            || cmd::text(&plan.authorization, "schema_version")?
                != "tos_work_expression_authorization_v1"
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition native Work origin not committed",
            ));
        }
        retained_transactions.insert(observation.transaction_id, actual);
        native_bytes = observation.bytes_read;
        native_state = observation.returned_state_bytes;
        native_reads.extend(observation.reads);
    } else {
        legacy_topology_binding(
            fs,
            cut,
            origins[0],
            &origin,
            &mut digests,
            &mut total,
            &mut legacy,
            deadline,
            cancelled,
        )?;
    }
    let mut editions = BTreeMap::new();
    for (id, entry) in &records {
        if entry.object_get("record_type").and_then(JsonValue::as_str) == Some("edition")
            && entry
                .object_get("links")
                .and_then(|x| x.object_get("embodies_expression_refs"))
                .and_then(JsonValue::as_array)
                .is_some_and(|rows| {
                    rows.iter()
                        .any(|x| x.as_str() == Some(owner.expression_id.as_str()))
                })
        {
            editions.insert(
                id.clone(),
                catalog_record(
                    fs,
                    cut,
                    entry,
                    &mut digests,
                    &mut total,
                    deadline,
                    cancelled,
                )?,
            );
        }
    }
    let mut selected = BTreeMap::new();
    for (id, entry) in &claims {
        if entry.object_get("predicate").and_then(JsonValue::as_str) == Some("embodied_by")
            && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                == Some(owner.expression_id.as_str())
        {
            selected.insert(
                id.clone(),
                catalog_claim(
                    fs,
                    cut,
                    entry,
                    &mut digests,
                    &mut total,
                    deadline,
                    cancelled,
                )?,
            );
        }
    }
    let declared = cmd::array(&expression, "embodiment_claim_refs")?
        .iter()
        .map(|row| {
            row.as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition embodiment reference",
                ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let targets = selected
        .values()
        .map(|claim| cmd::text(claim, "object").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if declared.len() != selected.len()
        || declared.into_iter().collect::<BTreeSet<_>>() != selected.keys().cloned().collect()
        || targets != editions.keys().cloned().collect()
        || selected.len() != editions.len()
        || editions.values().any(|edition| {
            !edition
                .object_get("embodies_expression_refs")
                .and_then(JsonValue::as_array)
                .is_some_and(|rows| {
                    rows.iter()
                        .any(|id| id.as_str() == Some(owner.expression_id.as_str()))
                })
        })
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition complete forward/backlink closure differs",
        ));
    }
    for (id, claim) in selected {
        active(deadline, cancelled)?;
        let entry = &claims[&id];
        let path = cmd::text(entry, "source_claim_file_ref")?;
        if path.ends_with("/source-claims.jsonl") {
            let mut remaining = limits;
            remaining.max_total_bytes = remaining
                .max_total_bytes
                .checked_sub(native_bytes)
                .and_then(|n| n.checked_sub(total as u64))
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition cumulative native bytes",
                ))?;
            remaining.max_state_bytes = remaining.max_state_bytes.checked_sub(native_state).ok_or(
                SourceCommandError::Invalid("ExpressionEdition cumulative native state"),
            )?;
            let observation = tos_validation::native_compound::verify_expression_edition_from_cut(
                cut,
                worker,
                path,
                &serde_value(&claim)?,
                remaining,
                cancelled,
            )
            .map_err(item_error)?;
            let (actual, plan, _, _) = work_transaction::inspect_committed(
                fs,
                &observation.transaction_id,
                deadline,
                cancelled,
            )?;
            if observation.transport
                != tos_validation::native_compound::NativeTransportState::Committed
                || actual != observation.manifest_sha256
                || cmd::text(&plan.authorization, "schema_version")? != AUTHORIZATION
                || cmd::text(cmd::field(&plan.authorization, "scope")?, "expression_id")?
                    != owner.expression_id
                || !history["receipts"]
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("ExpressionEdition history"))?
                    .iter()
                    .any(|receipt| {
                        receipt["publication"]["transaction_id"].as_str()
                            == Some(observation.transaction_id.as_str())
                    })
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition native member lacks exact parent lineage",
                ));
            }
            if retained_transactions
                .insert(observation.transaction_id, actual.clone())
                .is_some_and(|old| old != actual)
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition retained transaction changed",
                ));
            }
            native_bytes = native_bytes.checked_add(observation.bytes_read).ok_or(
                SourceCommandError::Invalid("ExpressionEdition native byte overflow"),
            )?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition native state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else {
            legacy_topology_binding(
                fs,
                cut,
                entry,
                &claim,
                &mut digests,
                &mut total,
                &mut legacy,
                deadline,
                cancelled,
            )?;
        }
    }
    check_publication()?;
    Ok(ExpressionEditionCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: total as u64,
    })
}

fn selected_before(
    fs: &CreationFilesystem,
    owner: &ExpressionEditionOwner,
    cut: &CorpusCutReader,
    snapshot: &PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    snapshot.verify_current(fs, deadline, cancelled)?;
    let parent_ref = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ExpressionEdition parent"))?
        .0;
    let parent = walk(&fs.root, parent_ref, fs.uid)?;
    let mut before = BTreeMap::new();
    for name in [
        "expression.json",
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        let observed =
            work_transaction::read_at(&parent, name, fs.uid, 2_097_152, deadline, cancelled)?;
        match observed {
            Some(bytes) => {
                let member = cut
                    .current()
                    .member(&path)
                    .ok_or(SourceCommandError::Conflict(
                        "physical ExpressionEdition selected member absent from cut",
                    ))?;
                if member.sha256 != Digest256::of_bytes(&bytes)
                    || member.size_bytes != bytes.len() as u64
                {
                    return Err(SourceCommandError::Conflict(
                        "physical ExpressionEdition member differs from selected cut",
                    ));
                }
                let selected = cut
                    .read_member(
                        cut.current().revision(),
                        &path,
                        2_097_152,
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| {
                        SourceCommandError::Unsupported(
                            "selected ExpressionEdition custody read incomplete",
                        )
                    })?;
                if selected.raw != bytes {
                    return Err(SourceCommandError::Conflict(
                        "ExpressionEdition cut and current bytes differ",
                    ));
                }
                before.insert(name.to_owned(), bytes);
            }
            None if name == "expression.json" || cut.current().member(&path).is_some() => {
                return Err(SourceCommandError::Conflict(
                    "selected ExpressionEdition member missing",
                ));
            }
            None => (),
        }
    }
    if before.values().map(Vec::len).sum::<usize>() > 8_388_608 {
        return Err(SourceCommandError::Invalid(
            "selected ExpressionEdition package byte budget",
        ));
    }
    snapshot.verify_current(fs, deadline, cancelled)?;
    Ok(before)
}

fn original_before_from_cut(
    owner: &ExpressionEditionOwner,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent_ref = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition original parent",
        ))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "expression.json",
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        if cut.current().member(&path).is_none() {
            if name == "expression.json" {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition original record absent",
                ));
            }
            continue;
        }
        let raw = cut
            .read_member(
                cut.current().revision(),
                &path,
                2_097_152,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported("ExpressionEdition original cut custody incomplete")
            })?
            .raw;
        total = total
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition original package overflow",
            ))?;
        if total > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "ExpressionEdition original package budget",
            ));
        }
        before.insert(name.to_owned(), raw);
    }
    Ok(before)
}

const IMPLEMENTATIONS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_compound_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py",
    "scripts/source_bibliographic_topology.py",
    "scripts/source_metadata_snapshot.py",
    "scripts/source_witness_human_forms.py",
    "scripts/source_record_profiles.py",
    "scripts/build_source_witness_catalog.py",
];

fn dependency_bindings(
    catalog: &ExpressionEditionCatalog,
    grammar: &BTreeMap<String, String>,
    ctx: &CommandContext,
) -> SourceCommandResult<JsonValue> {
    if catalog.digests.len() + grammar.len() + IMPLEMENTATIONS.len() > 512
        || catalog.retained_transactions.len() > 128
    {
        return Err(SourceCommandError::Invalid(
            "ExpressionEdition dependency binding budget",
        ));
    }
    let mut implementation = BTreeMap::new();
    for path in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "ExpressionEdition software subset incomplete",
            ))?;
        if raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid(
                "ExpressionEdition software member budget",
            ));
        }
        implementation.insert((*path).to_owned(), raw_hex(raw));
    }
    Ok(cmd::object(vec![
        ("catalog_and_sources", digest_map(&catalog.digests)),
        ("contracts", digest_map(grammar)),
        ("implementation", digest_map(&implementation)),
        (
            "retained_transactions",
            digest_map(&catalog.retained_transactions),
        ),
    ]))
}

fn remaining(
    catalog: &ExpressionEditionCatalog,
    mut limits: ItemLimits,
) -> SourceCommandResult<ItemLimits> {
    limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(catalog.native_bytes)
        .and_then(|n| n.checked_sub(catalog.source_bytes))
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition whole read budget",
        ))?;
    limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(catalog.native_state)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition whole state budget",
        ))?;
    Ok(limits)
}

impl ExpressionEditionOwner {
    fn new_directories(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<RelativePath>> {
        active(deadline, cancelled)?;
        let home = self
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("ExpressionEdition Claim home"))?
            .0;
        let parent = home
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition Claim parent",
            ))?
            .0;
        let mut directories = Vec::new();
        if work_transaction::read_existing_parent(fs, parent)?.is_none() {
            directories.push(relative(parent)?);
        }
        if work_transaction::read_existing_parent(fs, home)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition new Edition home occupied",
            ));
        }
        directories.push(relative(home)?);
        active(deadline, cancelled)?;
        Ok(directories)
    }
}
pub struct ExpressionEditionPreparation {
    request: JsonValue,
    projected_outputs: BTreeMap<String, Vec<u8>>,
}
impl ExpressionEditionPreparation {
    pub fn request(&self) -> &JsonValue {
        &self.request
    }
    pub fn projected_outputs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.projected_outputs
    }
}

pub fn prepare_isolated_expression_edition_from_proposal(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPreparation> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let mut owner = ExpressionEditionOwner::select(fs, ctx, true, limits.deadline, cancelled)?;
    complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    let before = selected_before(fs, &owner, cut, &snapshot, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        cut,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_current(fs, limits.deadline, cancelled),
        cancelled,
    )?;
    owner.new_directories(fs, limits.deadline, cancelled)?;
    let work = cmd::parse(
        before
            .get("expression.json")
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition preview selected record absent",
            ))?,
    )?;
    let mut refs = cmd::array(&work, "embodiment_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let claim = cmd::field(&owner.request, "claim")?;
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let remaining_limits = remaining(&catalog, limits)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_expression_edition_preview_bytes(
        cut,
        worker,
        &scope,
        &claim_raw,
        &before,
        &ctx.recorded_at,
        remaining_limits,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                let mut request = owner.request.clone();
                cmd::set(&mut request, "operation", cmd::string(OPERATION))?;
                cmd::set(
                    &mut request,
                    "command_id",
                    cmd::string("preview:uncommitted"),
                )?;
                cmd::set(
                    &mut request,
                    "fields",
                    cmd::object(vec![("embodiment_claim_refs", JsonValue::Array(refs))]),
                )?;
                cmd::set(
                    &mut request,
                    "expected_configuration",
                    cmd::string(&owner.configuration_digest),
                )?;
                cmd::set(
                    &mut request,
                    "expected_source",
                    cmd::reference(&work, "record_id", "record_version")?,
                )?;
                cmd::set(
                    &mut request,
                    "expected_revision",
                    cmd::string(&crate::source_revisions::revision(&before)?),
                )?;
                cmd::set(
                    &mut request,
                    "expected_dependencies",
                    cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
                )?;
                cmd::set(
                    &mut request,
                    "expected_publication",
                    snapshot
                        .token
                        .as_ref()
                        .map_or(JsonValue::Null, |value| cmd::string(value)),
                )?;
                owner.request = request.clone();
                let authorization = serde_value(&owner.authorization(dependencies)?)?;
                let mut raw = cmd::canonical(&request)?;
                raw.push(b'\n');
                prepared_request = Some(request);
                Ok((raw, authorization))
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("ExpressionEdition preview authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "ExpressionEdition preview full request absent",
    ))?;
    let authorization = foundation_value(core.authorization())?;
    let outputs = core.outputs().map_err(item_error)?;
    let mut all_reads = catalog.native_reads;
    all_reads.extend_from_slice(core.reads());
    selected_reads_current(
        fs,
        cut,
        &all_reads,
        &BTreeMap::new(),
        &outputs,
        WorkControlRead::Ready(&snapshot),
        limits.max_total_bytes,
        limits.deadline,
        cancelled,
    )?;
    drop(outputs);
    let projected_outputs = core.into_prepared_outputs().map_err(item_error)?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    software_current(fs, ctx, limits.deadline, cancelled)?;
    work_dependencies_current(
        fs,
        cmd::field(&authorization, "dependency_bindings")?,
        limits.deadline,
        cancelled,
    )?;
    complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    Ok(ExpressionEditionPreparation {
        request,
        projected_outputs,
    })
}

pub struct ExpressionEditionPublication {
    transaction_id: String,
    manifest_sha256: String,
    publication: JsonValue,
    receipt: JsonValue,
    replayed: bool,
}
impl ExpressionEditionPublication {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn publication(&self) -> &JsonValue {
        &self.publication
    }
    pub fn receipt(&self) -> &JsonValue {
        &self.receipt
    }
    pub fn replayed(&self) -> bool {
        self.replayed
    }
}
struct PreparedExpressionEditionApplication {
    plan: work_transaction::WorkPlan,
    guard: WorkApplicationGuard,
    receipt: JsonValue,
}
fn finished_receipt(child: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        child
            .get("expression-edition-receipt.json")
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition finished receipt absent",
            ))?,
    )
}

fn prepare_edition_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedExpressionEditionApplication> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let prior_id =
        original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let owner = ExpressionEditionOwner::select(fs, ctx, false, limits.deadline, cancelled)?;
    let requested_publication = cmd::field(&owner.request, "expected_publication")?;
    if requested_publication.as_str() != snapshot.token.as_deref()
        && !(requested_publication == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition requested publication snapshot changed",
        ));
    }
    let before = selected_before(fs, &owner, cut, &snapshot, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        cut,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_current(fs, limits.deadline, cancelled),
        cancelled,
    )?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_expression_edition_bytes(
        cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &ctx.recorded_at,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "ExpressionEdition current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("ExpressionEdition owner authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_expression_edition(
        &owner.request,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        owner.claim_path.as_str().rsplit_once('/').unwrap().0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_expression_edition_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let mut read_observations = catalog.native_reads;
    read_observations.extend(finished.reads);
    let plan = owner.plan(
        authorization,
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    let selected = selected_sides(&plan)?;
    after_cut_budget(cut, &selected)?;
    // The worker operation must finish before a protected source mutation.
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let old = cmd::parse(
        before
            .get("expression.json")
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition parent absent",
            ))?,
    )?;
    let stored_archive = work_transaction::expression_archive(
        fs,
        owner.expression_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        true,
    )?;
    if stored_archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained archive locator differs from recipe",
        ));
    }
    let authorization = plan.authorization.clone();
    Ok(PreparedExpressionEditionApplication {
        plan,
        guard: WorkApplicationGuard {
            snapshot,
            prior_id,
            selected,
            read_observations,
            stored_archive,
            authorization,
        },
        receipt,
    })
}

fn apply_edition_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    prepared: PreparedExpressionEditionApplication,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPublication> {
    let PreparedExpressionEditionApplication {
        plan,
        guard,
        receipt,
    } = prepared;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let applied = fence.apply(
        plan,
        &guard.snapshot,
        |summary, extent| guard.check(fs, ctx, cut, summary, extent, limits, cancelled),
        limits.deadline,
        cancelled,
    )?;
    if !applied.committed {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition selected publication rolled back",
        ));
    }
    Ok(ExpressionEditionPublication {
        transaction_id: applied.transaction_id,
        manifest_sha256: applied.manifest_sha256,
        publication: applied.publication,
        receipt,
        replayed: false,
    })
}

/// Publish one declared Expression/Edition link through the existing exact selected
/// journal only after current owner/cut/capture checks and the worker FINAL.
pub fn execute_isolated_expression_edition_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPublication> {
    if work_transaction::read_pending(fs, limits.deadline, cancelled)?.is_some() {
        return recover_edition_selected(
            fs,
            ctx,
            cut,
            software,
            components,
            worker,
            ExpressionEditionRecoveryDecision::Resume,
            false,
            limits,
            cancelled,
        );
    }
    let prepared = prepare_edition_application(
        fs, ctx, cut, software, components, worker, limits, cancelled,
    )?;
    apply_edition_application(fs, ctx, cut, prepared, limits, cancelled)
}

pub fn replay_isolated_expression_edition_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner =
        ExpressionEditionOwner::select(fs, original_ctx, false, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let transaction_id = owner.transaction_id()?;
    let (manifest_sha256, retained, base, terminal) =
        work_transaction::inspect_committed(fs, &transaction_id, limits.deadline, cancelled)?;
    let requested = cmd::field(&owner.request, "expected_publication")?;
    if cmd::field(&base, "token")? != requested
        || cmd::text(&retained.authorization, "schema_version")? != AUTHORIZATION
        || !cmd::same(
            &retained.authorization,
            &owner.authorization(
                cmd::field(&retained.authorization, "dependency_bindings")?.clone(),
            )?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition original replay basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition original revision differs",
        ));
    }
    let work_home = owner.expression_path.as_str().rsplit_once('/').unwrap().0;
    let expression_home = owner.claim_path.as_str().rsplit_once('/').unwrap().0;
    let mut parent = BTreeMap::new();
    let mut child = BTreeMap::new();
    for file in &retained.files {
        let path = file.path.as_str();
        let (home, name) = path.rsplit_once('/').ok_or(SourceCommandError::Invalid(
            "ExpressionEdition retained selected path",
        ))?;
        let after = file.after.as_ref().ok_or(SourceCommandError::Conflict(
            "ExpressionEdition retained selected output absent",
        ))?;
        if home == work_home {
            parent.insert(name.to_owned(), after.clone());
        } else if home == expression_home {
            child.insert(name.to_owned(), after.clone());
        } else {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition retained selected path outside scope",
            ));
        }
    }
    let expected = owner.plan(
        retained.authorization.clone(),
        &before,
        parent,
        child,
        retained.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &retained)? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained plan differs from original cut",
        ));
    }
    let receipt_path = format!("{expression_home}/expression-edition-receipt.json");
    let receipt_raw = retained
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition retained receipt absent",
        ))?;
    let receipt = cmd::parse(receipt_raw)?;
    let claims_path = relative(&format!("{expression_home}/source-claims.jsonl"))?;
    let claims_raw = current_cut
        .read_member(
            current_cut.current().revision(),
            &claims_path,
            2_097_152,
            limits.deadline,
            cancelled,
        )
        .map_err(|_| {
            SourceCommandError::Unsupported("current ExpressionEdition Claim custody incomplete")
        })?
        .raw;
    let mut current_claim = None;
    for line in catalog_lines(&claims_raw) {
        active(limits.deadline, cancelled)?;
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        let claim = cmd::parse(line)?;
        if cmd::text(&claim, "claim_id")? == owner.claim_id {
            if current_claim.replace(claim).is_some() {
                return Err(SourceCommandError::Conflict(
                    "duplicate current ExpressionEdition Claim",
                ));
            }
        }
    }
    let current_claim = current_claim.ok_or(SourceCommandError::Conflict(
        "current ExpressionEdition Claim absent",
    ))?;
    let observation = tos_validation::native_compound::verify_expression_edition_from_cut(
        current_cut,
        worker,
        claims_path.as_str(),
        &serde_value(&current_claim)?,
        limits,
        cancelled,
    )
    .map_err(item_error)?;
    if observation.transport != tos_validation::native_compound::NativeTransportState::Committed
        || observation.transaction_id != transaction_id
        || observation.manifest_sha256 != manifest_sha256
        || observation.claim_id != owner.claim_id
        || observation.work_parent_transition_sha256.as_deref()
            != Some(cmd::text(&receipt, "parent_transition_sha256")?)
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition committed native lineage differs",
        ));
    }
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    fs.current_context(original_ctx, limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;
    complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let (current_manifest, _, _, _) =
        work_transaction::inspect_committed(fs, &transaction_id, limits.deadline, cancelled)?;
    if current_manifest != manifest_sha256 {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained replay changed",
        ));
    }
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(ExpressionEditionPublication {
        transaction_id,
        manifest_sha256,
        publication: terminal,
        receipt,
        replayed: true,
    })
}

#[derive(Clone, Copy)]
pub enum ExpressionEditionRecoveryDecision {
    Resume,
    Rollback,
}

fn retained_owner(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    pending: &work_transaction::PendingWork,
    explicit: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionOwner> {
    let authority = &pending.plan.authorization;
    cmd::exact_keys(
        authority,
        &[
            "schema_version",
            "scope",
            "principal_id",
            "maker_type",
            "authority_ref",
            "owner_configuration",
            "command_id",
            "request_digest",
            "dependency_bindings",
        ],
    )?;
    if cmd::text(authority, "schema_version")? != AUTHORIZATION {
        return Err(SourceCommandError::Denied(
            "ExpressionEdition retained authority family",
        ));
    }
    let scope = cmd::field(authority, "scope")?;
    cmd::exact_keys(scope, SCOPE_KEYS)?;
    let mut grant = ExpressionEditionGrant::select(
        fs,
        ctx,
        Some(if explicit { RECOVERY } else { OPERATION }),
        limits.deadline,
        cancelled,
    )?;
    for key in SCOPE_KEYS.iter().filter(|key| !key.starts_with("allowed_")) {
        if !cmd::same(
            cmd::field(&grant.configuration, key)?,
            cmd::field(scope, key)?,
        )? {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition recovery changes frozen endpoint scope",
            ));
        }
    }
    let home = grant
        .claim_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition retained Claim home",
        ))?
        .0;
    let request_path = format!("{home}/source-create-request.json");
    let raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == request_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition retained request absent",
        ))?;
    let request = cmd::parse(raw)?;
    if cmd::record_digest(&request)?.to_prefixed() != cmd::text(authority, "request_digest")?
        || cmd::field(&request, "command_id")? != cmd::field(authority, "command_id")?
        || cmd::field(&request, "expected_configuration")?
            != cmd::field(authority, "owner_configuration")?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained request authority differs",
        ));
    }
    selected_forms(&request, "forms", &grant.expression_forms)?;
    selected_forms(&request, "claim_forms", &grant.claim_forms)?;
    selected_forms(&request, "edition_forms", &grant.edition_forms)?;
    if !explicit
        && (!cmd::same(&request, &cmd::parse(&ctx.request_raw)?)?
            || cmd::record_digest(&grant.configuration)?.to_prefixed()
                != cmd::text(authority, "owner_configuration")?)
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition exact retry changed original grant or request",
        ));
    }
    // Reconstruct historical scope only from the exact selected manifest.
    // Current grant selection above remains the sole permission/expiry source.
    for key in SCOPE_KEYS {
        cmd::set(
            &mut grant.configuration,
            key,
            cmd::field(scope, key)?.clone(),
        )?;
    }
    for key in ["principal_id", "maker_type", "authority_ref"] {
        cmd::set(
            &mut grant.configuration,
            key,
            cmd::field(authority, key)?.clone(),
        )?;
    }
    if !cmd::nonblank(cmd::text(&grant.configuration, "principal_id")?)
        || !cmd::nonblank(cmd::text(&grant.configuration, "authority_ref")?)
        || !matches!(
            cmd::text(&grant.configuration, "maker_type")?,
            "human" | "software" | "model"
        )
    {
        return Err(SourceCommandError::Denied(
            "ExpressionEdition historical actor shape",
        ));
    }
    grant.expression_forms = allowed_forms(&grant.configuration, "allowed_expression_form_ids")?;
    grant.claim_forms = allowed_forms(&grant.configuration, "allowed_claim_form_ids")?;
    grant.edition_forms = allowed_forms(&grant.configuration, "allowed_edition_form_ids")?;
    let owner = ExpressionEditionOwner::from_grant(
        grant,
        raw,
        false,
        Some(cmd::text(authority, "owner_configuration")?),
    )?;
    if owner.transaction_id()? != pending.plan.transaction_id
        || !cmd::same(
            authority,
            &owner.authorization(cmd::field(authority, "dependency_bindings")?.clone())?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained original identity differs",
        ));
    }
    Ok(owner)
}

fn recover_edition_selected(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: ExpressionEditionRecoveryDecision,
    explicit: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("ExpressionEdition exact pending transaction absent"),
    )?;
    let owner = retained_owner(fs, original_ctx, &pending, explicit, limits, cancelled)?;
    if explicit {
        let request = cmd::parse(&original_ctx.request_raw)?;
        cmd::exact_keys(
            &request,
            &[
                "schema_version",
                "operation",
                "transaction_id",
                "decision",
                "expected_configuration",
            ],
        )?;
        let config = cmd::parse(&original_ctx.configuration_raw)?;
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")? != RECOVERY
            || cmd::text(&request, "expected_configuration")?
                != cmd::record_digest(&config)?.to_prefixed()
            || cmd::text(&request, "transaction_id")? != pending.plan.transaction_id
            || cmd::text(&request, "decision")?
                != if matches!(decision, ExpressionEditionRecoveryDecision::Rollback) {
                    "rollback"
                } else {
                    "resume"
                }
        {
            return Err(SourceCommandError::Denied(
                "ExpressionEdition recovery exact current delegation/decision",
            ));
        }
    }
    let transaction_id = owner.transaction_id()?;
    if cmd::text(&pending.state, "transaction_id")? != transaction_id
        || cmd::field(&pending.base_publication, "token")?
            != cmd::field(&owner.request, "expected_publication")?
        || pending.plan.transaction_id != transaction_id
        || cmd::text(&pending.plan.authorization, "schema_version")? != AUTHORIZATION
        || !cmd::same(
            &pending.plan.authorization,
            &owner.authorization(
                cmd::field(&pending.plan.authorization, "dependency_bindings")?.clone(),
            )?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending owner/request basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending predecessor revision differs",
        ));
    }
    let old = cmd::parse(
        before
            .get("expression.json")
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition pending predecessor absent",
            ))?,
    )?;
    let archive = work_transaction::expression_archive(
        fs,
        owner.expression_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        false,
    )?;
    let publication_token = cmd::field(&owner.request, "expected_publication")?.as_str();
    let prior_id =
        original_prior_publication(original_cut, publication_token, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        original_cut,
        worker,
        limits,
        publication_token,
        || work_transaction::still_pending(fs, &pending.state, limits.deadline, cancelled),
        cancelled,
    )?;
    let receipt_path = format!(
        "{}/expression-edition-receipt.json",
        owner.claim_path.as_str().rsplit_once('/').unwrap().0
    );
    let retained_receipt_raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition pending receipt absent",
        ))?;
    let recorded_at = cmd::text(&cmd::parse(retained_receipt_raw)?, "recorded_at")?.to_owned();
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_expression_edition_bytes(
        original_cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &recorded_at,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, original_ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "ExpressionEdition pending dependencies changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("ExpressionEdition recovery authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != transaction_id || core.archive_path() != archive.path {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending recipe identity differs",
        ));
    }
    if !cmd::same(
        &foundation_value(core.authorization())?,
        &pending.plan.authorization,
    )? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending owner recipe differs",
        ));
    }
    let expression_home = owner.claim_path.as_str().rsplit_once('/').unwrap().0;
    let selected_after = |name: &str| -> SourceCommandResult<&[u8]> {
        let path = format!("{expression_home}/{name}");
        pending
            .plan
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
            .and_then(|file| file.after.as_deref())
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition pending native capture absent",
            ))
    };
    let finished = tos_validation::native_compound::restore_expression_edition_bytes(
        core,
        selected_after("source-create-environment.json")?,
        selected_after("source-create-provenance.jsonl")?,
        worker,
    )
    .map_err(item_error)?;
    let receipt = finished_receipt(&finished.child)?;
    let expected = owner.plan(
        pending.plan.authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        pending.plan.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &pending.plan)? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending plan differs from native recipe",
        ));
    }
    let selected = selected_sides(&pending.plan)?;
    let mut read_observations = catalog.native_reads;
    read_observations.extend(finished.reads);
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let held_pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("ExpressionEdition selected pending vanished before recovery"),
    )?;
    if !cmd::same(&held_pending.state, &pending.state)?
        || !cmd::same(&held_pending.base_publication, &pending.base_publication)?
        || !same_selected_plan(&held_pending.plan, &pending.plan)?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition pending changed before recovery lock",
        ));
    }
    let authorization = pending.plan.authorization.clone();
    let guard = |summary: &JsonValue, extent: work_transaction::WorkGuard<'_>| {
        if !cmd::same(cmd::field(summary, "authorization")?, &authorization)? {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition pending authorization changed",
            ));
        }
        physical_current(
            fs,
            original_ctx,
            original_cut,
            &selected,
            cmd::field(&authorization, "dependency_bindings")?,
            None,
            Some(&archive),
            prior_id.as_deref().zip(publication_token),
            &extent,
            limits.deadline,
            cancelled,
        )?;
        let pending_state = extent.pending_state.ok_or(SourceCommandError::Conflict(
            "ExpressionEdition recovery publication is not pending",
        ))?;
        selected_reads_current(
            fs,
            original_cut,
            &read_observations,
            &selected,
            &[],
            WorkControlRead::Pending(pending_state),
            limits.max_total_bytes,
            limits.deadline,
            cancelled,
        )
    };
    let rollback = matches!(decision, ExpressionEditionRecoveryDecision::Rollback);
    let current = cmd::parse(&original_ctx.configuration_raw)?;
    let renewal = if explicit {
        Some(cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_expression_edition_recovery_authorization_v1"),
            ),
            (
                "principal_id",
                cmd::field(&current, "principal_id")?.clone(),
            ),
            (
                "authority_ref",
                cmd::field(&current, "authority_ref")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::string(&cmd::record_digest(&current)?.to_prefixed()),
            ),
            ("transaction_id", cmd::string(&transaction_id)),
            (
                "decision",
                cmd::string(if rollback { "rollback" } else { "resume" }),
            ),
        ]))
    } else {
        None
    };
    let result = fence.recover(
        &held_pending,
        rollback,
        renewal,
        guard,
        limits.deadline,
        cancelled,
    )?;
    Ok(ExpressionEditionPublication {
        transaction_id: result.transaction_id,
        manifest_sha256: result.manifest_sha256,
        publication: result.publication,
        receipt: if result.committed {
            receipt
        } else {
            JsonValue::Null
        },
        replayed: false,
    })
}

pub fn recover_isolated_expression_edition_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: ExpressionEditionRecoveryDecision,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionPublication> {
    recover_edition_selected(
        fs,
        ctx,
        original_cut,
        software,
        components,
        worker,
        decision,
        true,
        limits,
        cancelled,
    )
}

pub(crate) fn current_result_fields(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    with_materializations: bool,
) -> SourceCommandResult<JsonValue> {
    let grant = ExpressionEditionGrant::select(fs, ctx, None, limits.deadline, cancelled)?;
    let config = grant.configuration;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let path = relative(cmd::text(&config, "expression_source_path")?)?;
    let home = path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition result parent path",
        ))?
        .0;
    let parent = walk(&fs.root, home, fs.uid)?;
    let mut package = BTreeMap::new();
    for name in [
        "expression.json",
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        if let Some(raw) =
            work_transaction::read_at(&parent, name, fs.uid, 2_097_152, limits.deadline, cancelled)?
        {
            package.insert(name.to_owned(), raw);
        }
    }
    let edition_raw = package
        .get("expression.json")
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition result parent absent",
        ))?;
    let edition = cmd::parse(edition_raw)?;
    tos_validation::native_compound::inspect_record_history(
        &package,
        edition_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let scope = JsonValue::Object(
        config
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition result grant",
            ))?
            .iter()
            .filter(|(k, _)| k.as_str().is_some_and(|k| SCOPE_KEYS.contains(&k)))
            .cloned()
            .collect(),
    );
    let mut profiles = Vec::new();
    let entity = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let relation = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let corpus = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/contracts/corpus-record.schema.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    for kind in ["expression", "edition"] {
        let matching = entity["types"]
            .as_array()
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition entity registry",
            ))?
            .iter()
            .filter(|entry| {
                entry["source_mappings"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["source_graph"] == "source-claims" && row["source_kind_id"] == kind
                    })
                })
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "ExpressionEdition descriptor mapping ambiguous",
            ));
        }
        let value = serde_json::json!({"type_id":matching[0]["type_id"],"record_type":kind,"schema_ref":"ToS/contracts/corpus-record.schema.json","schema_version":corpus["properties"]["schema_version"]["const"],"source_basename":format!("{kind}.json")});
        profiles.push((kind, foundation_value(&value)?));
    }
    let matching = relation["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition relation registry",
        ))?
        .iter()
        .filter(|entry| {
            entry["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == "embodied_by"
                        && row["scope"] == "claim-predicate"
                })
            })
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionEdition relation descriptor ambiguous",
        ));
    }
    let profile = &matching[0]["source_claim_profile"];
    let routes = profile["schemas"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition relation schema routes",
        ))?
        .iter()
        .filter(|row| row["schema_version"] == "tos_source_relation_claim_v1")
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionEdition Claim descriptor ambiguous",
        ));
    }
    profiles.push(("embodied_by",foundation_value(&serde_json::json!({"relation_type_id":matching[0]["relation_type_id"],"predicate":"embodied_by","reader":profile["reader"],"schema_ref":routes[0]["schema_ref"],"schema_version":routes[0]["schema_version"],"assertion_layers":profile["assertion_layers"],"source_basename":"source-claims.jsonl"}))?));
    let materializations = if with_materializations {
        let child_home = cmd::text(&config, "edition_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition result Claim home",
            ))?
            .0;
        let child_directory = walk(&fs.root, child_home, fs.uid)?;
        let mut child = BTreeMap::new();
        for name in [
            "edition.json".to_owned(),
            "edition.human-forms.json".to_owned(),
            "source-claims.jsonl".to_owned(),
            format!(
                "source-claims.{}.human-forms.json",
                Digest256::of_bytes(cmd::text(&config, "claim_id")?.as_bytes()).to_hex()
            ),
        ] {
            let raw = work_transaction::read_at(
                &child_directory,
                &name,
                fs.uid,
                2_097_152,
                limits.deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition result current Claim/form absent",
            ))?;
            child.insert(name, raw);
        }
        compound_views(&package, &child, cmd::text(&config, "claim_id")?)?
    } else {
        JsonValue::Null
    };
    let result = cmd::object(vec![
        (
            "source",
            cmd::reference(&edition, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&package)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_ref()
                .map_or(JsonValue::Null, |v| cmd::string(v)),
        ),
        ("scope", scope),
        ("materializations", materializations),
        ("source_profiles", cmd::object(profiles)),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&edition)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
    ]);
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    Ok(result)
}

fn compound_views(
    parent: &BTreeMap<String, Vec<u8>>,
    child: &BTreeMap<String, Vec<u8>>,
    claim_id: &str,
) -> SourceCommandResult<JsonValue> {
    let selected =
        |map: &BTreeMap<String, Vec<u8>>, name: &str| -> SourceCommandResult<JsonValue> {
            cmd::parse(map.get(name).ok_or(SourceCommandError::Conflict(
                "ExpressionEdition materialization buffer absent",
            ))?)
        };
    let expression = selected(parent, "expression.json")?;
    let claim = selected(child, "source-claims.jsonl")?;
    let edition = selected(child, "edition.json")?;
    Ok(cmd::object(vec![
        (
            "expression",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &expression,
                &selected(parent, "expression.human-forms.json")?,
            )?),
        ),
        (
            "edition",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &edition,
                &selected(child, "edition.human-forms.json")?,
            )?),
        ),
        (
            "claim",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &claim,
                &selected(
                    child,
                    &format!(
                        "source-claims.{}.human-forms.json",
                        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
                    ),
                )?,
            )?),
        ),
    ]))
}
impl ExpressionEditionPreparation {
    pub(crate) fn result_fields(
        &self,
        configuration: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        let expression_home = cmd::text(configuration, "expression_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition preview parent home",
            ))?
            .0;
        let claim_home = cmd::text(configuration, "edition_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition preview Claim home",
            ))?
            .0;
        let select = |home: &str| {
            self.projected_outputs
                .iter()
                .filter_map(|(path, raw)| {
                    path.strip_prefix(&format!("{home}/"))
                        .filter(|name| !name.contains('/'))
                        .map(|name| (name.to_owned(), raw.clone()))
                })
                .collect::<BTreeMap<_, _>>()
        };
        let parent = select(expression_home);
        let child = select(claim_home);
        let expression = cmd::parse(parent.get("expression.json").ok_or(
            SourceCommandError::Conflict("ExpressionEdition projected record absent"),
        )?)?;
        let claim = cmd::parse(child.get("source-claims.jsonl").ok_or(
            SourceCommandError::Conflict("ExpressionEdition projected Claim absent"),
        )?)?;
        let mut forms = Vec::new();
        for (kind, files, name) in [
            (
                "expression",
                &parent,
                "expression.human-forms.json".to_owned(),
            ),
            ("edition", &child, "edition.human-forms.json".to_owned()),
            (
                "claim",
                &child,
                format!(
                    "source-claims.{}.human-forms.json",
                    Digest256::of_bytes(cmd::text(configuration, "claim_id")?.as_bytes()).to_hex()
                ),
            ),
        ] {
            let set = cmd::parse(files.get(&name).ok_or(SourceCommandError::Conflict(
                "ExpressionEdition projected form set absent",
            ))?)?;
            forms.push((
                kind,
                JsonValue::Array(
                    cmd::array(&set, "forms")?
                        .iter()
                        .map(crate::source_forms::form_reference)
                        .collect::<SourceCommandResult<Vec<_>>>()?,
                ),
            ));
        }
        Ok(cmd::object(vec![
            ("prepared_request", self.request.clone()),
            (
                "prepared_fields",
                cmd::field(&self.request, "fields")?.clone(),
            ),
            (
                "prepared_claim",
                cmd::reference(&claim, "claim_id", "claim_version")?,
            ),
            (
                "prepared_expression",
                cmd::reference(&expression, "record_id", "record_version")?,
            ),
            (
                "prepared_edition",
                cmd::reference(
                    cmd::field(&self.request, "record")?,
                    "record_id",
                    "record_version",
                )?,
            ),
            ("prepared_forms", cmd::object(forms)),
            (
                "prepared_materializations",
                compound_views(&parent, &child, cmd::text(configuration, "claim_id")?)?,
            ),
            (
                "expected_dependencies",
                cmd::field(&self.request, "expected_dependencies")?.clone(),
            ),
            (
                "expected_publication",
                cmd::field(&self.request, "expected_publication")?.clone(),
            ),
        ]))
    }
}
