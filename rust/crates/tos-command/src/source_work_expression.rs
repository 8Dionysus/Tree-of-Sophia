//! One protected Work→Expression owner. The selected journal moves exact
//! bytes; this module alone may construct its Work-specific authorization.

use super::work_transaction::{self, PublicationSnapshot};
use super::{CreationFilesystem, MAX_BYTES, MAX_FILES, active, member_mode_matches, scan, walk};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::PredicateRead;
use tos_validation::item_rules::ItemLimits;
use tos_validation::item_rules::ItemRefusal;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONFIG: &str = "tos_local_work_expression_owner_v1";
const REQUEST: &str = "tos_local_work_expression_command_v1";
const OPERATION: &str = "work.expression.create";
const RECOVERY: &str = "work.expression.recover";
const AUTHORIZATION: &str = "tos_work_expression_authorization_v1";
const SCOPE_KEYS: &[&str] = &[
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_work_form_ids",
    "allowed_expression_form_ids",
    "allowed_claim_form_ids",
];
const IMPLEMENTATIONS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_compound_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
    "scripts/source_bibliographic_topology.py",
    "scripts/source_metadata_snapshot.py",
    "scripts/source_witness_human_forms.py",
    "scripts/source_record_profiles.py",
    "scripts/build_source_witness_catalog.py",
];

fn item_error(reason: ItemRefusal) -> SourceCommandError {
    SourceCommandError::SchemaExecution {
        path: "work.expression.create".to_owned(),
        root: "selected Work/Expression mechanics".to_owned(),
        reason,
    }
}

fn serde_value(value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("Work JSON value conversion"))
}

fn foundation_value(value: &serde_json::Value) -> SourceCommandResult<JsonValue> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| SourceCommandError::Invalid("Work JSON value conversion"))?;
    cmd::parse(&raw)
}

fn finished_receipt(child: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        child
            .get("work-expression-receipt.json")
            .ok_or(SourceCommandError::Conflict("Work finished receipt absent"))?,
    )
}

struct WorkOwner {
    configuration: JsonValue,
    request: JsonValue,
    work_path: RelativePath,
    expression_path: RelativePath,
    work_id: String,
    expression_id: String,
    claim_id: String,
    event_id: String,
    principal: String,
    authority: String,
    maker: String,
    configuration_digest: String,
}

pub(super) fn slug(value: &str, prefix: &str) -> bool {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return false;
    };
    !suffix.is_empty()
        && suffix.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
pub(super) fn form_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("tos.form.") else {
        return false;
    };
    !suffix.is_empty()
        && (suffix.as_bytes()[0].is_ascii_lowercase() || suffix.as_bytes()[0].is_ascii_digit())
        && suffix.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}
