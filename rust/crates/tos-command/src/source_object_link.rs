//! Exact new-only Object/Link owner; the shared kernel owns byte recipes and
//! the selected metadata journal owns durable publication and recovery.
use super::work_expression::{self as shared, digest_map, raw_hex, relative};
use super::{CreationFilesystem, active, walk, work_transaction};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::{
    PredicateRead,
    item_rules::{ItemLimits, ItemRefusal},
    source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor},
};
const CONFIG: &str = "tos_local_object_link_create_owner_v1";
const REQUEST: &str = "tos_local_object_link_command_v1";
const OPERATION: &str = "object.link.create";
const RECOVERY: &str = "object.link.recover";
const AUTHORIZATION: &str = "tos_object_link_authorization_v1";
const RECEIPT: &str = "object-link-creation-receipt.json";
const SCOPE: &[&str] = &[
    "subject_id",
    "subject_source_path",
    "subject_record_type",
    "link_id",
    "link_source_path",
    "claim_id",
    "claim_source_path",
    "predicate",
    "provenance_event_id",
    "allowed_link_form_ids",
    "allowed_claim_form_ids",
    "allowed_evidence_refs",
    "uri",
    "observation_ref",
];
const IMPLEMENTATIONS: &[&str] = &[
    "rust/crates/tos-command/src/source_object_link.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-command/src/source_read_owner.rs",
    "rust/crates/tos-command/src/source_claim_publication.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "scripts/source_record_profiles.py",
    "scripts/source_metadata_snapshot.py",
    "scripts/source_witness_bibliographic_graph_common.py",
    "scripts/source_witness_human_forms.py",
    "scripts/build_source_witness_catalog.py",
];
fn error(reason: ItemRefusal) -> SourceCommandError {
    SourceCommandError::SchemaExecution {
        path: OPERATION.into(),
        root: "selected ObjectLink mechanics".into(),
        reason,
    }
}
fn serde_value(v: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    serde_json::from_slice(&cmd::canonical(v)?)
        .map_err(|_| SourceCommandError::Invalid("ObjectLink JSON conversion"))
}
fn foundation(v: &serde_json::Value) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        &serde_json::to_vec(v)
            .map_err(|_| SourceCommandError::Invalid("ObjectLink JSON conversion"))?,
    )
}
fn configuration(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    current: bool,
    required: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue, String)> {
    if current {
        fs.current_context(ctx, limits.deadline, cancelled)?;
    }
    let config = cmd::parse(&ctx.configuration_raw)?;
    let mut keys = SCOPE.to_vec();
    keys.extend([
        "schema_version",
        "uid",
        "source_root",
        "principal_id",
        "maker_type",
        "authority_ref",
        "expires_at",
        "allowed_operations",
    ]);
    cmd::exact_keys(&config, &keys)?;
    if cmd::text(&config, "schema_version")? != CONFIG
        || cmd::integer(&config, "uid")? != u64::from(fs.uid)
        || Path::new(cmd::text(&config, "source_root")?) != fs.root_path
    {
        return Err(SourceCommandError::Denied(
            "ObjectLink owner root/account/schema",
        ));
    }
    if current {
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
    }
    let operations = cmd::array(&config, "allowed_operations")?;
    let mut unique = BTreeSet::new();
    if operations.len() > 2
        || operations.iter().any(|v| {
            v.as_str()
                .is_none_or(|v| !matches!(v, OPERATION | RECOVERY) || !unique.insert(v))
        })
        || required.is_some_and(|operation| !unique.contains(operation))
    {
        return Err(SourceCommandError::Denied("ObjectLink operations"));
    }
    for key in ["principal_id", "authority_ref"] {
        if tos_foundation::python_strip_unicode16_v1(cmd::text(&config, key)?, 1_048_576)
            .map_err(|_| SourceCommandError::Invalid("ObjectLink text budget"))?
            .is_empty()
        {
            return Err(SourceCommandError::Denied("ObjectLink principal/authority"));
        }
    }
    if !matches!(
        cmd::text(&config, "maker_type")?,
        "human" | "software" | "model"
    ) {
        return Err(SourceCommandError::Denied("ObjectLink maker"));
    }
    let scope = cmd::object(
        SCOPE
            .iter()
            .map(|key| Ok((*key, cmd::field(&config, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    tos_validation::native_compound::validate_object_link_scope(&serde_value(&scope)?)
        .map_err(error)?;
    let digest = cmd::record_digest(&config)?.to_prefixed();
    Ok((config, scope, digest))
}
struct Owner {
    frozen_authorization: Option<JsonValue>,
    config: JsonValue,
    request: JsonValue,
    scope: JsonValue,
    digest: String,
}
impl Owner {
    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        proposal: bool,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let (config, scope, digest) =
            configuration(fs, ctx, true, Some(OPERATION), limits, cancelled)?;
        let request = cmd::parse(&ctx.request_raw)?;
        let mut keys = vec![
            "schema_version",
            "operation",
            "subject",
            "link",
            "claim",
            "forms",
            "claim_forms",
            "reason",
        ];
        if !proposal {
            keys.extend([
                "command_id",
                "expected_configuration",
                "expected_dependencies",
                "expected_publication",
            ]);
        }
        cmd::exact_keys(&request, &keys)?;
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")?
                != if proposal {
                    "prepare-create"
                } else {
                    OPERATION
                }
            || tos_foundation::python_strip_unicode16_v1(cmd::text(&request, "reason")?, 1_048_576)
                .map_err(|_| SourceCommandError::Invalid("ObjectLink reason budget"))?
                .is_empty()
            || cmd::text(&request, "reason")?.chars().count() > 4096
        {
            return Err(SourceCommandError::Invalid("ObjectLink request"));
        }
        if !proposal
            && (cmd::text(&request, "expected_configuration")? != digest
                || !(1..=256).contains(&cmd::text(&request, "command_id")?.chars().count()))
        {
            return Err(SourceCommandError::Conflict(
                "ObjectLink prepared configuration/command",
            ));
        }
        Ok(Self {
            frozen_authorization: None,
            config,
            request,
            scope,
            digest,
        })
    }
    fn authorization(&self, dependencies: JsonValue) -> SourceCommandResult<JsonValue> {
        if let Some(authorization) = &self.frozen_authorization {
            if !cmd::same(
                cmd::field(authorization, "dependency_bindings")?,
                &dependencies,
            )? {
                return Err(SourceCommandError::Conflict(
                    "ObjectLink frozen dependencies differ",
                ));
            }
            return Ok(authorization.clone());
        }
        Ok(cmd::object(vec![
            ("schema_version", cmd::string(AUTHORIZATION)),
            ("scope", self.scope.clone()),
            (
                "principal_id",
                cmd::field(&self.config, "principal_id")?.clone(),
            ),
            (
                "maker_type",
                cmd::field(&self.config, "maker_type")?.clone(),
            ),
            (
                "authority_ref",
                cmd::field(&self.config, "authority_ref")?.clone(),
            ),
            ("owner_configuration", cmd::string(&self.digest)),
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
    fn home(&self, key: &str) -> SourceCommandResult<&str> {
        cmd::text(&self.scope, key)?
            .rsplit_once('/')
            .map(|v| v.0)
            .ok_or(SourceCommandError::Invalid("ObjectLink home"))
    }
    fn directories(
        &self,
        fs: &CreationFilesystem,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<RelativePath>> {
        let mut homes = BTreeSet::new();
        for key in ["link_source_path", "claim_source_path"] {
            let home = self.home(key)?;
            let (parent, name) = home
                .rsplit_once('/')
                .ok_or(SourceCommandError::Denied("ObjectLink new home"))?;
            let _fd = walk(&fs.root, parent, fs.uid)?;
            let _ = name;
            if work_transaction::read_existing_parent(fs, home)?.is_some() {
                return Err(SourceCommandError::Conflict("ObjectLink home occupied"));
            }
            homes.insert(relative(home)?);
            active(limits.deadline, cancelled)?;
        }
        Ok(homes.into_iter().collect())
    }
}
fn source_bytes(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    path: &str,
    cap: usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let relative = relative(path)?;
    if !tos_source_store::is_authored_source_path_v1(path) {
        return Err(SourceCommandError::Denied(
            "ObjectLink authored dependency path",
        ));
    }
    let member = cut
        .current()
        .member(&relative)
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink dependency absent from cut",
        ))?;
    let (parent, name) = path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink dependency parent"))?;
    let fd = walk(&fs.root, parent, fs.uid)?;
    let (raw, mode) =
        work_transaction::read_at_mode(&fd, name, fs.uid, cap, limits.deadline, cancelled)?
            .ok_or(SourceCommandError::Conflict("ObjectLink dependency absent"))?;
    if raw.len() as u64 != member.size_bytes
        || Digest256::of_bytes(&raw) != member.sha256
        || !super::member_mode_matches(mode, member.mode, true)
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink source bytes changed",
        ));
    }
    let selected = cut
        .read_member(
            cut.current().revision(),
            &relative,
            cap as u64,
            limits.deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("ObjectLink cut read failed"))?;
    if selected.raw != raw {
        return Err(SourceCommandError::Conflict(
            "ObjectLink custody bytes differ",
        ));
    }
    Ok(raw)
}
fn context(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    owner: &Owner,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    token: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let catalog = super::catalog_selection::select_catalog(fs, token, limits.deadline, cancelled)?;
    for id in [
        cmd::text(&owner.scope, "link_id")?,
        cmd::text(&owner.scope, "claim_id")?,
    ] {
        if catalog.record_ids.contains(id) || catalog.claim_ids.contains(id) {
            return Err(SourceCommandError::Conflict(
                "ObjectLink new identity occupied",
            ));
        }
    }
    let event = cmd::text(&owner.scope, "provenance_event_id")?;
    if catalog.entries.iter().any(|(_, v)| {
        v.object_get("provenance_event_ref")
            .and_then(JsonValue::as_str)
            == Some(event)
    }) {
        return Err(SourceCommandError::Conflict(
            "ObjectLink event identity occupied",
        ));
    }
    let subject_id = cmd::text(&owner.scope, "subject_id")?;
    let kind = cmd::text(&owner.scope, "subject_record_type")?;
    let entry = catalog
        .entries
        .iter()
        .find(|(k, v)| {
            k == kind && v.object_get("record_id").and_then(JsonValue::as_str) == Some(subject_id)
        })
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink catalog subject absent",
        ))?;
    let subject_path = cmd::text(&owner.scope, "subject_source_path")?;
    let subject = cmd::field(&owner.request, "subject")?;
    if cmd::text(&entry.1, "source_record_ref")? != subject_path
        || cmd::text(&entry.1, "record_sha256")? != cmd::record_digest(subject)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink subject catalog stale",
        ));
    }
    let raw = source_bytes(fs, cut, subject_path, 2_097_152, limits, cancelled)?;
    if !cmd::same(&cmd::parse(&raw)?, subject)? {
        return Err(SourceCommandError::Conflict(
            "ObjectLink exact subject changed",
        ));
    }
    let mut digests = catalog.digests;
    digests.insert(subject_path.into(), raw_hex(&raw));
    let mut selections = BTreeSet::new();
    for field in ["forms", "claim_forms"] {
        for row in cmd::array(&owner.request, field)? {
            selections.insert(cmd::text(row, "form_id")?.to_owned());
        }
    }
    let mut adjacent = BTreeSet::new();
    for (kind, entry) in &catalog.entries {
        if kind == "claim" {
            let path = cmd::text(entry, "source_claim_file_ref")?;
            if path.ends_with("/source-claims.jsonl") {
                let home = path.rsplit_once('/').unwrap().0;
                adjacent.insert(format!(
                    "{home}/source-claims.{}.human-forms.json",
                    Digest256::of_bytes(cmd::text(entry, "claim_id")?.as_bytes()).to_hex()
                ));
            }
        } else {
            let path = cmd::text(entry, "source_record_ref")?;
            if let Some(stem) = path.strip_suffix(".json") {
                adjacent.insert(format!("{stem}.human-forms.json"));
            }
        }
    }
    let mut form_bytes = 0usize;
    for path in adjacent {
        if cut.current().member(&relative(&path)?).is_none() {
            continue;
        }
        let raw = source_bytes(fs, cut, &path, 2_097_152, limits, cancelled)?;
        form_bytes = form_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid("ObjectLink form cost overflow"))?;
        if form_bytes > 16_777_216 || digests.len() >= 384 {
            return Err(SourceCommandError::Invalid(
                "ObjectLink adjacent form budget",
            ));
        }
        let value = cmd::parse(&raw)?;
        tos_validation::source_forms::source_copy_kernel::validate_history(
            &value,
            cmd::field(&value, "subject")?,
        )
        .map_err(crate::source_forms::form_error)?;
        for field in ["forms", "prior_forms"] {
            for form in cmd::array(&value, field)? {
                if selections.contains(cmd::text(form, "form_id")?) {
                    return Err(SourceCommandError::Conflict(
                        "ObjectLink form identity occupied",
                    ));
                }
            }
        }
        digests.insert(path, raw_hex(&raw));
    }
    let claim = cmd::field(&owner.request, "claim")?;
    for reference in claim
        .object_get("alternative_claim_refs")
        .map(|v| {
            v.as_array()
                .ok_or(SourceCommandError::Invalid("ObjectLink alternative claims"))
        })
        .transpose()?
        .unwrap_or(&[])
    {
        if !catalog.claim_ids.contains(
            reference
                .as_str()
                .ok_or(SourceCommandError::Invalid("ObjectLink alternative claim"))?,
        ) {
            return Err(SourceCommandError::Conflict(
                "ObjectLink alternative claim absent",
            ));
        }
    }
    let mut evidence = BTreeSet::new();
    evidence.insert(cmd::text(&owner.scope, "observation_ref")?.to_owned());
    for (record, key) in [
        (cmd::field(&owner.request, "link")?, "source_refs"),
        (claim, "evidence_refs"),
        (claim, "counterevidence_refs"),
    ] {
        if let Some(values) = record.object_get(key) {
            for v in values
                .as_array()
                .ok_or(SourceCommandError::Invalid("ObjectLink evidence refs"))?
            {
                evidence.insert(
                    v.as_str()
                        .ok_or(SourceCommandError::Invalid("ObjectLink evidence ref"))?
                        .to_owned(),
                );
            }
        }
    }
    for path in evidence {
        if path.starts_with("ToS/") {
            let raw = source_bytes(fs, cut, &path, 2_097_152, limits, cancelled)?;
            digests.insert(path, raw_hex(&raw));
        } else {
            tos_validation::native_compound::validate_object_link_address(&path).map_err(error)?;
        }
    }
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let report = tos_validation::record_rules::validate_source_claim_from_cut(
        cut, &claim_raw, worker, limits, cancelled,
    )
    .map_err(error)?;
    if !report.issues.is_empty() {
        return Err(SourceCommandError::Invalid(
            "ObjectLink local claim profile",
        ));
    }
    let mut contracts = report
        .dependency_digests
        .into_iter()
        .map(|(p, d)| (p, d.to_hex()))
        .collect::<BTreeMap<_, _>>();
    let subject_schema = if kind == "artifact" {
        match cmd::text(subject, "schema_version")? {
            "tos_artifact_source_witness_v1" => "ToS/contracts/artifact-source-witness.schema.json",
            "tos_artifact_source_witness_v2" => {
                "ToS/contracts/artifact-source-witness-v2.schema.json"
            }
            _ => return Err(SourceCommandError::Invalid("ObjectLink artifact schema")),
        }
    } else {
        "ToS/contracts/corpus-record.schema.json"
    };
    for path in [
        subject_schema,
        "ToS/contracts/source-link.schema.json",
        "ToS/contracts/object-link-claim-v2.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
    ] {
        let raw = source_bytes(fs, cut, path, 2_097_152, limits, cancelled)?;
        contracts.insert(path.into(), raw_hex(&raw));
    }
    let mut implementation = BTreeMap::new();
    for path in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "ObjectLink implementation subset incomplete",
            ))?;
        if raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid(
                "ObjectLink software byte budget",
            ));
        }
        implementation.insert((*path).into(), raw_hex(raw));
    }
    // Implementation bytes are supplied separately by the selected command context.
    Ok(cmd::object(vec![
        ("catalog_and_sources", digest_map(&digests)),
        ("contracts", digest_map(&contracts)),
        ("implementation", digest_map(&implementation)),
        ("retained_transactions", cmd::object(vec![])),
    ]))
}
pub struct ObjectLinkPreparation {
    request: JsonValue,
    projected_outputs: BTreeMap<String, Vec<u8>>,
    receipt: JsonValue,
    materializations: JsonValue,
}
impl ObjectLinkPreparation {
    pub fn request(&self) -> &JsonValue {
        &self.request
    }
    pub fn projected_outputs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.projected_outputs
    }
    pub fn receipt(&self) -> &JsonValue {
        &self.receipt
    }
    pub fn materializations(&self) -> &JsonValue {
        &self.materializations
    }
}
pub struct ObjectLinkPublication {
    transaction_id: String,
    manifest_sha256: String,
    publication: JsonValue,
    receipt: Option<JsonValue>,
    replayed: bool,
    materializations: Option<JsonValue>,
}
impl ObjectLinkPublication {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn publication(&self) -> &JsonValue {
        &self.publication
    }
    pub fn receipt(&self) -> Option<&JsonValue> {
        self.receipt.as_ref()
    }
    pub fn replayed(&self) -> bool {
        self.replayed
    }
    pub fn materializations(&self) -> Option<&JsonValue> {
        self.materializations.as_ref()
    }
}
#[derive(Clone, Copy)]
pub enum ObjectLinkRecoveryDecision {
    Resume,
    Rollback,
}
fn compose(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    owner: &Owner,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    dependencies: JsonValue,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(work_transaction::WorkPlan, JsonValue, Vec<PredicateRead>)> {
    let authorization = owner.authorization(dependencies)?;
    let mut raw = cmd::canonical(&owner.request)?;
    raw.push(b'\n');
    let mut capture_error = None;
    let prepared = tos_validation::native_compound::prepare_native_object_link_bytes(
        cut,
        worker,
        &serde_value(&owner.scope)?,
        &serde_value(&authorization)?,
        &raw,
        &ctx.recorded_at,
        limits,
        cancelled,
        |outputs| {
            let rows = outputs
                .iter()
                .map(|(path, raw)| (path.as_str(), *raw))
                .collect::<Vec<_>>();
            let result = crate::source_serialization::capture_object_link(
                &owner.request,
                cmd::text(&owner.scope, "provenance_event_id")
                    .map_err(|_| ItemRefusal::Source("ObjectLink event identity".into()))?,
                owner
                    .home("claim_source_path")
                    .map_err(|_| ItemRefusal::Source("ObjectLink claim home".into()))?,
                &rows,
                software,
                components,
                limits.deadline,
                cancelled,
            );
            result
                .map(|capture| (capture.environment_raw, capture.event_raw))
                .map_err(|e| {
                    capture_error = Some(e);
                    ItemRefusal::Source("ObjectLink process capture failed".into())
                })
        },
    );
    let prepared = prepared.map_err(|e| capture_error.unwrap_or_else(|| error(e)))?;
    let receipt = foundation(&prepared.receipt)?;
    let files = prepared
        .files
        .into_iter()
        .map(|(path, raw)| {
            Ok(work_transaction::SelectedFile {
                path: relative(&path)?,
                before: None,
                after: Some(raw),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let directories = [
        owner.home("link_source_path")?,
        owner.home("claim_source_path")?,
    ]
    .into_iter()
    .map(relative)
    .collect::<SourceCommandResult<BTreeSet<_>>>()?
    .into_iter()
    .collect();
    let plan = work_transaction::WorkPlan {
        transaction_id: prepared.transaction_id,
        authorization,
        item_path_profile: None,
        files,
        new_directories: directories,
        source_readset: None,
        source_successor: None,
    };
    let selected = shared::selected_sides(&plan)?;
    shared::after_cut_budget(cut, &selected)?;
    let _ = fs;
    Ok((plan, receipt, prepared.reads))
}
fn guard(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    plan: &work_transaction::WorkPlan,
    reads: &[PredicateRead],
    snapshot: Option<&work_transaction::PublicationSnapshot>,
    summary: &JsonValue,
    extent: work_transaction::WorkGuard<'_>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let config = cmd::parse(&ctx.configuration_raw)?;
    cmd::validate_expiry(
        cmd::text(&config, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    if !cmd::same(cmd::field(summary, "authorization")?, &plan.authorization)? {
        return Err(SourceCommandError::Conflict(
            "ObjectLink retained authorization changed",
        ));
    }
    let (_, scope, _) = configuration(fs, ctx, true, None, limits, cancelled)?;
    for key in SCOPE.iter().filter(|key| !key.starts_with("allowed_")) {
        if !cmd::same(
            cmd::field(&scope, key)?,
            cmd::field(cmd::field(&plan.authorization, "scope")?, key)?,
        )? {
            return Err(SourceCommandError::Denied(
                "ObjectLink guard exact scope changed",
            ));
        }
    }
    tos_validation::native_compound::validate_object_link_recovery_scope(
        &serde_value(&scope)?,
        &serde_value(&cmd::parse(&ctx.request_raw)?)?,
        &serde_value(&plan.authorization)?,
    )
    .map_err(error)?;
    let selected = shared::selected_sides(plan)?;
    shared::physical_current(
        fs,
        ctx,
        cut,
        &selected,
        cmd::field(&plan.authorization, "dependency_bindings")?,
        snapshot,
        None,
        None,
        &extent,
        limits.deadline,
        cancelled,
    )?;
    let control = match extent.pending_state {
        Some(state) => shared::WorkControlRead::Pending(state),
        None => shared::WorkControlRead::Ready(snapshot.ok_or(SourceCommandError::Conflict(
            "ObjectLink ready snapshot absent",
        ))?),
    };
    shared::selected_reads_current(
        fs,
        cut,
        reads,
        &selected,
        &[],
        control,
        limits.max_total_bytes,
        limits.deadline,
        cancelled,
    )
}
pub fn prepare_isolated_object_link_from_proposal(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ObjectLinkPreparation> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let mut owner = Owner::select(fs, ctx, true, limits, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    shared::complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    owner.directories(fs, limits, cancelled)?;
    cmd::set(&mut owner.request, "operation", cmd::string(OPERATION))?;
    cmd::set(
        &mut owner.request,
        "command_id",
        cmd::string("preview:uncommitted"),
    )?;
    cmd::set(
        &mut owner.request,
        "expected_configuration",
        cmd::string(&owner.digest),
    )?;
    cmd::set(
        &mut owner.request,
        "expected_publication",
        snapshot
            .token
            .as_deref()
            .map(cmd::string)
            .unwrap_or(JsonValue::Null),
    )?;
    let dependencies = context(
        fs,
        ctx,
        &owner,
        cut,
        worker,
        snapshot.token.as_deref(),
        limits,
        cancelled,
    )?;
    cmd::set(
        &mut owner.request,
        "expected_dependencies",
        cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
    )?;
    let (plan, receipt, _) = compose(
        fs,
        ctx,
        &owner,
        cut,
        software,
        components,
        worker,
        dependencies,
        limits,
        cancelled,
    )?;
    worker.finish(limits.deadline, cancelled).map_err(error)?;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    shared::work_dependencies_current(
        fs,
        cmd::field(&plan.authorization, "dependency_bindings")?,
        limits.deadline,
        cancelled,
    )?;
    Ok(ObjectLinkPreparation {
        materializations: materializations(&owner.request, &plan)?,
        request: owner.request,
        receipt,
        projected_outputs: plan
            .files
            .into_iter()
            .map(|f| (f.path.as_str().to_owned(), f.after.unwrap()))
            .collect(),
    })
}
pub fn execute_isolated_object_link_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ObjectLinkPublication> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let owner = Owner::select(fs, ctx, false, limits, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    if cmd::field(&owner.request, "expected_publication")?.as_str() != snapshot.token.as_deref() {
        return Err(SourceCommandError::Conflict(
            "ObjectLink prepared publication changed",
        ));
    }
    shared::complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    owner.directories(fs, limits, cancelled)?;
    let dependencies = context(
        fs,
        ctx,
        &owner,
        cut,
        worker,
        snapshot.token.as_deref(),
        limits,
        cancelled,
    )?;
    if cmd::record_digest(&dependencies)?.to_prefixed()
        != cmd::text(&owner.request, "expected_dependencies")?
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink prepared dependencies changed",
        ));
    }
    let (plan, receipt, reads) = compose(
        fs,
        ctx,
        &owner,
        cut,
        software,
        components,
        worker,
        dependencies,
        limits,
        cancelled,
    )?;
    worker.finish(limits.deadline, cancelled).map_err(error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let views = materializations(&owner.request, &plan)?;
    let guard_plan = plan.clone();
    let result = fence.apply(
        plan,
        &snapshot,
        |summary, extent| {
            guard(
                fs,
                ctx,
                cut,
                &guard_plan,
                &reads,
                Some(&snapshot),
                summary,
                extent,
                limits,
                cancelled,
            )
        },
        limits.deadline,
        cancelled,
    )?;
    if !result.committed {
        return Err(SourceCommandError::Conflict(
            "ObjectLink publication rolled back",
        ));
    }
    Ok(ObjectLinkPublication {
        transaction_id: result.transaction_id,
        manifest_sha256: result.manifest_sha256,
        publication: result.publication,
        receipt: Some(receipt),
        replayed: false,
        materializations: Some(views),
    })
}
fn materializations(
    request: &JsonValue,
    plan: &work_transaction::WorkPlan,
) -> SourceCommandResult<JsonValue> {
    let link = cmd::field(request, "link")?;
    let claim = cmd::field(request, "claim")?;
    let selected = |suffix: &str| -> SourceCommandResult<JsonValue> {
        let raw = plan
            .files
            .iter()
            .find(|file| file.path.as_str().ends_with(suffix))
            .and_then(|file| file.after.as_ref())
            .ok_or(SourceCommandError::Conflict(
                "ObjectLink materialization source absent",
            ))?;
        cmd::parse(raw)
    };
    let link_forms = selected("/link.human-forms.json")?;
    let claim_forms = selected(&format!(
        "/source-claims.{}.human-forms.json",
        Digest256::of_bytes(cmd::text(claim, "claim_id")?.as_bytes()).to_hex()
    ))?;
    Ok(cmd::object(vec![
        (
            "link",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                link,
                &link_forms,
            )?),
        ),
        (
            "claim",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                claim,
                &claim_forms,
            )?),
        ),
    ]))
}
fn retained_package(
    owner: &Owner,
    plan: &work_transaction::WorkPlan,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    if cmd::text(&plan.authorization, "schema_version")? != AUTHORIZATION
        || !cmd::same(cmd::field(&plan.authorization, "scope")?, &owner.scope)?
    {
        return Err(SourceCommandError::Denied("ObjectLink retained scope"));
    }
    let expected = [
        owner.home("link_source_path")?,
        owner.home("claim_source_path")?,
    ]
    .into_iter()
    .map(relative)
    .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if plan
        .new_directories
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        != expected
        || plan.new_directories.len() != expected.len()
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink exact retained new homes differ",
        ));
    }
    let mut files = BTreeMap::new();
    for file in &plan.files {
        if file.before.is_some()
            || file.after.is_none()
            || files
                .insert(file.path.as_str().to_owned(), file.after.clone().unwrap())
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ObjectLink retained new-only plan",
            ));
        }
    }
    Ok(files)
}
fn exact_retained(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    owner: &Owner,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    plan: &work_transaction::WorkPlan,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Vec<PredicateRead>)> {
    let files = retained_package(owner, plan)?;
    let home = owner.home("claim_source_path")?;
    let get = |name: &str| {
        files
            .get(&format!("{home}/{name}"))
            .ok_or(SourceCommandError::Conflict(
                "ObjectLink retained companion absent",
            ))
    };
    let receipt = cmd::parse(get(RECEIPT)?)?;
    let request = cmd::parse(get("source-create-request.json")?)?;
    if !cmd::same(&request, &owner.request)?
        || !cmd::same(
            &plan.authorization,
            &owner
                .authorization(cmd::field(&plan.authorization, "dependency_bindings")?.clone())?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink retained request/authority differs",
        ));
    }
    let prepared = tos_validation::native_compound::reconstruct_object_link_bytes(
        cut,
        worker,
        &serde_value(&owner.scope)?,
        &serde_value(&plan.authorization)?,
        get("source-create-request.json")?,
        get("source-create-environment.json")?,
        get("source-create-provenance.jsonl")?,
        cmd::text(&receipt, "recorded_at")?,
        limits,
        cancelled,
    )
    .map_err(error)?;
    if prepared.transaction_id != plan.transaction_id
        || prepared.files != files
        || !cmd::same(&foundation(&prepared.receipt)?, &receipt)?
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink retained exact recipe differs",
        ));
    }
    let _ = (fs, ctx);
    Ok((receipt, prepared.reads))
}
pub fn recover_isolated_object_link_from_captures(
    fs: &CreationFilesystem,
    renewal_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: ObjectLinkRecoveryDecision,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ObjectLinkPublication> {
    renewal_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let recovery_request = cmd::parse(&renewal_ctx.request_raw)?;
    let explicit = cmd::text(&recovery_request, "operation")? == RECOVERY;
    let (renewed, renewed_scope, renewed_digest) = configuration(
        fs,
        renewal_ctx,
        true,
        Some(if explicit { RECOVERY } else { OPERATION }),
        limits,
        cancelled,
    )?;
    if explicit {
        cmd::exact_keys(
            &recovery_request,
            &[
                "schema_version",
                "operation",
                "transaction_id",
                "decision",
                "expected_configuration",
            ],
        )?;
        if cmd::text(&recovery_request, "schema_version")? != REQUEST
            || cmd::text(&recovery_request, "expected_configuration")? != renewed_digest
            || cmd::text(&recovery_request, "decision")?
                != if matches!(decision, ObjectLinkRecoveryDecision::Rollback) {
                    "rollback"
                } else {
                    "resume"
                }
        {
            return Err(SourceCommandError::Denied(
                "ObjectLink recovery exact request",
            ));
        }
    } else if cmd::text(&recovery_request, "operation")? != OPERATION
        || !matches!(decision, ObjectLinkRecoveryDecision::Resume)
    {
        return Err(SourceCommandError::Denied(
            "ObjectLink implicit recovery only resumes original creation",
        ));
    }
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict("ObjectLink pending absent"))?;
    let authorization = &pending.plan.authorization;
    if cmd::text(authorization, "schema_version")? != AUTHORIZATION {
        return Err(SourceCommandError::Denied(
            "ObjectLink pending adapter differs",
        ));
    }
    let scope = cmd::field(authorization, "scope")?.clone();
    let home = cmd::text(&scope, "claim_source_path")?
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink pending claim home"))?
        .0;
    let request_path = format!("{home}/source-create-request.json");
    let request_raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == request_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink pending retained request absent",
        ))?;
    let request = cmd::parse(request_raw)?;
    let owner = Owner {
        config: renewed.clone(),
        scope,
        digest: cmd::text(authorization, "owner_configuration")?.into(),
        request,
        frozen_authorization: Some(authorization.clone()),
    };
    for key in SCOPE.iter().filter(|key| !key.starts_with("allowed_")) {
        if !cmd::same(
            cmd::field(&owner.scope, key)?,
            cmd::field(&renewed_scope, key)?,
        )? {
            return Err(SourceCommandError::Denied(
                "ObjectLink renewed scope differs",
            ));
        }
    }
    if cmd::field(&pending.base_publication, "token")?
        != cmd::field(&owner.request, "expected_publication")?
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink pending publication basis differs",
        ));
    }
    if !explicit
        && (!cmd::same(&recovery_request, &owner.request)?
            || cmd::text(&owner.request, "expected_configuration")? != renewed_digest)
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink implicit resume requires original request/grant",
        ));
    }
    let mut guard_ctx = renewal_ctx.clone();
    guard_ctx.request_raw = request_raw.clone();
    if explicit && cmd::text(&recovery_request, "transaction_id")? != pending.plan.transaction_id {
        return Err(SourceCommandError::Conflict(
            "ObjectLink recovery selects another transaction",
        ));
    }
    tos_validation::native_compound::validate_object_link_recovery_scope(
        &serde_value(&renewed_scope)?,
        &serde_value(&owner.request)?,
        &serde_value(&pending.plan.authorization)?,
    )
    .map_err(error)?;
    let (receipt, reads) = exact_retained(
        fs,
        &guard_ctx,
        &owner,
        original_cut,
        worker,
        &pending.plan,
        limits,
        cancelled,
    )?;
    // The original cut fixes dependencies; physical_current checks every mover
    // edge against that cut and the actual head-selected pending plan.
    worker.finish(limits.deadline, cancelled).map_err(error)?;
    let rollback = matches!(decision, ObjectLinkRecoveryDecision::Rollback);
    let renewal = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_object_link_recovery_authorization_v1"),
        ),
        (
            "principal_id",
            cmd::field(&renewed, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&renewed, "authority_ref")?.clone(),
        ),
        ("owner_configuration", cmd::string(&renewed_digest)),
        ("transaction_id", cmd::string(&pending.plan.transaction_id)),
        (
            "decision",
            cmd::string(if rollback { "rollback" } else { "resume" }),
        ),
    ]);
    let views = if rollback {
        None
    } else {
        Some(materializations(&owner.request, &pending.plan)?)
    };
    let result = fence.recover(
        &pending,
        rollback,
        if explicit { Some(renewal) } else { None },
        |summary, extent| {
            guard(
                fs,
                &guard_ctx,
                original_cut,
                &pending.plan,
                &reads,
                None,
                summary,
                extent,
                limits,
                cancelled,
            )
        },
        limits.deadline,
        cancelled,
    )?;
    Ok(ObjectLinkPublication {
        transaction_id: result.transaction_id,
        manifest_sha256: result.manifest_sha256,
        publication: result.publication,
        receipt: if rollback { None } else { Some(receipt) },
        replayed: false,
        materializations: views,
    })
}
pub fn replay_isolated_object_link_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ObjectLinkPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner = Owner::select(fs, original_ctx, false, limits, cancelled)?;
    let home = owner.home("claim_source_path")?;
    let receipt_raw = source_bytes(
        fs,
        current_cut,
        &format!("{home}/{RECEIPT}"),
        2_097_152,
        limits,
        cancelled,
    )?;
    let receipt = cmd::parse(&receipt_raw)?;
    let id = cmd::text(&receipt, "transaction_id")?;
    let (manifest, plan, _, terminal) =
        work_transaction::inspect_committed(fs, id, limits.deadline, cancelled)?;
    // Retained byte recipe is checked against the original selected cut; current
    // Claim/Link correction history is checked by the native public read kernel.
    let files = retained_package(&owner, &plan)?;
    if !cmd::same(
        &cmd::parse(
            files
                .get(&format!("{home}/source-create-request.json"))
                .ok_or(SourceCommandError::Conflict(
                    "ObjectLink replay request absent",
                ))?,
        )?,
        &owner.request,
    )? || !cmd::same(
        &plan.authorization,
        &owner.authorization(cmd::field(&plan.authorization, "dependency_bindings")?.clone())?,
    )? {
        return Err(SourceCommandError::Conflict(
            "ObjectLink replay exact request differs",
        ));
    }
    let claim_path = cmd::text(&owner.scope, "claim_source_path")?;
    let raw = source_bytes(fs, current_cut, claim_path, 2_097_152, limits, cancelled)?;
    let claim = shared::catalog_lines(&raw)
        .map(cmd::parse)
        .collect::<SourceCommandResult<Vec<_>>>()?
        .into_iter()
        .find(|v| {
            v.object_get("claim_id").and_then(JsonValue::as_str)
                == owner
                    .scope
                    .object_get("claim_id")
                    .and_then(JsonValue::as_str)
        })
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink replay claim absent",
        ))?;
    let observation = tos_validation::native_compound::verify_object_link_from_cut(
        current_cut,
        worker,
        claim_path,
        &serde_value(&claim)?,
        limits,
        cancelled,
    )
    .map_err(error)?;
    if observation.transaction_id != id
        || observation.manifest_sha256 != manifest
        || !matches!(
            observation.transport,
            tos_validation::native_compound::NativeTransportState::Committed
        )
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink replay verified transport differs",
        ));
    }
    worker.finish(limits.deadline, cancelled).map_err(error)?;
    let snapshot = work_transaction::PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    fs.current_context(original_ctx, limits.deadline, cancelled)?;
    shared::complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    shared::software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let (current_manifest, _, _, _) =
        work_transaction::inspect_committed(fs, id, limits.deadline, cancelled)?;
    if current_manifest != manifest {
        return Err(SourceCommandError::Conflict(
            "ObjectLink replay journal changed",
        ));
    }
    fence.verify(limits.deadline, cancelled)?;
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(ObjectLinkPublication {
        transaction_id: id.into(),
        manifest_sha256: manifest,
        publication: terminal,
        receipt: Some(receipt),
        replayed: true,
        materializations: None,
    })
}
/// Maintained descriptive fields from exact protected configuration and selected
/// source registries; no write grant is manufactured by the result.
pub(crate) fn current_result_fields(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let (config, scope, digest) = configuration(fs, ctx, true, None, limits, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let entity = serde_value(&cmd::parse(&source_bytes(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        2_097_152,
        limits,
        cancelled,
    )?)?)?;
    let relations = serde_value(&cmd::parse(&source_bytes(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        2_097_152,
        limits,
        cancelled,
    )?)?)?;
    let mappings = entity["types"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("ObjectLink entity registry"))?;
    let type_id = mappings
        .iter()
        .find(|v| {
            v["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims" && row["source_kind_id"] == "link"
                })
            })
        })
        .and_then(|v| v["type_id"].as_str())
        .ok_or(SourceCommandError::Invalid(
            "ObjectLink link profile mapping",
        ))?;
    let predicate = cmd::text(&scope, "predicate")?;
    let relation = relations["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("ObjectLink relation registry"))?
        .iter()
        .find(|v| {
            v["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == predicate
                })
            })
        })
        .and_then(|v| v["relation_type_id"].as_str())
        .ok_or(SourceCommandError::Invalid(
            "ObjectLink relation profile mapping",
        ))?;
    let profiles = cmd::object(vec![
        (
            "link",
            cmd::object(vec![
                ("record_type", cmd::string("link")),
                ("type_id", cmd::string(type_id)),
                ("identity_field", cmd::string("record_id")),
                (
                    "schema_ref",
                    cmd::string("ToS/contracts/source-link.schema.json"),
                ),
                ("schema_version", cmd::string("tos_source_link_v1")),
                ("source_basename", cmd::string("link.json")),
            ]),
        ),
        (
            "claim",
            cmd::object(vec![
                ("predicate", cmd::string(predicate)),
                (
                    "schema_ref",
                    cmd::string("ToS/contracts/object-link-claim-v2.schema.json"),
                ),
                ("relation_type_id", cmd::string(relation)),
                ("schema_version", cmd::string("tos_object_link_claim_v2")),
                ("source_basename", cmd::string("source-claims.jsonl")),
                ("reader", cmd::string("identity-relation-v1")),
            ]),
        ),
    ]);
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(cmd::object(vec![
        ("schema_version", cmd::string("tos_object_link_result_v1")),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&digest)),
        ("operation", cmd::string(OPERATION)),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare-create", OPERATION, RECOVERY]
                    .iter()
                    .map(|v| cmd::string(v))
                    .collect(),
            ),
        ),
        (
            "allowed_operations",
            cmd::field(&config, "allowed_operations")?.clone(),
        ),
        ("scope", scope),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("source_profiles", profiles),
        ("receipt", JsonValue::Null),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", JsonValue::Null),
        ("grants_admission", JsonValue::Bool(false)),
    ]))
}