pub(super) fn selected_forms(
    request: &JsonValue,
    field: &str,
    allowed: &BTreeSet<String>,
) -> SourceCommandResult<()> {
    let rows = cmd::array(request, field)?;
    if rows.is_empty() || rows.len() > 32 {
        return Err(SourceCommandError::Denied(
            "bounded Work source-copy selections",
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        cmd::exact_keys(row, &["form_id", "field_id"])?;
        let id = cmd::text(row, "form_id")?;
        if !allowed.contains(id) || !seen.insert(id) || cmd::text(row, "field_id")?.is_empty() {
            return Err(SourceCommandError::Denied(
                "Work form selection leaves grant",
            ));
        }
    }
    Ok(())
}
pub(super) fn allowed_forms(
    config: &JsonValue,
    field: &str,
) -> SourceCommandResult<BTreeSet<String>> {
    let rows = cmd::array(config, field)?;
    if rows.is_empty() || rows.len() > 32 {
        return Err(SourceCommandError::Denied("bounded Work form grant"));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let id = row
            .as_str()
            .ok_or(SourceCommandError::Invalid("Work form identity"))?;
        if !form_id(id) || !result.insert(id.to_owned()) {
            return Err(SourceCommandError::Denied("Work form grant identity"));
        }
    }
    Ok(result)
}
pub(super) fn relative(value: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(value).map_err(|_| SourceCommandError::Denied("Work source path"))
}
impl WorkOwner {
    fn scope(&self) -> SourceCommandResult<JsonValue> {
        Ok(JsonValue::Object(
            self.configuration
                .as_object()
                .ok_or(SourceCommandError::Invalid("Work scope"))?
                .iter()
                .filter(|(key, _)| key.as_str().is_some_and(|k| SCOPE_KEYS.contains(&k)))
                .cloned()
                .collect(),
        ))
    }

    fn plan(
        &self,
        authorization: JsonValue,
        before: &BTreeMap<String, Vec<u8>>,
        parent: BTreeMap<String, Vec<u8>>,
        child: BTreeMap<String, Vec<u8>>,
        new_directories: Vec<RelativePath>,
    ) -> SourceCommandResult<work_transaction::WorkPlan> {
        let work_home = self
            .work_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work selected home"))?
            .0;
        let expression_home = self
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Expression selected home"))?
            .0;
        let parent_names = [
            "work.json",
            "work.human-forms.json",
            "source-revision-history.json",
        ];
        if parent.len() != parent_names.len()
            || parent_names.iter().any(|name| !parent.contains_key(*name))
            || before
                .keys()
                .any(|name| !parent_names.contains(&name.as_str()))
        {
            return Err(SourceCommandError::Invalid(
                "Work selected parent package shape",
            ));
        }
        let claim_forms = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(self.claim_id.as_bytes()).to_hex()
        );
        let child_names = [
            "expression.json",
            "expression.human-forms.json",
            "source-claims.jsonl",
            claim_forms.as_str(),
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "work-expression-receipt.json",
        ];
        if child.len() != child_names.len()
            || child_names.iter().any(|name| !child.contains_key(*name))
        {
            return Err(SourceCommandError::Invalid(
                "Work selected child package shape",
            ));
        }
        let mut files = Vec::with_capacity(parent.len() + child.len());
        for (name, after) in parent {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{work_home}/{name}"))?,
                before: before.get(&name).cloned(),
                after: Some(after),
            });
        }
        for (name, after) in child {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{expression_home}/{name}"))?,
                before: None,
                after: Some(after),
            });
        }
        Ok(work_transaction::WorkPlan {
            transaction_id: self.transaction_id()?,
            authorization,
            item_path_profile: None,
            files,
            new_directories,
        })
    }

    fn transaction_id(&self) -> SourceCommandResult<String> {
        let request_digest = cmd::record_digest(&self.request)?.to_prefixed();
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
            ("request_digest", cmd::string(&request_digest)),
        ]))?
        .to_prefixed())
    }

    fn new_directories(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<RelativePath>> {
        active(deadline, cancelled)?;
        let child = self
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Expression home"))?
            .0;
        let parent = child
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Expression parent"))?
            .0;
        let mut new = Vec::new();
        if work_transaction::read_existing_parent(fs, parent)?.is_none() {
            new.push(relative(parent)?);
        }
        if work_transaction::read_existing_parent(fs, child)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "new Expression home is already occupied",
            ));
        }
        new.push(relative(child)?);
        active(deadline, cancelled)?;
        Ok(new)
    }

    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::select_request(fs, ctx, false, deadline, cancelled)
    }

    fn select_proposal(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::select_request(fs, ctx, true, deadline, cancelled)
    }

    fn select_request(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        proposal: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        fs.current_context(ctx, deadline, cancelled)?;
        let config = cmd::parse(&ctx.configuration_raw)?;
        cmd::exact_keys(
            &config,
            &[
                "schema_version",
                "uid",
                "principal_id",
                "maker_type",
                "source_root",
                "authority_ref",
                "expires_at",
                "allowed_operations",
                "work_id",
                "work_source_path",
                "expression_id",
                "expression_source_path",
                "claim_id",
                "provenance_event_id",
                "allowed_work_form_ids",
                "allowed_expression_form_ids",
                "allowed_claim_form_ids",
            ],
        )?;
        if cmd::text(&config, "schema_version")? != CONFIG
            || cmd::integer(&config, "uid")? != u64::from(fs.uid)
            || Path::new(cmd::text(&config, "source_root")?) != fs.root_path
        {
            return Err(SourceCommandError::Denied(
                "Work delegation root/account/schema",
            ));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let operations = cmd::array(&config, "allowed_operations")?;
        let mut allowed = BTreeSet::new();
        if operations.is_empty() || operations.len() > 2 {
            return Err(SourceCommandError::Denied("Work delegated operation count"));
        }
        for value in operations {
            let operation = value
                .as_str()
                .ok_or(SourceCommandError::Invalid("Work operation"))?;
            if !matches!(operation, OPERATION | RECOVERY) || !allowed.insert(operation) {
                return Err(SourceCommandError::Denied("Work delegated operation"));
            }
        }
        if !allowed.contains(OPERATION) {
            return Err(SourceCommandError::Denied("Work creation not delegated"));
        }
        let work_id = cmd::text(&config, "work_id")?.to_owned();
        let expression_id = cmd::text(&config, "expression_id")?.to_owned();
        let claim_id = cmd::text(&config, "claim_id")?.to_owned();
        let event_id = cmd::text(&config, "provenance_event_id")?.to_owned();
        if !slug(&work_id, "tos.work.")
            || !slug(&expression_id, "tos.expression.")
            || !slug(&claim_id, "tos.claim.")
            || !slug(&event_id, "tos.event.")
        {
            return Err(SourceCommandError::Denied("Work typed scope identities"));
        }
        let work_path = relative(cmd::text(&config, "work_source_path")?)?;
        let expression_path = relative(cmd::text(&config, "expression_source_path")?)?;
        let work_parts = work_path.as_str().split('/').collect::<Vec<_>>();
        let expression_parent = expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Expression source parent"))?
            .0;
        let work_parent = work_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Work source parent"))?
            .0;
        let expression_home = expression_parent
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Expression home"))?
            .1;
        if work_parts.len() < 5
            || work_parts[..3] != ["ToS", "source-witnesses", "works"]
            || work_parts.last() != Some(&"work.json")
            || !expression_path.as_str().ends_with("/expression.json")
            || !expression_parent.starts_with(&format!("{work_parent}/expressions/"))
            || expression_parent != format!("{work_parent}/expressions/{expression_home}")
            || !slug(expression_home, "")
        {
            return Err(SourceCommandError::Denied("Work/Expression home scope"));
        }
        let work_forms = allowed_forms(&config, "allowed_work_form_ids")?;
        let expression_forms = allowed_forms(&config, "allowed_expression_form_ids")?;
        let claim_forms = allowed_forms(&config, "allowed_claim_form_ids")?;
        if !work_forms.is_disjoint(&expression_forms)
            || !work_forms.is_disjoint(&claim_forms)
            || !expression_forms.is_disjoint(&claim_forms)
        {
            return Err(SourceCommandError::Denied("Work form identities overlap"));
        }
        let principal = cmd::text(&config, "principal_id")?.to_owned();
        let authority = cmd::text(&config, "authority_ref")?.to_owned();
        let maker = cmd::text(&config, "maker_type")?.to_owned();
        if principal.is_empty()
            || authority.is_empty()
            || !matches!(maker.as_str(), "human" | "software" | "model")
        {
            return Err(SourceCommandError::Denied("Work principal/maker/authority"));
        }
        let request = cmd::parse(&ctx.request_raw)?;
        if proposal {
            cmd::exact_keys(
                &request,
                &[
                    "schema_version",
                    "operation",
                    "record",
                    "claim",
                    "forms",
                    "expression_forms",
                    "claim_forms",
                    "reason",
                ],
            )?;
        } else {
            cmd::exact_keys(
                &request,
                &[
                    "schema_version",
                    "operation",
                    "command_id",
                    "fields",
                    "record",
                    "claim",
                    "forms",
                    "expression_forms",
                    "claim_forms",
                    "reason",
                    "expected_configuration",
                    "expected_source",
                    "expected_revision",
                    "expected_dependencies",
                    "expected_publication",
                ],
            )?;
        }
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
                "Work create request schema/operation/budget",
            ));
        }
        let reason = cmd::text(&request, "reason")?;
        if reason.is_empty() || reason.len() > 4096 || !cmd::nonblank(reason) {
            return Err(SourceCommandError::Invalid("Work command identity/reason"));
        }
        if !proposal {
            let command_id = cmd::text(&request, "command_id")?;
            if command_id.is_empty() || command_id.len() > 256 {
                return Err(SourceCommandError::Invalid("Work command identity"));
            }
            for field in [
                "expected_configuration",
                "expected_revision",
                "expected_dependencies",
            ] {
                if !super::work_transaction::is_hash(cmd::text(&request, field)?) {
                    return Err(SourceCommandError::Invalid("Work expected digest"));
                }
            }
            if cmd::field(&request, "expected_publication")? != &JsonValue::Null
                && !cmd::field(&request, "expected_publication")?
                    .as_str()
                    .is_some_and(super::work_transaction::is_hash)
            {
                return Err(SourceCommandError::Invalid(
                    "Work expected publication token",
                ));
            }
            if cmd::text(&request, "expected_configuration")?
                != cmd::record_digest(&config)?.to_prefixed()
            {
                return Err(SourceCommandError::Conflict(
                    "Work current owner digest differs",
                ));
            }
        }
        let record = cmd::field(&request, "record")?;
        let claim = cmd::field(&request, "claim")?;
        let maker_ref = cmd::object(vec![
            ("maker_type", cmd::string(&maker)),
            ("agent_ref", cmd::string(&principal)),
        ]);
        if cmd::text(record, "record_id")? != expression_id
            || cmd::text(record, "work_ref")? != work_id
            || cmd::text(claim, "claim_id")? != claim_id
            || cmd::text(claim, "subject_ref")? != work_id
            || cmd::text(claim, "object")? != expression_id
            || cmd::text(claim, "provenance_event_ref")? != event_id
            || !cmd::same(cmd::field(claim, "maker")?, &maker_ref)?
        {
            return Err(SourceCommandError::Denied(
                "Work record/Claim identity scope",
            ));
        }
        selected_forms(&request, "forms", &work_forms)?;
        selected_forms(&request, "expression_forms", &expression_forms)?;
        selected_forms(&request, "claim_forms", &claim_forms)?;
        let configuration_digest = cmd::record_digest(&config)?.to_prefixed();
        Ok(Self {
            configuration: config,
            request,
            work_path,
            expression_path,
            work_id,
            expression_id,
            claim_id,
            event_id,
            principal,
            authority,
            maker,
            configuration_digest,
        })
    }

    fn authorization(&self, dependencies: JsonValue) -> SourceCommandResult<JsonValue> {
        Ok(cmd::object(vec![
            ("schema_version", cmd::string(AUTHORIZATION)),
            ("scope", self.scope()?),
            ("principal_id", cmd::string(&self.principal)),
            ("maker_type", cmd::string(&self.maker)),
            ("authority_ref", cmd::string(&self.authority)),
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

/// Only selected Work package files are an input to the compound byte law;
/// descendants and generated catalog rows keep their distinct owner readers.
fn selected_before(
    fs: &CreationFilesystem,
    owner: &WorkOwner,
    cut: &CorpusCutReader,
    snapshot: &PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    snapshot.verify_current(fs, deadline, cancelled)?;
    let parent_ref = owner
        .work_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work parent"))?
        .0;
    let parent = walk(&fs.root, parent_ref, fs.uid)?;
    let mut before = BTreeMap::new();
    for name in [
        "work.json",
        "work.human-forms.json",
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
                        "physical Work selected member absent from cut",
                    ))?;
                if member.sha256 != Digest256::of_bytes(&bytes)
                    || member.size_bytes != bytes.len() as u64
                {
                    return Err(SourceCommandError::Conflict(
                        "physical Work member differs from selected cut",
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
                        SourceCommandError::Unsupported("selected Work custody read incomplete")
                    })?;
                if selected.raw != bytes {
                    return Err(SourceCommandError::Conflict(
                        "Work cut and current bytes differ",
                    ));
                }
                before.insert(name.to_owned(), bytes);
            }
            None if name == "work.json" || cut.current().member(&path).is_some() => {
                return Err(SourceCommandError::Conflict("selected Work member missing"));
            }
            None => (),
        }
    }
    if before.values().map(Vec::len).sum::<usize>() > 8_388_608 {
        return Err(SourceCommandError::Invalid(
            "selected Work package byte budget",
        ));
    }
    snapshot.verify_current(fs, deadline, cancelled)?;
    Ok(before)
}

fn original_before_from_cut(
    owner: &WorkOwner,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent_ref = owner
        .work_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work original parent"))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "work.json",
        "work.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        if cut.current().member(&path).is_none() {
            if name == "work.json" {
                return Err(SourceCommandError::Conflict("Work original record absent"));
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
            .map_err(|_| SourceCommandError::Unsupported("Work original cut custody incomplete"))?
            .raw;
        total = total
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Work original package overflow",
            ))?;
        if total > 8_388_608 {
            return Err(SourceCommandError::Invalid("Work original package budget"));
        }
        before.insert(name.to_owned(), raw);
    }
    Ok(before)
}

pub(super) fn original_prior_publication(
    cut: &CorpusCutReader,
    expected_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<String>> {
    let path = relative("ToS/source-witnesses/.metadata-publication.json")?;
    let Some(_) = cut.current().member(&path) else {
        if expected_token.is_some() {
            return Err(SourceCommandError::Conflict(
                "Work prior publication absent from cut",
            ));
        }
        return Ok(None);
    };
    let raw = cut
        .read_member(cut.current().revision(), &path, 8192, deadline, cancelled)
        .map_err(|_| SourceCommandError::Unsupported("Work prior publication custody incomplete"))?
        .raw;
    let state = cmd::parse(&raw)?;
    work_transaction::state(&state)?;
    if cmd::text(&state, "phase")? != "ready" || Some(cmd::text(&state, "token")?) != expected_token
    {
        return Err(SourceCommandError::Conflict(
            "Work prior publication differs from cut",
        ));
    }
    Ok(Some(cmd::text(&state, "transaction_id")?.to_owned()))
}

fn prior_completion_current(
    fs: &CreationFilesystem,
    id: &str,
    expected_token: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let (_, _, _, terminal) = work_transaction::inspect_committed(fs, id, deadline, cancelled)?;
    if cmd::text(&terminal, "token")? != expected_token {
        return Err(SourceCommandError::Conflict(
            "Work prior completion token changed",
        ));
    }
    Ok(format!(
        "ToS/source-witnesses/.metadata-transactions/{}/completion.json",
        &id[7..]
    ))
}

pub(super) fn complete_current_cut(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    snapshot: &PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    snapshot.verify_current(fs, deadline, cancelled)?;
    let tos = walk(&fs.root, "ToS", fs.uid)?;
    let mut observed = BTreeMap::new();
    let mut total = 0usize;
    let mut directories = 0usize;
    scan(
        &tos,
        "ToS",
        fs.uid,
        None,
        None,
        None,
        &mut observed,
        &mut total,
        &mut directories,
        deadline,
        cancelled,
    )?;
    if observed.len() > MAX_FILES
        || total > MAX_BYTES
        || observed.len() != cut.current().members().count()
    {
        return Err(SourceCommandError::Conflict(
            "Work current complete cut budget/membership",
        ));
    }
    for member in cut.current().members() {
        let Some((sha, size, mode)) = observed.remove(member.path.as_str()) else {
            return Err(SourceCommandError::Conflict(
                "Work current member disappeared",
            ));
        };
        if sha != member.sha256
            || size != member.size_bytes
            || !member_mode_matches(mode, member.mode, true)
        {
            return Err(SourceCommandError::Conflict(
                "Work current cut member changed",
            ));
        }
    }
    if !observed.is_empty() {
        return Err(SourceCommandError::Conflict(
            "Work current unselected member appeared",
        ));
    }
    snapshot.verify_current(fs, deadline, cancelled)
}

struct WorkCatalog {
    // Raw lowercase SHA-256 hex, exactly the maintained dependency dialect.
    digests: BTreeMap<String, String>,
    prior_claims: BTreeMap<String, JsonValue>,
    retained_transactions: BTreeMap<String, String>,
}

/// Exact isolated Work transaction outcome, with no source, rights, semantic,
/// publication-to-public, or canon admission claim.
pub struct WorkExpressionPublication {
    transaction_id: String,
    manifest_sha256: String,
    publication: JsonValue,
    receipt: JsonValue,
    replayed: bool,
}

/// A provisional create request derived from one protected current Work read.
/// It carries no publication, source, rights, or semantic authority; the
/// create entry independently selects and revalidates every input.
pub struct WorkExpressionPreparation {
    request: JsonValue,
    projected_outputs: BTreeMap<String, Vec<u8>>,
}
impl WorkExpressionPreparation {
    pub fn request(&self) -> &JsonValue {
        &self.request
    }
    pub fn projected_outputs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.projected_outputs
    }
}
impl WorkExpressionPublication {
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

pub(super) fn digest_map(values: &BTreeMap<String, String>) -> JsonValue {
    JsonValue::Object(
        values
            .iter()
            .map(|(key, value)| {
                (
                    tos_foundation::JsonString::from_utf8(key),
                    cmd::string(value),
                )
            })
            .collect(),
    )
}
fn selected_implementation(ctx: &CommandContext) -> SourceCommandResult<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for reference in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(reference)?)?
            .ok_or(SourceCommandError::Unsupported(
                "Work implementation software subset incomplete",
            ))?;
        if raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid(
                "Work implementation member byte budget",
            ));
        }
        result.insert((*reference).to_owned(), raw_hex(raw));
    }
    Ok(result)
}
fn dependency_bindings(
    catalog: &WorkCatalog,
    grammar: &BTreeMap<String, String>,
    ctx: &CommandContext,
) -> SourceCommandResult<JsonValue> {
    if catalog.digests.len() + grammar.len() + IMPLEMENTATIONS.len() > 512
        || catalog.retained_transactions.len() > 128
    {
        return Err(SourceCommandError::Invalid("Work dependency binding count"));
    }
    Ok(cmd::object(vec![
        ("catalog_and_sources", digest_map(&catalog.digests)),
        ("contracts", digest_map(grammar)),
        ("implementation", digest_map(&selected_implementation(ctx)?)),
        (
            "retained_transactions",
            digest_map(&catalog.retained_transactions),
        ),
    ]))
}
pub(super) fn raw_hex(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
pub(super) fn checked_current_source(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    reference: &str,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if !reference.starts_with("ToS/source-witnesses/") {
        return Err(SourceCommandError::Denied("Work catalog source locator"));
    }
    checked_current_member(fs, cut, reference, cap, deadline, cancelled)
}

/// Only the three current source descriptors used by owner result adapters.
pub(super) fn checked_current_descriptor(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    reference: &str,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if !matches!(
        reference,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            | "ToS/doctrine/semantic-interchange/relation-types.v1.json"
            | "ToS/contracts/corpus-record.schema.json"
    ) {
        return Err(SourceCommandError::Denied(
            "owner descriptor source locator",
        ));
    }
    checked_current_member(fs, cut, reference, cap, deadline, cancelled)
}

fn checked_current_member(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    reference: &str,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = relative(reference)?;
    let (parent_ref, name) = reference
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work catalog source parent"))?;
    let parent = walk(&fs.root, parent_ref, fs.uid)?;
    let raw = work_transaction::read_at(&parent, name, fs.uid, cap, deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("catalog selected source absent"),
    )?;
    let member = cut
        .current()
        .member(&path)
        .ok_or(SourceCommandError::Conflict(
            "catalog selected source absent from cut",
        ))?;
    if member.sha256 != Digest256::of_bytes(&raw) || member.size_bytes != raw.len() as u64 {
        return Err(SourceCommandError::Conflict(
            "catalog current source differs from cut",
        ));
    }
    let selected = cut
        .read_member(
            cut.current().revision(),
            &path,
            cap as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Unsupported("catalog source custody read incomplete"))?;
    if selected.raw != raw {
        return Err(SourceCommandError::Conflict(
            "catalog exact source bytes differ",
        ));
    }
    Ok(raw)
}
pub(super) fn catalog_lines(raw: &[u8]) -> impl Iterator<Item = &[u8]> {
    // Empty input has no physical line. A real blank line remains visible to
    // the caller, while a terminal LF does not create a further line.
    raw.split_inclusive(|b| *b == b'\n')
        .map(|line| line.strip_suffix(b"\n").unwrap_or(line))
}
pub(super) fn known_catalog_kind(kind: &str, basename: &str) -> bool {
    let expected = match kind {
        "agent" => "agents.jsonl",
        "place" => "places.jsonl",
        "organization" => "organizations.jsonl",
        "work" => "works.jsonl",
        "expression" => "expressions.jsonl",
        "edition" => "editions.jsonl",
        "collection" => "collections.jsonl",
        "item" => "items.jsonl",
        "link" => "links.jsonl",
        "historical-event" => "historical-events.jsonl",
        "historical-process" => "historical-processes.jsonl",
        "historical-state" => "historical-states.jsonl",
        "artifact" => "artifacts.jsonl",
        "composite" => "composites.jsonl",
        _ => return false,
    };
    basename == expected
}
fn current_catalog(
    fs: &CreationFilesystem,
    owner: &WorkOwner,
    before: &BTreeMap<String, Vec<u8>>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    publication_token: Option<&str>,
    mut check_publication: impl FnMut() -> SourceCommandResult<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkCatalog> {
    check_publication()?;
    let catalog = walk(&fs.root, "ToS/source-witnesses/catalog", fs.uid)?;
    let manifest_raw = work_transaction::read_at(
        &catalog,
        "catalog.manifest.json",
        fs.uid,
        2_097_152,
        deadline,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict("Work catalog manifest absent"))?;
    let manifest = cmd::parse(&manifest_raw)?;
    if cmd::text(&manifest, "schema_version")? != "tos_source_witness_catalog_v3"
        || cmd::text(&manifest, "claim_file")? != "ToS/source-witnesses/catalog/claims.jsonl"
    {
        return Err(SourceCommandError::Unsupported(
            "Work catalog route/version",
        ));
    }
    let record_files = cmd::field(&manifest, "record_files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Work catalog record routes"))?;
    if record_files.is_empty() || record_files.len() > 128 {
        return Err(SourceCommandError::Invalid("Work catalog route count"));
    }
    let mut routes = Vec::with_capacity(record_files.len() + 1);
    for (kind, value) in record_files {
        let kind = kind
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog kind"))?;
        let reference = value
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog route"))?;
        let prefix = "ToS/source-witnesses/catalog/";
        if !reference.starts_with(prefix) || !known_catalog_kind(kind, &reference[prefix.len()..]) {
            return Err(SourceCommandError::Unsupported(
                "Work catalog undeclared profile route",
            ));
        }
        routes.push((kind.to_owned(), reference.to_owned()));
    }
    routes.sort();
    routes.push((
        "claim".to_owned(),
        "ToS/source-witnesses/catalog/claims.jsonl".to_owned(),
    ));
    let mut digests = BTreeMap::new();
    let mut record_ids = BTreeSet::new();
    let mut claim_ids = BTreeSet::new();
    let mut expressions = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut total_bytes = manifest_raw.len();
    let mut count = 0usize;
    for (kind, reference) in routes {
        active(deadline, cancelled)?;
        let leaf = reference
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("catalog leaf"))?
            .1;
        let raw =
            work_transaction::read_at(&catalog, leaf, fs.uid, 16_777_216, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict("Work catalog route absent"))?;
        total_bytes = total_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Work catalog aggregate overflow",
            ))?;
        if total_bytes > 16_777_216 {
            return Err(SourceCommandError::Invalid(
                "Work catalog aggregate byte budget",
            ));
        }
        digests.insert(reference.clone(), raw_hex(&raw));
        for line in catalog_lines(&raw) {
            active(deadline, cancelled)?;
            count += 1;
            if count > 8192 {
                return Err(SourceCommandError::Invalid("Work catalog row budget"));
            }
            let entry = cmd::parse(line)?;
            let id = if kind == "claim" {
                if cmd::text(&entry, "schema_version")?
                    != "tos_source_witness_claim_catalog_entry_v1"
                {
                    return Err(SourceCommandError::Invalid("Work claim catalog schema"));
                }
                cmd::text(&entry, "claim_id")?
            } else {
                if cmd::text(&entry, "schema_version")? != "tos_source_witness_catalog_entry_v1"
                    || cmd::text(&entry, "record_type")? != kind
                {
                    return Err(SourceCommandError::Invalid("Work record catalog schema"));
                }
                cmd::text(&entry, "record_id")?
            };
            if record_ids.contains(id)
                || claim_ids.contains(id)
                || !(if kind == "claim" {
                    &mut claim_ids
                } else {
                    &mut record_ids
                })
                .insert(id.to_owned())
            {
                return Err(SourceCommandError::Conflict(
                    "duplicate Work catalog identity",
                ));
            }
            if kind == "work" && id == owner.work_id {
                let raw_work = before
                    .get("work.json")
                    .ok_or(SourceCommandError::Conflict("Work selected record absent"))?;
                let value = cmd::parse(raw_work)?;
                if cmd::text(&entry, "source_record_ref")? != owner.work_path.as_str()
                    || cmd::text(&entry, "record_sha256")? != cmd::record_digest(&value)?.to_hex()
                {
                    return Err(SourceCommandError::Conflict("Work catalog parent stale"));
                }
            }
            if kind == "expression"
                && cmd::field(&entry, "links")?
                    .object_get("work_ref")
                    .and_then(JsonValue::as_str)
                    == Some(owner.work_id.as_str())
            {
                expressions.insert(id.to_owned(), entry.clone());
            }
            if kind == "claim"
                && entry.object_get("predicate").and_then(JsonValue::as_str)
                    == Some("has_expression")
                && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                    == Some(owner.work_id.as_str())
            {
                claims.insert(id.to_owned(), entry);
            }
        }
    }
    let binding = manifest.object_get("selected_metadata_publication");
    match (binding, publication_token) {
        (None, None) => (),
        (Some(binding), Some(token)) => {
            cmd::exact_keys(binding, &["protocol", "token", "files"])?;
            if cmd::text(binding, "protocol")? != "tos_selected_source_metadata_v1"
                || cmd::text(binding, "token")? != token
            {
                return Err(SourceCommandError::Conflict(
                    "Work catalog publication token",
                ));
            }
            let rows =
                cmd::field(binding, "files")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid(
                        "Work catalog publication file map",
                    ))?;
            if rows.len() != digests.len()
                || digests.iter().any(|(path, digest)| {
                    rows.iter()
                        .find(|(key, _)| key.as_str() == Some(path.as_str()))
                        .and_then(|(_, value)| value.as_str())
                        != Some(digest.as_str())
                })
            {
                return Err(SourceCommandError::Conflict(
                    "Work catalog publication file closure",
                ));
            }
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "Work catalog publication binding absent",
            ));
        }
    }
    digests.insert(
        "ToS/source-witnesses/catalog/catalog.manifest.json".to_owned(),
        raw_hex(&manifest_raw),
    );
    if !record_ids.contains(&owner.work_id)
        || record_ids.contains(&owner.expression_id)
        || record_ids.contains(&owner.claim_id)
        || claim_ids.contains(&owner.expression_id)
        || claim_ids.contains(&owner.claim_id)
        || claims.values().any(|entry| {
            entry
                .object_get("provenance_event_ref")
                .and_then(JsonValue::as_str)
                == Some(owner.event_id.as_str())
        })
    {
        return Err(SourceCommandError::Conflict(
            "Work catalog current or absent identity",
        ));
    }
    let work = cmd::parse(
        before
            .get("work.json")
            .ok_or(SourceCommandError::Conflict("Work selected record absent"))?,
    )?;
    let refs = cmd::array(&work, "expression_claim_refs")?;
    let reference_ids = refs
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid("Work expression Claim ref"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if reference_ids.len() != refs.len()
        || reference_ids != claims.keys().cloned().collect()
        || expressions.len() != claims.len()
    {
        return Err(SourceCommandError::Conflict(
            "Work topology forward/backlink closure",
        ));
    }
    let mut prior_claims = BTreeMap::new();
    let mut retained_transactions = BTreeMap::new();
    let mut expression_targets = BTreeSet::new();
    for (id, entry) in &claims {
        let expression_id = cmd::text(entry, "object")?;
        let expression = expressions
            .get(expression_id)
            .ok_or(SourceCommandError::Conflict(
                "Work topology Expression missing",
            ))?;
        let raw_expression = checked_current_source(
            fs,
            cut,
            cmd::text(expression, "source_record_ref")?,
            2_097_152,
            deadline,
            cancelled,
        )?;
        let value = cmd::parse(&raw_expression)?;
        if cmd::text(&value, "record_id")? != expression_id
            || cmd::text(&value, "work_ref")? != owner.work_id
            || cmd::record_digest(&value)?.to_hex() != cmd::text(expression, "record_sha256")?
            || !expression_targets.insert(expression_id.to_owned())
        {
            return Err(SourceCommandError::Conflict(
                "Work topology Expression stale",
            ));
        }
        digests.insert(
            cmd::text(expression, "source_record_ref")?.to_owned(),
            raw_hex(&raw_expression),
        );
        let claim_ref = cmd::text(entry, "source_claim_file_ref")?;
        let raw_claims =
            checked_current_source(fs, cut, claim_ref, 8_388_608, deadline, cancelled)?;
        let line_number = usize::try_from(cmd::integer(entry, "source_claim_line")?)
            .map_err(|_| SourceCommandError::Invalid("Work topology Claim line"))?;
        let line = catalog_lines(&raw_claims)
            .nth(
                line_number
                    .checked_sub(1)
                    .ok_or(SourceCommandError::Invalid("Work topology Claim line"))?,
            )
            .ok_or(SourceCommandError::Conflict(
                "Work topology Claim line absent",
            ))?;
        let claim = cmd::parse(line)?;
        if cmd::text(&claim, "claim_id")? != id
            || cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")?
            || cmd::text(&claim, "subject_ref")? != owner.work_id
            || cmd::text(&claim, "object")? != expression_id
        {
            return Err(SourceCommandError::Conflict("Work topology Claim stale"));
        }
        digests.insert(claim_ref.to_owned(), raw_hex(&raw_claims));
        if claim_ref.ends_with("/source-claims.jsonl") {
            let claim_home = claim_ref
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("prior Work Claim home"))?
                .0;
            let receipt_ref = format!("{claim_home}/work-expression-receipt.json");
            let receipt_raw =
                checked_current_source(fs, cut, &receipt_ref, 2_097_152, deadline, cancelled)?;
            let receipt = cmd::parse(&receipt_raw)?;
            let transaction_id = cmd::text(&receipt, "transaction_id")?;
            let verified = tos_validation::native_compound::verify_work_expression_from_cut(
                cut,
                worker,
                claim_ref,
                &serde_value(&claim)?,
                limits,
                cancelled,
            )
            .map_err(item_error)?;
            let (manifest_sha256, plan, _, _) =
                work_transaction::inspect_committed(fs, transaction_id, deadline, cancelled)?;
            if verified.transport
                != tos_validation::native_compound::NativeTransportState::Committed
                || verified.transaction_id != transaction_id
                || verified.manifest_sha256 != manifest_sha256
                || cmd::text(&receipt, "schema_version")? != "tos_work_expression_receipt_v1"
                || cmd::text(&receipt, "operation")? != OPERATION
                || cmd::text(cmd::field(&receipt, "claim")?, "id")? != id
                || cmd::text(&plan.authorization, "schema_version")? != AUTHORIZATION
                || cmd::text(cmd::field(&plan.authorization, "scope")?, "work_id")? != owner.work_id
                || cmd::text(cmd::field(&plan.authorization, "scope")?, "expression_id")?
                    != expression_id
                || cmd::text(cmd::field(&plan.authorization, "scope")?, "claim_id")? != id
                || !plan.files.iter().any(|selected| {
                    selected.path.as_str() == claim_ref
                        && selected.after.as_deref() == Some(raw_claims.as_slice())
                })
                || !plan.files.iter().any(|selected| {
                    selected.path.as_str() == receipt_ref
                        && selected.after.as_deref() == Some(receipt_raw.as_slice())
                })
            {
                return Err(SourceCommandError::Conflict(
                    "prior Work Claim lacks exact committed transaction",
                ));
            }
            let history_raw =
                before
                    .get("source-revision-history.json")
                    .ok_or(SourceCommandError::Conflict(
                        "prior Work selected lineage absent",
                    ))?;
            let history = cmd::parse(history_raw)?;
            if !cmd::array(&history, "receipts")?.iter().any(|entry| {
                entry
                    .object_get("publication")
                    .and_then(|publication| publication.object_get("transaction_id"))
                    .and_then(JsonValue::as_str)
                    == Some(transaction_id)
            }) {
                return Err(SourceCommandError::Conflict(
                    "prior Work transaction absent from current parent lineage",
                ));
            }
            if retained_transactions
                .insert(transaction_id.to_owned(), manifest_sha256)
                .is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "duplicate prior Work transaction",
                ));
            }
        } else {
            if cmd::text(&claim, "claim_type")? != "bibliographic"
                || cmd::text(&claim, "assertion_layer")? != "bibliographic_assertion"
                || cmd::text(&claim, "provenance_event_ref")?
                    != "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
            {
                return Err(SourceCommandError::Denied(
                    "legacy Work Claim lacks declared evidence route",
                ));
            }
            let legacy_ref = "ToS/source-witnesses/relations/provenance.jsonl";
            let raw = checked_current_source(fs, cut, legacy_ref, 8_388_608, deadline, cancelled)?;
            let mut rows =
                catalog_lines(&raw).filter(|line| line.iter().any(|b| !b.is_ascii_whitespace()));
            let only = rows
                .next()
                .ok_or(SourceCommandError::Conflict("legacy Work batch absent"))?;
            if rows.next().is_some() {
                return Err(SourceCommandError::Conflict(
                    "legacy Work batch has multiple rows",
                ));
            }
            let batch = cmd::parse(only)?;
            if !cmd::array(&batch, "outputs")?.iter().any(|output| {
                output.object_get("ref").and_then(JsonValue::as_str) == Some(claim_ref)
                    && output.object_get("sha256").and_then(JsonValue::as_str)
                        == digests.get(claim_ref).map(String::as_str)
            }) {
                return Err(SourceCommandError::Conflict(
                    "legacy Work Claim retained batch differs",
                ));
            }
            digests.insert(legacy_ref.to_owned(), raw_hex(&raw));
        }
        prior_claims.insert(id.clone(), claim);
    }
    if expression_targets != expressions.keys().cloned().collect() {
        return Err(SourceCommandError::Conflict(
            "Work topology unbound Expression",
        ));
    }
    check_publication()?;
    Ok(WorkCatalog {
        digests,
        prior_claims,
        retained_transactions,
    })
}

#[derive(Clone)]
pub(super) struct SelectedSides {
    before: Option<(Digest256, u64)>,
    after: Option<(Digest256, u64)>,
}
pub(super) fn selected_sides(
    plan: &work_transaction::WorkPlan,
) -> SourceCommandResult<BTreeMap<String, SelectedSides>> {
    let mut rows = BTreeMap::new();
    for file in &plan.files {
        let sides = SelectedSides {
            before: file
                .before
                .as_ref()
                .map(|raw| (Digest256::of_bytes(raw), raw.len() as u64)),
            after: file
                .after
                .as_ref()
                .map(|raw| (Digest256::of_bytes(raw), raw.len() as u64)),
        };
        if rows.insert(file.path.as_str().to_owned(), sides).is_some() {
            return Err(SourceCommandError::Invalid(
                "Work duplicate selected physical path",
            ));
        }
    }
    Ok(rows)
}

pub(super) fn same_selected_plan(
    expected: &work_transaction::WorkPlan,
    retained: &work_transaction::WorkPlan,
) -> SourceCommandResult<bool> {
    let mut left = expected.files.iter().collect::<Vec<_>>();
    let mut right = retained.files.iter().collect::<Vec<_>>();
    left.sort_by_key(|file| file.path.as_str());
    right.sort_by_key(|file| file.path.as_str());
    Ok(expected.transaction_id == retained.transaction_id
        && cmd::same(&expected.authorization, &retained.authorization)?
        && expected.item_path_profile == retained.item_path_profile
        && expected.new_directories == retained.new_directories
        && left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.path == b.path && a.before == b.before && a.after == b.after))
}

pub(super) fn after_cut_budget(
    cut: &CorpusCutReader,
    selected: &BTreeMap<String, SelectedSides>,
) -> SourceCommandResult<()> {
    let mut count = cut.current().members().count();
    let mut bytes = cut
        .current()
        .members()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .ok_or(SourceCommandError::Invalid(
            "Work cut byte aggregate overflow",
        ))?;
    for (path, sides) in selected {
        let original = cut.current().member(&relative(path)?);
        match (original, sides.before) {
            (Some(member), Some((sha, size)))
                if member.sha256 == sha && member.size_bytes == size => {}
            (None, None) => {}
            _ => {
                return Err(SourceCommandError::Conflict(
                    "Work selected cut member differs",
                ));
            }
        }
        if sides.before.is_none() {
            count = count.checked_add(1).ok_or(SourceCommandError::Invalid(
                "Work projected member count overflow",
            ))?;
        } else {
            bytes =
                bytes
                    .checked_sub(sides.before.unwrap().1)
                    .ok_or(SourceCommandError::Invalid(
                        "Work projected byte subtraction",
                    ))?;
        }
        if let Some((_, size)) = sides.after {
            bytes = bytes.checked_add(size).ok_or(SourceCommandError::Invalid(
                "Work projected byte aggregate overflow",
            ))?;
        } else {
            count = count.checked_sub(1).ok_or(SourceCommandError::Invalid(
                "Work projected member count subtraction",
            ))?;
        }
    }
    if count > MAX_FILES || bytes > MAX_BYTES as u64 {
        return Err(SourceCommandError::Invalid(
            "Work projected complete cut exceeds existing finite budget",
        ));
    }
    Ok(())
}

pub(super) enum WorkControlRead<'a> {
    Ready(&'a PublicationSnapshot),
    Pending(&'a JsonValue),
}

impl WorkControlRead<'_> {
    fn verify(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        match self {
            Self::Ready(snapshot) => snapshot.verify_current(fs, deadline, cancelled),
            Self::Pending(state) => work_transaction::still_pending(fs, state, deadline, cancelled),
        }
    }
}

/// Current validator reads stay physical at each mover edge. A preview also
/// contains ExactPath observations of its not-yet-published output bytes;
/// those must match the prepared buffers rather than the predecessor cut.
/// The original CONTROL read stays cut-bound while the live ready/pending
/// state is checked through the protected publication owner.
pub(super) fn selected_reads_current(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    reads: &[PredicateRead],
    selected: &BTreeMap<String, SelectedSides>,
    projected: &[(String, &[u8])],
    control: WorkControlRead<'_>,
    limit: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut total = 0u64;
    for read in reads {
        let PredicateRead::ExactPath { path, digest } = read else {
            continue;
        };
        if selected.contains_key(path) {
            continue;
        }
        if projected.iter().any(|(candidate_path, raw)| {
            candidate_path == path && Digest256::of_bytes(raw).to_prefixed() == *digest
        }) {
            continue;
        }
        if path == "ToS/source-witnesses/.metadata-publication.json" {
            let reference = relative(path)?;
            let member = cut
                .current()
                .member(&reference)
                .ok_or(SourceCommandError::Conflict(
                    "Work original publication absent from cut",
                ))?;
            if member.sha256.to_prefixed() != *digest {
                return Err(SourceCommandError::Conflict(
                    "Work original publication read differs from selected cut",
                ));
            }
            if let WorkControlRead::Ready(snapshot) = &control {
                if snapshot.member_binding()? != Some((member.sha256, member.size_bytes)) {
                    return Err(SourceCommandError::Conflict(
                        "Work ready publication differs from selected cut",
                    ));
                }
            }
            continue;
        }
        let retained = path.starts_with("ToS/source-witnesses/.record-revisions/")
            || path.starts_with("ToS/source-witnesses/.metadata-transactions/");
        if !retained && !tos_source_store::is_authored_source_path_v1(path) {
            return Err(SourceCommandError::Unsupported(
                "Work selected read lies outside authored or retained owner paths",
            ));
        }
        let reference = relative(path)?;
        let member = cut
            .current()
            .member(&reference)
            .ok_or(SourceCommandError::Conflict(
                "Work selected read absent from original cut",
            ))?;
        let (parent_ref, name) = reference
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work retained read parent"))?;
        let parent = walk(&fs.root, parent_ref, fs.uid)?;
        let (raw, mode) =
            work_transaction::read_at_mode(&parent, name, fs.uid, 8_388_608, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict(
                    "Work selected read disappeared",
                ))?;
        total = total
            .checked_add(raw.len() as u64)
            .ok_or(SourceCommandError::Invalid(
                "Work retained read aggregate overflow",
            ))?;
        let actual_digest = Digest256::of_bytes(&raw);
        if total > limit
            || !member_mode_matches(mode, member.mode, true)
            || raw.len() as u64 != member.size_bytes
            || actual_digest != member.sha256
            || actual_digest.to_prefixed() != *digest
        {
            return Err(SourceCommandError::Conflict(
                "Work selected read changed before publication",
            ));
        }
    }
    control.verify(fs, deadline, cancelled)
}

fn selected_current(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    selected: &BTreeMap<String, SelectedSides>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for (path, sides) in selected {
        active(deadline, cancelled)?;
        let (parent_ref, name) = path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work selected member parent"))?;
        let current = match work_transaction::read_existing_parent(fs, parent_ref)? {
            Some(parent) => work_transaction::read_at_mode(
                &parent, name, fs.uid, 8_388_608, deadline, cancelled,
            )?,
            None => None,
        };
        let observed = current
            .as_ref()
            .map(|(raw, _)| (Digest256::of_bytes(raw), raw.len() as u64));
        let declared = cut
            .current()
            .member(&relative(path)?)
            .map(|member| member.mode)
            .unwrap_or(0o644);
        if observed != sides.before && observed != sides.after
            || current
                .as_ref()
                .is_some_and(|(_, mode)| !member_mode_matches(*mode, declared, true))
        {
            return Err(SourceCommandError::Conflict(
                "Work selected physical third state",
            ));
        }
    }
    Ok(())
}

pub(super) fn software_current(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for input in ctx
        .files
        .iter()
        .filter(|input| !input.path.as_str().starts_with("ToS/"))
    {
        active(deadline, cancelled)?;
        let (parent_ref, name) = input
            .path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work software parent"))?;
        let parent = walk(&fs.root, parent_ref, fs.uid)?;
        let current =
            work_transaction::read_at(&parent, name, fs.uid, 8_388_608, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict("Work software member absent"))?;
        if current != input.raw {
            return Err(SourceCommandError::Conflict("Work software member changed"));
        }
    }
    Ok(())
}

/// Recheck the selected Work bytes and consulted dependencies on every mover
/// edge. A complete authored traversal additionally runs before pending and
/// after selected durability, before the ready publication. The generated
/// catalog and software retain separate exact byte checks.
pub(super) fn physical_current(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    selected: &BTreeMap<String, SelectedSides>,
    bindings: &JsonValue,
    snapshot: Option<&PublicationSnapshot>,
    archive: &work_transaction::WorkArchive,
    prior: Option<(&str, &str)>,
    guard: &work_transaction::WorkGuard<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    fs.current_context(ctx, deadline, cancelled)?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "Work physical guard cut changed",
        ));
    }
    selected_current(fs, cut, selected, deadline, cancelled)?;
    archive.verify_current(fs, deadline, cancelled)?;
    let prior_completion = if guard.prior_completion_ready {
        prior
            .map(|(id, token)| prior_completion_current(fs, id, token, deadline, cancelled))
            .transpose()?
    } else {
        None
    };
    if guard.pending_state.is_none() {
        snapshot
            .ok_or(SourceCommandError::Invalid("Work ready snapshot absent"))?
            .verify_current(fs, deadline, cancelled)?;
    }
    if guard.full_membership {
        let control = "ToS/source-witnesses/.metadata-publication.json".to_owned();
        let mut auxiliary = archive.member_paths().collect::<BTreeSet<_>>();
        auxiliary.extend(guard.journal_members.iter().cloned());
        auxiliary.insert(control.clone());
        if let Some(path) = prior_completion {
            auxiliary.insert(path);
        }
        let tos = walk(&fs.root, "ToS", fs.uid)?;
        let mut observed = BTreeMap::new();
        let mut total = 0usize;
        let mut directories = 0usize;
        scan(
            &tos,
            "ToS",
            fs.uid,
            None,
            None,
            Some(&auxiliary),
            &mut observed,
            &mut total,
            &mut directories,
            deadline,
            cancelled,
        )?;
        let mut base = BTreeMap::new();
        for member in cut.current().members() {
            if !member.path.as_str().starts_with("ToS/") {
                return Err(SourceCommandError::Denied("Work authored cut namespace"));
            }
            base.insert(member.path.as_str().to_owned(), member);
        }
        if guard.pending_state.is_none() {
            let baseline = base
                .get(&control)
                .map(|member| (member.sha256, member.size_bytes));
            if baseline
                != snapshot
                    .ok_or(SourceCommandError::Invalid("Work ready snapshot absent"))?
                    .member_binding()?
            {
                return Err(SourceCommandError::Conflict(
                    "Work publication baseline differs from selected cut",
                ));
            }
        }
        let mut auxiliary_count = 0usize;
        for path in &auxiliary {
            let (parent_ref, name) = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Work auxiliary parent"))?;
            let parent = walk(&fs.root, parent_ref, fs.uid)?;
            let current = work_transaction::read_at_mode(
                &parent, name, fs.uid, 8_388_608, deadline, cancelled,
            )?;
            match current {
                Some((raw, mode)) => {
                    auxiliary_count = auxiliary_count
                        .checked_add(1)
                        .ok_or(SourceCommandError::Invalid("Work auxiliary count overflow"))?;
                    total = total
                        .checked_add(raw.len())
                        .ok_or(SourceCommandError::Invalid("Work auxiliary byte overflow"))?;
                    if observed
                        .len()
                        .checked_add(auxiliary_count)
                        .is_none_or(|count| count > MAX_FILES)
                        || total > MAX_BYTES
                    {
                        return Err(SourceCommandError::Invalid(
                            "Work current complete cut budget",
                        ));
                    }
                    if !matches!(mode, 0o600 | 0o644) {
                        return Err(SourceCommandError::Denied(
                            "Work auxiliary physical mode unsafe",
                        ));
                    }
                    if let Some(member) = base.get(path) {
                        if !member_mode_matches(mode, member.mode, true)
                            || (path != &control || guard.pending_state.is_none())
                                && (Digest256::of_bytes(&raw) != member.sha256
                                    || raw.len() as u64 != member.size_bytes)
                        {
                            return Err(SourceCommandError::Conflict(
                                "Work auxiliary original cut binding changed",
                            ));
                        }
                    }
                }
                None if path == &control
                    && guard.pending_state.is_none()
                    && !base.contains_key(path) => {}
                None => {
                    return Err(SourceCommandError::Conflict(
                        "Work auxiliary member disappeared",
                    ));
                }
            }
        }
        if observed
            .len()
            .checked_add(auxiliary_count)
            .is_none_or(|count| count > MAX_FILES)
            || total > MAX_BYTES
        {
            return Err(SourceCommandError::Invalid(
                "Work current complete cut budget",
            ));
        }
        for path in &auxiliary {
            base.remove(path);
        }
        for (path, sides) in selected {
            match (base.get(path), &sides.before) {
                (Some(member), Some((sha, size)))
                    if member.sha256 == *sha && member.size_bytes == *size => {}
                (None, None) => {}
                _ => {
                    return Err(SourceCommandError::Conflict(
                        "Work selected original membership differs from cut",
                    ));
                }
            }
        }
        for (path, (sha, size, mode)) in &observed {
            if let Some(file) = selected.get(path.as_str()) {
                let current = (sha.to_owned(), *size);
                let declared = base.get(path).map(|member| member.mode).unwrap_or(0o644);
                if (Some(current) != file.before && Some(current) != file.after)
                    || !member_mode_matches(*mode, declared, true)
                {
                    return Err(SourceCommandError::Conflict(
                        "Work selected physical third state",
                    ));
                }
            } else {
                let member = base.remove(path).ok_or(SourceCommandError::Conflict(
                    "unselected authored source appeared",
                ))?;
                if member.sha256 != *sha
                    || member.size_bytes != *size
                    || !member_mode_matches(*mode, member.mode, true)
                {
                    return Err(SourceCommandError::Conflict(
                        "unselected authored source differs from original cut",
                    ));
                }
            }
        }
        for (path, member) in base {
            if let Some(file) = selected.get(path.as_str()) {
                if file.before.is_none()
                    || member.sha256 != file.before.as_ref().unwrap().0
                    || member.size_bytes != file.before.as_ref().unwrap().1
                {
                    return Err(SourceCommandError::Conflict(
                        "Work selected original cut mismatch",
                    ));
                }
            } else {
                return Err(SourceCommandError::Conflict(
                    "unselected authored source disappeared",
                ));
            }
        }
    }
    // Software is independently selected from source membership. The same
    // protected root carries fixture copies; current physical bytes still
    // have to equal each authenticated software capture at every guard edge.
    software_current(fs, ctx, deadline, cancelled)?;
    work_dependencies_current(fs, bindings, deadline, cancelled)?;
    Ok(())
}

/// Exact current catalog/grammar/software-source and retained transaction
/// dependencies, shared by provisional preparation and every publication edge.
pub(super) fn work_dependencies_current(
    fs: &CreationFilesystem,
    bindings: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut binding_count = 0usize;
    let mut binding_bytes = 0usize;
    for group in ["catalog_and_sources", "contracts", "implementation"] {
        let values = cmd::field(bindings, group)?
            .as_object()
            .ok_or(SourceCommandError::Invalid("Work dependency group"))?;
        for (reference, digest) in values {
            active(deadline, cancelled)?;
            binding_count += 1;
            if binding_count > 512 {
                return Err(SourceCommandError::Invalid("Work dependency count budget"));
            }
            let reference = reference
                .as_str()
                .ok_or(SourceCommandError::Invalid("Work dependency path"))?;
            relative(reference)?;
            let expected = digest
                .as_str()
                .ok_or(SourceCommandError::Invalid("Work dependency digest"))?;
            if expected.len() != 64
                || !expected
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            {
                return Err(SourceCommandError::Invalid("Work dependency digest shape"));
            }
            let (parent_ref, name) = reference
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Work dependency parent"))?;
            let parent = walk(&fs.root, parent_ref, fs.uid)?;
            let cap = if group == "catalog_and_sources" {
                16_777_216
            } else {
                2_097_152
            };
            let raw = work_transaction::read_at(&parent, name, fs.uid, cap, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict("Work dependency absent"))?;
            binding_bytes =
                binding_bytes
                    .checked_add(raw.len())
                    .ok_or(SourceCommandError::Invalid(
                        "Work dependency aggregate overflow",
                    ))?;
            if binding_bytes > 33_554_432 || raw_hex(&raw) != expected {
                return Err(SourceCommandError::Conflict(
                    "Work dependency bytes changed",
                ));
            }
        }
    }
    let retained = cmd::field(bindings, "retained_transactions")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "Work retained dependency group",
        ))?;
    if retained.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "Work retained dependency count",
        ));
    }
    for (id, digest) in retained {
        active(deadline, cancelled)?;
        let id = id
            .as_str()
            .ok_or(SourceCommandError::Invalid("Work retained identity"))?;
        let expected = digest
            .as_str()
            .ok_or(SourceCommandError::Invalid("Work retained digest"))?;
        let (actual, _, _, _) = work_transaction::inspect_committed(fs, id, deadline, cancelled)?;
        if actual != expected {
            return Err(SourceCommandError::Conflict(
                "Work prior transaction changed",
            ));
        }
    }
    Ok(())
}

/// Provisional Work preparation. This derives one request from current source
/// and the shared byte recipe, but never stages a journal or grants publication.
pub fn prepare_isolated_work_expression_from_proposal(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkExpressionPreparation> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let mut owner = WorkOwner::select_proposal(fs, ctx, limits.deadline, cancelled)?;
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
        limits.deadline,
        cancelled,
    )?;
    owner.new_directories(fs, limits.deadline, cancelled)?;
    let work = cmd::parse(before.get("work.json").ok_or(SourceCommandError::Conflict(
        "Work preview selected record absent",
    ))?)?;
    let mut refs = cmd::array(&work, "expression_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "Work preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let claim = cmd::field(&owner.request, "claim")?;
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_work_expression_preview_bytes(
        cut,
        worker,
        &scope,
        &claim_raw,
        &before,
        &ctx.recorded_at,
        limits,
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
                    cmd::object(vec![("expression_claim_refs", JsonValue::Array(refs))]),
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
                ItemRefusal::Source("Work preview authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "Work preview full request absent",
    ))?;
    let authorization = foundation_value(core.authorization())?;
    let outputs = core.outputs().map_err(item_error)?;
    selected_reads_current(
        fs,
        cut,
        core.reads(),
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
    Ok(WorkExpressionPreparation {
        request,
        projected_outputs,
    })
}

struct PreparedWorkApplication {
    plan: work_transaction::WorkPlan,
    guard: WorkApplicationGuard,
    receipt: JsonValue,
}

pub(super) struct WorkApplicationGuard {
    pub(super) snapshot: PublicationSnapshot,
    pub(super) prior_id: Option<String>,
    pub(super) selected: BTreeMap<String, SelectedSides>,
    pub(super) read_observations: Vec<PredicateRead>,
    pub(super) stored_archive: work_transaction::WorkArchive,
    pub(super) authorization: JsonValue,
}

impl WorkApplicationGuard {
    pub(super) fn check(
        &self,
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        cut: &CorpusCutReader,
        summary: &JsonValue,
        extent: work_transaction::WorkGuard<'_>,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if !cmd::same(cmd::field(summary, "authorization")?, &self.authorization)? {
            return Err(SourceCommandError::Conflict(
                "Work retained authorization differs",
            ));
        }
        physical_current(
            fs,
            ctx,
            cut,
            &self.selected,
            cmd::field(&self.authorization, "dependency_bindings")?,
            Some(&self.snapshot),
            &self.stored_archive,
            self.prior_id.as_deref().zip(self.snapshot.token.as_deref()),
            &extent,
            limits.deadline,
            cancelled,
        )?;
        let control = match extent.pending_state {
            Some(state) => WorkControlRead::Pending(state),
            None => WorkControlRead::Ready(&self.snapshot),
        };
        selected_reads_current(
            fs,
            cut,
            &self.read_observations,
            &self.selected,
            &[],
            control,
            limits.max_total_bytes,
            limits.deadline,
            cancelled,
        )
    }
}

/// Preparation and native capture happen outside the corpus lock. The
/// retained plan is built only after the one schema operation reached FINAL.
fn prepare_work_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedWorkApplication> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let prior_id =
        original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let owner = WorkOwner::select(fs, ctx, limits.deadline, cancelled)?;
    let requested_publication = cmd::field(&owner.request, "expected_publication")?;
    if requested_publication.as_str() != snapshot.token.as_deref()
        && !(requested_publication == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "Work requested publication snapshot changed",
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
        limits.deadline,
        cancelled,
    )?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_work_expression_bytes(
        cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &ctx.recorded_at,
        limits,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Work current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Work owner authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "Work transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_work_expression(
        &owner.request,
        &owner.event_id,
        owner.expression_path.as_str().rsplit_once('/').unwrap().0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_work_expression_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Work finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let read_observations = finished.reads;
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
            .get("work.json")
            .ok_or(SourceCommandError::Conflict("Work parent absent"))?,
    )?;
    let stored_archive = work_transaction::work_archive(
        fs,
        owner.work_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        true,
    )?;
    if stored_archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Work retained archive locator differs from recipe",
        ));
    }
    let authorization = plan.authorization.clone();
    Ok(PreparedWorkApplication {
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

fn apply_work_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    prepared: PreparedWorkApplication,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkExpressionPublication> {
    let PreparedWorkApplication {
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
            "Work selected publication rolled back",
        ));
    }
    Ok(WorkExpressionPublication {
        transaction_id: applied.transaction_id,
        manifest_sha256: applied.manifest_sha256,
        publication: applied.publication,
        receipt,
        replayed: false,
    })
}

/// The only Work creation writer. The journal consumes the owner-built plan
/// and the same physical-current guard after the worker has reached FINAL.
pub fn execute_isolated_work_expression_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkExpressionPublication> {
    let prepared = prepare_work_application(
        fs, ctx, cut, software, components, worker, limits, cancelled,
    )?;
    apply_work_application(fs, ctx, cut, prepared, limits, cancelled)
}

#[cfg(test)]
#[path = "source_work_expression_tests.rs"]
mod tests;

/// Observe one already committed native Work creation after a cold reopen.
/// The original selected cut authenticates the request and predecessor; the
/// current complete cut and native compound reader own later sibling/correction
/// lineage. This issues no renewed write, source, rights or canon authority.
pub fn replay_isolated_work_expression_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkExpressionPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner = WorkOwner::select(fs, original_ctx, limits.deadline, cancelled)?;
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
            "Work original replay basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Work original revision differs",
        ));
    }
    let work_home = owner.work_path.as_str().rsplit_once('/').unwrap().0;
    let expression_home = owner.expression_path.as_str().rsplit_once('/').unwrap().0;
    let mut parent = BTreeMap::new();
    let mut child = BTreeMap::new();
    for file in &retained.files {
        let path = file.path.as_str();
        let (home, name) = path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work retained selected path"))?;
        let after = file.after.as_ref().ok_or(SourceCommandError::Conflict(
            "Work retained selected output absent",
        ))?;
        if home == work_home {
            parent.insert(name.to_owned(), after.clone());
        } else if home == expression_home {
            child.insert(name.to_owned(), after.clone());
        } else {
            return Err(SourceCommandError::Conflict(
                "Work retained selected path outside scope",
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
            "Work retained plan differs from original cut",
        ));
    }
    let receipt_path = format!("{expression_home}/work-expression-receipt.json");
    let receipt_raw = retained
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict("Work retained receipt absent"))?;
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
        .map_err(|_| SourceCommandError::Unsupported("current Work Claim custody incomplete"))?
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
                return Err(SourceCommandError::Conflict("duplicate current Work Claim"));
            }
        }
    }
    let current_claim =
        current_claim.ok_or(SourceCommandError::Conflict("current Work Claim absent"))?;
    let observation = tos_validation::native_compound::verify_work_expression_from_cut(
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
            "Work committed native lineage differs",
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
        return Err(SourceCommandError::Conflict("Work retained replay changed"));
    }
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(WorkExpressionPublication {
        transaction_id,
        manifest_sha256,
        publication: terminal,
        receipt,
        replayed: true,
    })
}

#[derive(Clone, Copy)]
pub enum WorkRecoveryDecision {
    Resume,
    Rollback,
}

/// Resolve only the head-selected pending Work transaction. The original
/// command/cut and retained native event regenerate its exact plan before the
/// owner-held mover is allowed to resume or roll back either selected side.
pub fn recover_isolated_work_expression_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: WorkRecoveryDecision,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkExpressionPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner = WorkOwner::select(fs, original_ctx, limits.deadline, cancelled)?;
    if !cmd::array(&owner.configuration, "allowed_operations")?
        .iter()
        .any(|value| value.as_str() == Some(RECOVERY))
    {
        return Err(SourceCommandError::Denied("Work recovery not delegated"));
    }
    let pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("selected Work pending transaction absent"),
    )?;
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
            "Work pending owner/request basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Work pending predecessor revision differs",
        ));
    }
    let old = cmd::parse(before.get("work.json").ok_or(SourceCommandError::Conflict(
        "Work pending predecessor absent",
    ))?)?;
    let archive = work_transaction::work_archive(
        fs,
        owner.work_path.as_str(),
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
        limits.deadline,
        cancelled,
    )?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_work_expression_bytes(
        original_cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &original_ctx.recorded_at,
        limits,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, original_ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Work pending dependencies changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Work recovery authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != transaction_id || core.archive_path() != archive.path {
        return Err(SourceCommandError::Conflict(
            "Work pending recipe identity differs",
        ));
    }
    if !cmd::same(
        &foundation_value(core.authorization())?,
        &pending.plan.authorization,
    )? {
        return Err(SourceCommandError::Conflict(
            "Work pending owner recipe differs",
        ));
    }
    let expression_home = owner.expression_path.as_str().rsplit_once('/').unwrap().0;
    let selected_after = |name: &str| -> SourceCommandResult<&[u8]> {
        let path = format!("{expression_home}/{name}");
        pending
            .plan
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
            .and_then(|file| file.after.as_deref())
            .ok_or(SourceCommandError::Conflict(
                "Work pending native capture absent",
            ))
    };
    let finished = tos_validation::native_compound::finish_work_expression_bytes(
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
            "Work pending plan differs from native recipe",
        ));
    }
    let selected = selected_sides(&pending.plan)?;
    let read_observations = finished.reads;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let held_pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("Work selected pending vanished before recovery"),
    )?;
    if !cmd::same(&held_pending.state, &pending.state)?
        || !cmd::same(&held_pending.base_publication, &pending.base_publication)?
        || !same_selected_plan(&held_pending.plan, &pending.plan)?
    {
        return Err(SourceCommandError::Conflict(
            "Work pending changed before recovery lock",
        ));
    }
    let authorization = pending.plan.authorization.clone();
    let guard = |summary: &JsonValue, extent: work_transaction::WorkGuard<'_>| {
        if !cmd::same(cmd::field(summary, "authorization")?, &authorization)? {
            return Err(SourceCommandError::Conflict(
                "Work pending authorization changed",
            ));
        }
        physical_current(
            fs,
            original_ctx,
            original_cut,
            &selected,
            cmd::field(&authorization, "dependency_bindings")?,
            None,
            &archive,
            prior_id.as_deref().zip(publication_token),
            &extent,
            limits.deadline,
            cancelled,
        )?;
        let pending_state = extent.pending_state.ok_or(SourceCommandError::Conflict(
            "Work recovery publication is not pending",
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
    let rollback = matches!(decision, WorkRecoveryDecision::Rollback);
    let renewal = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_work_expression_recovery_authorization_v1"),
        ),
        ("principal_id", cmd::string(&owner.principal)),
        ("authority_ref", cmd::string(&owner.authority)),
        (
            "owner_configuration",
            cmd::string(&owner.configuration_digest),
        ),
        ("transaction_id", cmd::string(&transaction_id)),
        (
            "decision",
            cmd::string(if rollback { "rollback" } else { "resume" }),
        ),
    ]);
    let result = fence.recover(
        &held_pending,
        rollback,
        Some(renewal),
        guard,
        limits.deadline,
        cancelled,
    )?;
    Ok(WorkExpressionPublication {
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
