//! One protected Edition→Item owner. Existing transport moves selected metadata;
//! separately scoped File custody remains with the actual deposit owner.
use super::work_expression::{
    WorkApplicationGuard, WorkControlRead, after_cut_budget, allowed_forms, catalog_lines,
    checked_current_source, complete_current_cut, digest_map, known_catalog_kind,
    original_prior_publication, physical_current, raw_hex, relative, same_selected_plan,
    selected_forms, selected_reads_current, selected_sides, slug, software_current,
    work_dependencies_current,
};
use super::work_transaction::{self, PublicationSnapshot};
use super::{CreationFilesystem, active, walk};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_item_deposit as deposit;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const CONFIG: &str = "tos_local_item_adoption_owner_v1";
const REQUEST: &str = "tos_local_item_adoption_command_v1";
const OPERATION: &str = "item.adopt";
const RECOVERY: &str = "item.adoption.recover";
const AUTHORIZATION: &str = "tos_item_adoption_authorization_v1";
const SCOPE_KEYS: &[&str] = &[
    "edition_id",
    "edition_source_path",
    "item_id",
    "item_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_edition_form_ids",
    "allowed_item_form_ids",
    "allowed_claim_form_ids",
    "file_id",
    "payload_basename",
    "original_basename",
    "media_type",
    "byte_size",
    "sha256",
    "rights_id",
    "acquisition_event_id",
    "inventory_event_id",
];
const GENERATOR: &str = "2";
fn item_error(reason: ItemRefusal) -> SourceCommandError {
    SourceCommandError::SchemaExecution {
        path: OPERATION.to_owned(),
        root: "selected Edition/Item mechanics".to_owned(),
        reason,
    }
}
fn serde_value(value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    deposit::value(value)
}
fn foundation_value(value: &serde_json::Value) -> SourceCommandResult<JsonValue> {
    deposit::foundation(value)
}
fn finished_receipt(child: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        child
            .get("edition-item-receipt.json")
            .ok_or(SourceCommandError::Conflict("Item finished receipt absent"))?,
    )
}
fn validate_file_scope(config: &JsonValue) -> SourceCommandResult<()> {
    for (key, prefix) in [
        ("file_id", "tos.file."),
        ("rights_id", "tos.rights."),
        ("acquisition_event_id", "tos.event."),
        ("inventory_event_id", "tos.event."),
    ] {
        if !slug(cmd::text(config, key)?, prefix) {
            return Err(SourceCommandError::Denied(
                "Item typed File/rights/event scope",
            ));
        }
    }
    let events = [
        cmd::text(config, "provenance_event_id")?,
        cmd::text(config, "acquisition_event_id")?,
        cmd::text(config, "inventory_event_id")?,
    ];
    if events.into_iter().collect::<BTreeSet<_>>().len() != 3 {
        return Err(SourceCommandError::Denied(
            "Item copy/enumeration/serialization events overlap",
        ));
    }
    let name = cmd::text(config, "payload_basename")?;
    let original = cmd::text(config, "original_basename")?;
    let media = cmd::text(config, "media_type")?;
    let valid_mime = media.split_once('/').is_some_and(|(a, b)| {
        !a.is_empty()
            && !b.is_empty()
            && a.bytes().chain(b.bytes()).all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-')
            })
    });
    if name.is_empty()
        || name.len() > 201
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
        || original.is_empty()
        || original.chars().count() > 256
        || original.contains(['/', '\\', '\n', '\r', '\0'])
        || !valid_mime
        || Digest256::from_hex(cmd::text(config, "sha256")?).is_err()
        || !(1..=512 * 1024 * 1024).contains(&cmd::integer(config, "byte_size")?)
    {
        return Err(SourceCommandError::Denied("Item exact bounded File grant"));
    }
    Ok(())
}
fn validate_request_rights(config: &JsonValue, request: &JsonValue) -> SourceCommandResult<()> {
    let rights = cmd::field(request, "rights")?;
    let scopes = cmd::array(rights, "scope_refs")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(SourceCommandError::Denied("Item rights scope"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if cmd::text(rights, "rights_id")? != cmd::text(config, "rights_id")?
        || scopes
            != [cmd::text(config, "item_id")?, cmd::text(config, "file_id")?]
                .into_iter()
                .collect()
        || cmd::text(rights, "visibility")? != "local_only"
        || cmd::text(rights, "review_status")? != "unreviewed"
        || !matches!(
            cmd::text(rights, "assessment_status")?,
            "not_assessed" | "copyright_not_evaluated" | "copyright_undetermined"
        )
        || cmd::text(rights, "redistribution_posture")? != "not_authorized"
        || cmd::text(rights, "derivative_posture")? != "local_research_only"
        || !cmd::array(rights, "permissions")?.is_empty()
        || rights
            .object_get("layer_assessments")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        || !matches!(
            cmd::text(request, "item_kind")?,
            "born_digital" | "digitized_physical_copy" | "derived_publication" | "unknown"
        )
    {
        return Err(SourceCommandError::Denied(
            "Item supplied rights must remain unreviewed local-only",
        ));
    }
    Ok(())
}
struct ItemOwner {
    configuration: JsonValue,
    request: JsonValue,
    edition_path: RelativePath,
    item_path: RelativePath,
    edition_id: String,
    item_id: String,
    claim_id: String,
    event_id: String,
    principal: String,
    authority: String,
    maker: String,
    configuration_digest: String,
}

impl ItemOwner {
    fn scope(&self) -> SourceCommandResult<JsonValue> {
        Ok(JsonValue::Object(
            self.configuration
                .as_object()
                .ok_or(SourceCommandError::Invalid("Item scope"))?
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
        let edition_home = self
            .edition_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Item selected home"))?
            .0;
        let item_home = self
            .item_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Item selected home"))?
            .0;
        let parent_names = [
            "edition.json",
            "edition.human-forms.json",
            "source-revision-history.json",
        ];
        if parent.len() != parent_names.len()
            || parent_names.iter().any(|name| !parent.contains_key(*name))
            || before
                .keys()
                .any(|name| !parent_names.contains(&name.as_str()))
        {
            return Err(SourceCommandError::Invalid(
                "Item selected parent package shape",
            ));
        }
        let claim_forms = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(self.claim_id.as_bytes()).to_hex()
        );
        let child_names = [
            "item.json",
            "item.human-forms.json",
            "source-claims.jsonl",
            claim_forms.as_str(),
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "edition-item-receipt.json",
            "item.manifest.json",
            "rights.json",
            "resource-inventory.json",
            "fixity.sha256",
            "forensic-report.md",
            "provenance.jsonl",
            "item-deposit-receipt.json",
        ];
        if child.len() != child_names.len()
            || child_names.iter().any(|name| !child.contains_key(*name))
        {
            return Err(SourceCommandError::Invalid(
                "Item selected child package shape",
            ));
        }
        let mut files = Vec::with_capacity(parent.len() + child.len());
        for (name, after) in parent {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{edition_home}/{name}"))?,
                before: before.get(&name).cloned(),
                after: Some(after),
            });
        }
        for (name, after) in child {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{item_home}/{name}"))?,
                before: None,
                after: Some(after),
            });
        }
        Ok(work_transaction::WorkPlan {
            transaction_id: self.transaction_id()?,
            authorization,
            item_path_profile: Some(self.item_path.clone()),
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
        stage: Option<&serde_json::Value>,
    ) -> SourceCommandResult<Vec<RelativePath>> {
        active(deadline, cancelled)?;
        let child = self
            .item_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Item home"))?
            .0;
        let parent = child
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Item parent"))?
            .0;
        let mut new = Vec::new();
        if work_transaction::read_existing_parent(fs, parent)?.is_none() {
            new.push(relative(parent)?);
        }
        if let Some(home) = work_transaction::read_existing_parent(fs, child)? {
            let config = deposit::value(&self.configuration)?;
            if stage.is_none()
                || deposit::destination(&config)?
                    .parent()
                    .and_then(Path::parent)
                    != Some(fs.root_path.join(child).as_path())
            {
                return Err(SourceCommandError::Conflict(
                    "new Item metadata home is occupied",
                ));
            }
            let mut entries = std::fs::read_dir(format!(
                "/proc/self/fd/{}",
                std::os::fd::AsRawFd::as_raw_fd(&home)
            ))
            .map_err(|_| SourceCommandError::Denied("Item payload-only metadata home"))?;
            let entry = entries
                .next()
                .transpose()
                .map_err(|_| SourceCommandError::Denied("Item home entry"))?
                .ok_or(SourceCommandError::Conflict("Item payload home empty"))?;
            if entry.file_name() != "payload" || entries.next().is_some() {
                return Err(SourceCommandError::Conflict(
                    "Item payload home contains unrelated metadata",
                ));
            }
        } else {
            new.push(relative(child)?);
        }
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
        Self::select_values(fs, ctx, proposal, None, deadline, cancelled)
    }
    fn select_values(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        proposal: bool,
        retained: Option<(&JsonValue, &JsonValue)>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        fs.current_context(ctx, deadline, cancelled)?;
        let config = retained.map_or_else(
            || cmd::parse(&ctx.configuration_raw),
            |(config, _)| Ok(config.clone()),
        )?;
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
                "edition_id",
                "edition_source_path",
                "item_id",
                "item_source_path",
                "claim_id",
                "provenance_event_id",
                "allowed_edition_form_ids",
                "allowed_item_form_ids",
                "allowed_claim_form_ids",
                "file_id",
                "payload_basename",
                "original_basename",
                "media_type",
                "byte_size",
                "sha256",
                "rights_id",
                "acquisition_event_id",
                "inventory_event_id",
                "payload_root",
                "input_path",
                "payload_authority_ref",
                "payload_expires_at",
                "recovery_root",
            ],
        )?;
        if cmd::text(&config, "schema_version")? != CONFIG
            || cmd::integer(&config, "uid")? != u64::from(fs.uid)
            || Path::new(cmd::text(&config, "source_root")?) != fs.root_path
        {
            return Err(SourceCommandError::Denied(
                "Item delegation root/account/schema",
            ));
        }
        if retained.is_none() {
            cmd::validate_expiry(
                cmd::text(&config, "expires_at")?,
                &crate::source_serialization::instant()?,
            )?;
        }
        let operations = cmd::array(&config, "allowed_operations")?;
        let mut allowed = BTreeSet::new();
        if operations.is_empty() || operations.len() > 2 {
            return Err(SourceCommandError::Denied("Item delegated operation count"));
        }
        for value in operations {
            let operation = value
                .as_str()
                .ok_or(SourceCommandError::Invalid("Item operation"))?;
            if !matches!(operation, OPERATION | RECOVERY) || !allowed.insert(operation) {
                return Err(SourceCommandError::Denied("Item delegated operation"));
            }
        }
        if !allowed.contains(OPERATION) {
            return Err(SourceCommandError::Denied("Item creation not delegated"));
        }
        let edition_id = cmd::text(&config, "edition_id")?.to_owned();
        let item_id = cmd::text(&config, "item_id")?.to_owned();
        let claim_id = cmd::text(&config, "claim_id")?.to_owned();
        let event_id = cmd::text(&config, "provenance_event_id")?.to_owned();
        if !slug(&edition_id, "tos.edition.")
            || !slug(&item_id, "tos.item.")
            || !slug(&claim_id, "tos.claim.")
            || !slug(&event_id, "tos.event.")
        {
            return Err(SourceCommandError::Denied("Item typed scope identities"));
        }
        let edition_path = relative(cmd::text(&config, "edition_source_path")?)?;
        let item_path = relative(cmd::text(&config, "item_source_path")?)?;
        let work_parts = edition_path.as_str().split('/').collect::<Vec<_>>();
        let item_parent = item_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Item source parent"))?
            .0;
        let edition_parent = edition_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Item source parent"))?
            .0;
        let item_home = item_parent
            .rsplit_once('/')
            .ok_or(SourceCommandError::Denied("Item home"))?
            .1;
        if work_parts.len() < 5
            || work_parts[..2] != ["ToS", "source-witnesses"]
            || !work_parts.contains(&"editions")
            || work_parts.last() != Some(&"edition.json")
            || !item_path.as_str().ends_with("/item.json")
            || !item_parent.starts_with(&format!("{edition_parent}/items/"))
            || item_parent != format!("{edition_parent}/items/{item_home}")
            || !slug(item_home, "")
        {
            return Err(SourceCommandError::Denied("Item/Item home scope"));
        }
        let edition_forms = allowed_forms(&config, "allowed_edition_form_ids")?;
        let item_forms = allowed_forms(&config, "allowed_item_form_ids")?;
        let claim_forms = allowed_forms(&config, "allowed_claim_form_ids")?;
        if !edition_forms.is_disjoint(&item_forms)
            || !edition_forms.is_disjoint(&claim_forms)
            || !item_forms.is_disjoint(&claim_forms)
        {
            return Err(SourceCommandError::Denied("Item form identities overlap"));
        }
        let principal = cmd::text(&config, "principal_id")?.to_owned();
        let authority = cmd::text(&config, "authority_ref")?.to_owned();
        let maker = cmd::text(&config, "maker_type")?.to_owned();
        if principal.is_empty()
            || authority.is_empty()
            || !matches!(maker.as_str(), "human" | "software" | "model")
        {
            return Err(SourceCommandError::Denied("Item principal/maker/authority"));
        }
        validate_file_scope(&config)?;
        if retained.is_some() {
            deposit::validate_retained_config(&deposit::value(&config)?, fs.uid)?;
        } else {
            deposit::validate_config(&deposit::value(&config)?, fs.uid)?;
        }
        let request = retained.map_or_else(
            || cmd::parse(&ctx.request_raw),
            |(_, request)| Ok(request.clone()),
        )?;
        if proposal {
            cmd::exact_keys(
                &request,
                &[
                    "schema_version",
                    "operation",
                    "record",
                    "claim",
                    "forms",
                    "item_forms",
                    "claim_forms",
                    "reason",
                    "rights",
                    "item_kind",
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
                    "item_forms",
                    "claim_forms",
                    "reason",
                    "expected_configuration",
                    "expected_source",
                    "expected_revision",
                    "expected_dependencies",
                    "expected_publication",
                    "rights",
                    "item_kind",
                    "inventory",
                    "inventory_limitation",
                    "fixity_verified_at",
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
                "Item create request schema/operation/budget",
            ));
        }
        let reason = cmd::text(&request, "reason")?;
        if reason.is_empty() || reason.len() > 4096 || !cmd::nonblank(reason) {
            return Err(SourceCommandError::Invalid("Item command identity/reason"));
        }
        if !proposal {
            let command_id = cmd::text(&request, "command_id")?;
            if command_id.is_empty() || command_id.len() > 256 {
                return Err(SourceCommandError::Invalid("Item command identity"));
            }
            for field in [
                "expected_configuration",
                "expected_revision",
                "expected_dependencies",
            ] {
                if !super::work_transaction::is_hash(cmd::text(&request, field)?) {
                    return Err(SourceCommandError::Invalid("Item expected digest"));
                }
            }
            if cmd::field(&request, "expected_publication")? != &JsonValue::Null
                && !cmd::field(&request, "expected_publication")?
                    .as_str()
                    .is_some_and(super::work_transaction::is_hash)
            {
                return Err(SourceCommandError::Invalid(
                    "Item expected publication token",
                ));
            }
            if cmd::text(&request, "expected_configuration")?
                != cmd::record_digest(&config)?.to_prefixed()
            {
                return Err(SourceCommandError::Conflict(
                    "Item current owner digest differs",
                ));
            }
        }
        let record = cmd::field(&request, "record")?;
        let claim = cmd::field(&request, "claim")?;
        let maker_ref = cmd::object(vec![
            ("maker_type", cmd::string(&maker)),
            ("agent_ref", cmd::string(&principal)),
        ]);
        if cmd::text(record, "record_id")? != item_id
            || cmd::text(record, "embodiment_ref")? != edition_id
            || cmd::text(record, "item_manifest_ref")?
                != format!(
                    "{}/item.manifest.json",
                    item_path.as_str().rsplit_once('/').unwrap().0
                )
            || cmd::text(claim, "claim_id")? != claim_id
            || cmd::text(claim, "subject_ref")? != edition_id
            || cmd::text(claim, "object")? != item_id
            || cmd::text(claim, "provenance_event_ref")? != event_id
            || !cmd::same(cmd::field(claim, "maker")?, &maker_ref)?
        {
            return Err(SourceCommandError::Denied(
                "Item record/Claim identity scope",
            ));
        }
        validate_request_rights(&config, &request)?;
        if !proposal {
            cmd::validate_instant(cmd::text(&request, "fixity_verified_at")?)?;
            if (cmd::field(&request, "inventory")? == &JsonValue::Null)
                != (cmd::field(&request, "inventory_limitation")? != &JsonValue::Null)
            {
                return Err(SourceCommandError::Invalid(
                    "Item inventory completeness/limitation",
                ));
            }
            cmd::exact_keys(cmd::field(&request, "fields")?, &["exemplar_claim_refs"])?;
        }
        selected_forms(&request, "forms", &edition_forms)?;
        selected_forms(&request, "item_forms", &item_forms)?;
        selected_forms(&request, "claim_forms", &claim_forms)?;
        let configuration_digest = cmd::record_digest(&config)?.to_prefixed();
        Ok(Self {
            configuration: config,
            request,
            edition_path,
            item_path,
            edition_id,
            item_id,
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

/// Only selected Item package files are an input to the compound byte law;
/// descendants and generated catalog rows keep their distinct owner readers.
fn selected_before(
    fs: &CreationFilesystem,
    owner: &ItemOwner,
    cut: &CorpusCutReader,
    snapshot: &PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    snapshot.verify_current(fs, deadline, cancelled)?;
    let parent_ref = owner
        .edition_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Item parent"))?
        .0;
    let parent = walk(&fs.root, parent_ref, fs.uid)?;
    let mut before = BTreeMap::new();
    for name in [
        "edition.json",
        "edition.human-forms.json",
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
                        "physical Item selected member absent from cut",
                    ))?;
                if member.sha256 != Digest256::of_bytes(&bytes)
                    || member.size_bytes != bytes.len() as u64
                {
                    return Err(SourceCommandError::Conflict(
                        "physical Item member differs from selected cut",
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
                        SourceCommandError::Unsupported("selected Item custody read incomplete")
                    })?;
                if selected.raw != bytes {
                    return Err(SourceCommandError::Conflict(
                        "Item cut and current bytes differ",
                    ));
                }
                before.insert(name.to_owned(), bytes);
            }
            None if name == "edition.json" || cut.current().member(&path).is_some() => {
                return Err(SourceCommandError::Conflict("selected Item member missing"));
            }
            None => (),
        }
    }
    if before.values().map(Vec::len).sum::<usize>() > 8_388_608 {
        return Err(SourceCommandError::Invalid(
            "selected Item package byte budget",
        ));
    }
    snapshot.verify_current(fs, deadline, cancelled)?;
    Ok(before)
}

fn original_before_from_cut(
    owner: &ItemOwner,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent_ref = owner
        .edition_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Item original parent"))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        if cut.current().member(&path).is_none() {
            if name == "edition.json" {
                return Err(SourceCommandError::Conflict("Item original record absent"));
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
            .map_err(|_| SourceCommandError::Unsupported("Item original cut custody incomplete"))?
            .raw;
        total = total
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Item original package overflow",
            ))?;
        if total > 8_388_608 {
            return Err(SourceCommandError::Invalid("Item original package budget"));
        }
        before.insert(name.to_owned(), raw);
    }
    Ok(before)
}

pub(super) struct ItemCatalog {
    digests: BTreeMap<String, String>,
    retained_transactions: BTreeMap<String, String>,
    native_reads: Vec<tos_validation::PredicateRead>,
    native_bytes: u64,
    native_state: usize,
    source_bytes: u64,
}
fn current_catalog(
    fs: &CreationFilesystem,
    owner: &ItemOwner,
    before: &BTreeMap<String, Vec<u8>>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    publication_token: Option<&str>,
    mut check_publication: impl FnMut() -> SourceCommandResult<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemCatalog> {
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
    .ok_or(SourceCommandError::Conflict("Item catalog manifest absent"))?;
    let manifest = cmd::parse(&manifest_raw)?;
    if cmd::text(&manifest, "schema_version")? != "tos_source_witness_catalog_v3"
        || cmd::text(&manifest, "claim_file")? != "ToS/source-witnesses/catalog/claims.jsonl"
    {
        return Err(SourceCommandError::Unsupported(
            "Item catalog route/version",
        ));
    }
    let record_files = cmd::field(&manifest, "record_files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Item catalog record routes"))?;
    if record_files.is_empty() || record_files.len() > 128 {
        return Err(SourceCommandError::Invalid("Item catalog route count"));
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
                "Item catalog undeclared profile route",
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
    let mut records = BTreeMap::new();
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
                .ok_or(SourceCommandError::Conflict("Item catalog route absent"))?;
        total_bytes = total_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Item catalog aggregate overflow",
            ))?;
        if total_bytes > 16_777_216 {
            return Err(SourceCommandError::Invalid(
                "Item catalog aggregate byte budget",
            ));
        }
        digests.insert(reference.clone(), raw_hex(&raw));
        for line in catalog_lines(&raw) {
            active(deadline, cancelled)?;
            count += 1;
            if count > 8192 {
                return Err(SourceCommandError::Invalid("Item catalog row budget"));
            }
            let entry = cmd::parse(line)?;
            let id = if kind == "claim" {
                if cmd::text(&entry, "schema_version")?
                    != "tos_source_witness_claim_catalog_entry_v1"
                {
                    return Err(SourceCommandError::Invalid("Item claim catalog schema"));
                }
                cmd::text(&entry, "claim_id")?
            } else {
                if cmd::text(&entry, "schema_version")? != "tos_source_witness_catalog_entry_v1"
                    || cmd::text(&entry, "record_type")? != kind
                {
                    return Err(SourceCommandError::Invalid("Item record catalog schema"));
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
                    "duplicate Item catalog identity",
                ));
            }
            if kind == "edition" && id == owner.edition_id {
                let raw_work = before
                    .get("edition.json")
                    .ok_or(SourceCommandError::Conflict("Item selected record absent"))?;
                let value = cmd::parse(raw_work)?;
                if cmd::text(&entry, "source_record_ref")? != owner.edition_path.as_str()
                    || cmd::text(&entry, "record_sha256")? != cmd::record_digest(&value)?.to_hex()
                {
                    return Err(SourceCommandError::Conflict("Item catalog parent stale"));
                }
            }
            if kind == "claim" {
                claims.insert(id.to_owned(), entry);
            } else {
                records.insert(id.to_owned(), entry);
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
                    "Item catalog publication token",
                ));
            }
            let rows =
                cmd::field(binding, "files")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid(
                        "Item catalog publication file map",
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
                    "Item catalog publication file closure",
                ));
            }
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "Item catalog publication binding absent",
            ));
        }
    }
    digests.insert(
        "ToS/source-witnesses/catalog/catalog.manifest.json".to_owned(),
        raw_hex(&manifest_raw),
    );
    let edition_raw = before
        .get("edition.json")
        .ok_or(SourceCommandError::Conflict("Item selected Edition absent"))?;
    let edition = cmd::parse(edition_raw)?;
    let history = tos_validation::native_compound::inspect_record_history(
        before,
        edition_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let parent = records
        .get(&owner.edition_id)
        .ok_or(SourceCommandError::Conflict("Item catalog Edition absent"))?;
    if cmd::text(&edition, "record_type")? != "edition"
        || cmd::text(&edition, "record_id")? != owner.edition_id
        || cmd::text(parent, "source_record_ref")? != owner.edition_path.as_str()
        || cmd::text(parent, "record_sha256")? != cmd::record_digest(&edition)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "Item catalog exact Edition stale",
        ));
    }
    for id in [
        &owner.item_id,
        cmd::text(&owner.configuration, "file_id")?,
        &owner.claim_id,
    ] {
        if records.contains_key(id) || claims.contains_key(id) {
            return Err(SourceCommandError::Conflict(
                "new Item/File/Claim identity occupied",
            ));
        }
    }
    let events = [
        owner.event_id.as_str(),
        cmd::text(&owner.configuration, "acquisition_event_id")?,
        cmd::text(&owner.configuration, "inventory_event_id")?,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if claims.values().any(|entry| {
        entry
            .object_get("provenance_event_ref")
            .and_then(JsonValue::as_str)
            .is_some_and(|id| events.contains(id))
    }) {
        return Err(SourceCommandError::Conflict(
            "new Item event already cataloged",
        ));
    }
    let origins = claims
        .values()
        .filter(|entry| {
            entry.object_get("predicate").and_then(JsonValue::as_str) == Some("embodied_by")
                && entry.object_get("object").and_then(JsonValue::as_str)
                    == Some(owner.edition_id.as_str())
        })
        .collect::<Vec<_>>();
    let declared = cmd::array(&edition, "embodies_expression_refs")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(SourceCommandError::Invalid("Edition origin identity"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let observed = origins
        .iter()
        .map(|entry| cmd::text(entry, "subject_ref"))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if declared != observed {
        return Err(SourceCommandError::Conflict(
            "Edition lacks exact declared Expression origins",
        ));
    }
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    for entry in origins {
        let claim = catalog_claim(
            fs,
            cut,
            entry,
            &mut digests,
            &mut total_bytes,
            deadline,
            cancelled,
        )?;
        let path = cmd::text(entry, "source_claim_file_ref")?;
        if path.ends_with("/source-claims.jsonl") {
            let mut remaining = limits;
            remaining.max_total_bytes = limits.max_total_bytes.checked_sub(native_bytes).ok_or(
                SourceCommandError::Invalid("Item cumulative topology byte budget"),
            )?;
            remaining.max_state_bytes = limits.max_state_bytes.checked_sub(native_state).ok_or(
                SourceCommandError::Invalid("Item cumulative topology state budget"),
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
            retain_native_topology(
                fs,
                &observation,
                &mut retained_transactions,
                deadline,
                cancelled,
            )?;
            // Full original/current child reconstruction stays with the native
            // reader. The physical original Edition source is checked above.
            let receipt_ref = format!(
                "{}/expression-edition-receipt.json",
                path.rsplit_once('/').unwrap().0
            );
            let raw = source_dependency(
                fs,
                cut,
                &receipt_ref,
                &mut digests,
                &mut total_bytes,
                deadline,
                cancelled,
            )?;
            let receipt = cmd::parse(&raw)?;
            initial_child(&receipt, "edition", &edition, &history)?;
            native_bytes = native_bytes
                .checked_add(observation.bytes_read)
                .ok_or(SourceCommandError::Invalid("Item native read overflow"))?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "Item topology retained state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else {
            legacy_claim(
                fs,
                cut,
                path,
                &claim,
                &mut digests,
                &mut total_bytes,
                deadline,
                cancelled,
            )?;
        }
    }
    let item_entries = records
        .iter()
        .filter(|(_, entry)| {
            entry.object_get("record_type").and_then(JsonValue::as_str) == Some("item")
        })
        .collect::<Vec<_>>();
    if item_entries.len() > 96 {
        return Err(SourceCommandError::Invalid(
            "Item global manifest identity budget",
        ));
    }
    let mut existing_items = BTreeMap::new();
    for (id, entry) in item_entries {
        let path = cmd::text(entry, "source_record_ref")?;
        let raw = source_dependency(
            fs,
            cut,
            path,
            &mut digests,
            &mut total_bytes,
            deadline,
            cancelled,
        )?;
        let record = cmd::parse(&raw)?;
        if cmd::text(&record, "record_id")? != id
            || cmd::text(&record, "record_type")? != "item"
            || cmd::record_digest(&record)?.to_hex() != cmd::text(entry, "record_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "existing Item catalog record stale",
            ));
        }
        let manifest_ref = format!(
            "{}/item.manifest.json",
            path.rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Item catalog home"))?
                .0
        );
        if cmd::text(&record, "item_manifest_ref")? != manifest_ref
            || cmd::text(cmd::field(entry, "links")?, "item_manifest_ref")? != manifest_ref
        {
            return Err(SourceCommandError::Conflict(
                "Item manifest locator differs",
            ));
        }
        let manifest = cmd::parse(&source_dependency(
            fs,
            cut,
            &manifest_ref,
            &mut digests,
            &mut total_bytes,
            deadline,
            cancelled,
        )?)?;
        if cmd::text(&manifest, "schema_version")? != "tos_source_item_manifest_v1"
            || cmd::text(&manifest, "item_id")? != id
            || cmd::array(&manifest, "payload_files")?
                .iter()
                .any(|row| row.object_get("file_id") == owner.configuration.object_get("file_id"))
            || events.contains(cmd::text(&manifest, "acquisition_event_ref")?)
        {
            return Err(SourceCommandError::Conflict(
                "Item File/event identity already owned",
            ));
        }
        for (field, key, rights) in [
            ("rights_ref", "rights_id", true),
            ("resource_inventory_ref", "provenance_event_ref", false),
        ] {
            let ref_path = cmd::text(&manifest, field)?;
            relative(ref_path)?;
            let value = cmd::parse(&source_dependency(
                fs,
                cut,
                ref_path,
                &mut digests,
                &mut total_bytes,
                deadline,
                cancelled,
            )?)?;
            if if rights {
                cmd::text(&value, key)? == cmd::text(&owner.configuration, "rights_id")?
            } else {
                events.contains(cmd::text(&value, key)?)
            } {
                return Err(SourceCommandError::Conflict(
                    "new Item rights/event already owned",
                ));
            }
        }
        if cmd::text(&manifest, "embodiment_ref")? == owner.edition_id {
            existing_items.insert(id.clone(), record);
        }
    }
    let selected = claims
        .iter()
        .filter(|(_, entry)| {
            entry.object_get("predicate").and_then(JsonValue::as_str) == Some("exemplified_by")
                && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                    == Some(owner.edition_id.as_str())
        })
        .collect::<BTreeMap<_, _>>();
    let forward = cmd::array(&edition, "exemplar_claim_refs")?;
    let ids = forward
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or(SourceCommandError::Invalid("Edition exemplar Claim ref"))
        })
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if ids.len() != forward.len()
        || ids != selected.keys().map(|id| id.as_str()).collect()
        || selected.len() != existing_items.len()
    {
        return Err(SourceCommandError::Conflict(
            "Edition Item forward/backlink closure",
        ));
    }
    let mut targets = BTreeSet::new();
    for (id, entry) in selected {
        let claim = catalog_claim(
            fs,
            cut,
            entry,
            &mut digests,
            &mut total_bytes,
            deadline,
            cancelled,
        )?;
        let target = cmd::text(&claim, "object")?;
        if !existing_items.contains_key(target) || !targets.insert(target.to_owned()) {
            return Err(SourceCommandError::Conflict(
                "Edition exemplar target closure",
            ));
        }
        let path = cmd::text(entry, "source_claim_file_ref")?;
        if path.ends_with("/source-claims.jsonl") {
            let mut remaining = limits;
            remaining.max_total_bytes = limits.max_total_bytes.checked_sub(native_bytes).ok_or(
                SourceCommandError::Invalid("Item cumulative native byte budget"),
            )?;
            remaining.max_state_bytes = limits.max_state_bytes.checked_sub(native_state).ok_or(
                SourceCommandError::Invalid("Item cumulative topology state budget"),
            )?;
            let observation = tos_validation::native_compound::verify_edition_item_from_cut(
                cut,
                worker,
                path,
                &serde_value(&claim)?,
                remaining,
                cancelled,
            )
            .map_err(item_error)?;
            retain_native_topology(
                fs,
                &observation,
                &mut retained_transactions,
                deadline,
                cancelled,
            )?;
            if !history["receipts"]
                .as_array()
                .ok_or(SourceCommandError::Invalid("Edition history"))?
                .iter()
                .any(|receipt| {
                    receipt["publication"]["transaction_id"].as_str()
                        == Some(observation.transaction_id.as_str())
                })
            {
                return Err(SourceCommandError::Conflict(
                    "native Item absent from Edition lineage",
                ));
            }
            native_bytes = native_bytes
                .checked_add(observation.bytes_read)
                .ok_or(SourceCommandError::Invalid("Item native bytes overflow"))?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "Item topology retained state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else {
            legacy_claim(
                fs,
                cut,
                path,
                &claim,
                &mut digests,
                &mut total_bytes,
                deadline,
                cancelled,
            )?;
        }
        if cmd::text(&claim, "claim_id")? != id {
            return Err(SourceCommandError::Conflict("Item Claim identity changed"));
        }
    }
    if targets != existing_items.keys().cloned().collect() {
        return Err(SourceCommandError::Conflict("Edition unbound Item"));
    }
    check_publication()?;
    Ok(ItemCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: total_bytes as u64,
    })
}

fn source_dependency(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    path: &str,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = checked_current_source(fs, cut, path, 2_097_152, deadline, cancelled)?;
    *total = total
        .checked_add(raw.len())
        .ok_or(SourceCommandError::Invalid(
            "Item current source byte overflow",
        ))?;
    if *total > 33_554_432 || digests.len() >= 512 {
        return Err(SourceCommandError::Invalid(
            "Item current source dependency budget",
        ));
    }
    if let Some(prior) = digests.insert(path.to_owned(), raw_hex(&raw)) {
        if prior != raw_hex(&raw) {
            return Err(SourceCommandError::Conflict(
                "Item current dependency changed",
            ));
        }
    }
    Ok(raw)
}
fn catalog_claim(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_claim_file_ref")?;
    let raw = source_dependency(fs, cut, path, digests, total, deadline, cancelled)?;
    let line = usize::try_from(cmd::integer(entry, "source_claim_line")?)
        .map_err(|_| SourceCommandError::Invalid("Item Claim line"))?
        .checked_sub(1)
        .ok_or(SourceCommandError::Invalid("Item Claim line"))?;
    let claim = cmd::parse(
        catalog_lines(&raw)
            .nth(line)
            .ok_or(SourceCommandError::Conflict("Item Claim line absent"))?,
    )?;
    for key in [
        "claim_id",
        "predicate",
        "object",
        "subject_ref",
        "provenance_event_ref",
    ] {
        if cmd::field(&claim, key)? != cmd::field(entry, key)? {
            return Err(SourceCommandError::Conflict(
                "Item catalog Claim endpoint stale",
            ));
        }
    }
    if cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")? {
        return Err(SourceCommandError::Conflict(
            "Item catalog Claim digest stale",
        ));
    }
    Ok(claim)
}
fn legacy_claim(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    path: &str,
    claim: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if cmd::text(claim, "claim_type")? != "bibliographic"
        || cmd::text(claim, "assertion_layer")? != "bibliographic_assertion"
        || cmd::text(claim, "provenance_event_ref")?
            != "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
    {
        return Err(SourceCommandError::Denied(
            "legacy Item Claim lacks exact evidence route",
        ));
    }
    let raw = source_dependency(
        fs,
        cut,
        "ToS/source-witnesses/relations/provenance.jsonl",
        digests,
        total,
        deadline,
        cancelled,
    )?;
    let mut rows = catalog_lines(&raw);
    let batch = cmd::parse(
        rows.next()
            .ok_or(SourceCommandError::Conflict("legacy topology batch absent"))?,
    )?;
    if rows.next().is_some()
        || !cmd::array(&batch, "outputs")?.iter().any(|row| {
            row.object_get("ref").and_then(JsonValue::as_str) == Some(path)
                && row.object_get("sha256").and_then(JsonValue::as_str)
                    == digests.get(path).map(String::as_str)
        })
    {
        return Err(SourceCommandError::Conflict(
            "legacy topology retained batch differs",
        ));
    }
    Ok(())
}
fn initial_child(
    receipt: &JsonValue,
    key: &str,
    current: &JsonValue,
    history: &serde_json::Value,
) -> SourceCommandResult<()> {
    let initial = cmd::field(receipt, key)?;
    let current_ref = cmd::reference(current, "record_id", "record_version")?;
    if cmd::same(initial, &current_ref)? {
        return Ok(());
    }
    let initial = serde_value(initial)?;
    if history["receipts"]
        .as_array()
        .is_none_or(|rows| !rows.iter().any(|row| row["previous_source"] == initial))
    {
        return Err(SourceCommandError::Conflict(
            "native initial child is outside current exact lineage",
        ));
    }
    Ok(())
}
fn retain_native_topology(
    fs: &CreationFilesystem,
    observation: &tos_validation::native_compound::NativeCompoundReadObservation,
    retained: &mut BTreeMap<String, String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let (actual, _, _, _) =
        work_transaction::inspect_committed(fs, &observation.transaction_id, deadline, cancelled)?;
    if observation.transport != tos_validation::native_compound::NativeTransportState::Committed
        || actual != observation.manifest_sha256
    {
        return Err(SourceCommandError::Conflict(
            "Item native topology lacks exact physical committed transaction",
        ));
    }
    if let Some(old) = retained.insert(observation.transaction_id.clone(), actual.clone()) {
        if old != actual {
            return Err(SourceCommandError::Conflict(
                "Item repeated native transaction changed",
            ));
        }
    }
    Ok(())
}

const IMPLEMENTATIONS: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_compound_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py",
    "scripts/source_bibliographic_topology.py",
    "scripts/source_metadata_snapshot.py",
    "scripts/source_witness_human_forms.py",
    "scripts/source_record_profiles.py",
    "scripts/build_source_witness_catalog.py",
    "scripts/build_source_resource_inventories.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_deposit.py",
];

fn dependency_bindings(
    catalog: &ItemCatalog,
    grammar: &BTreeMap<String, String>,
    ctx: &CommandContext,
) -> SourceCommandResult<JsonValue> {
    if catalog.digests.len() + grammar.len() + IMPLEMENTATIONS.len() > 512
        || catalog.retained_transactions.len() > 128
    {
        return Err(SourceCommandError::Invalid(
            "Item dependency binding budget",
        ));
    }
    let mut implementation = BTreeMap::new();
    for path in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "Item software subset incomplete",
            ))?;
        if raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid("Item software member budget"));
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

fn remaining(catalog: &ItemCatalog, mut limits: ItemLimits) -> SourceCommandResult<ItemLimits> {
    limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(catalog.native_bytes)
        .and_then(|n| n.checked_sub(catalog.source_bytes))
        .ok_or(SourceCommandError::Invalid("Item whole read budget"))?;
    limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(catalog.native_state)
        .ok_or(SourceCommandError::Invalid("Item whole state budget"))?;
    Ok(limits)
}

fn current_grant(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    // The already parsed grant remains usable only while its exact protected
    // bytes are freshly selected. Avoid reparsing that same JSON per payload chunk.
    fs.current_configuration_bytes(ctx, deadline, cancelled)?;
    let now = crate::source_serialization::instant()?;
    cmd::validate_expiry(cmd::text(config, "expires_at")?, &now)?;
    cmd::validate_expiry(cmd::text(config, "payload_expires_at")?, &now)?;
    Ok(())
}

pub struct ItemAdoptionPreparation {
    request: JsonValue,
    projected_outputs: BTreeMap<String, Vec<u8>>,
    projected_result: JsonValue,
}
impl ItemAdoptionPreparation {
    pub fn projected_result(&self) -> &JsonValue {
        &self.projected_result
    }
    pub fn request(&self) -> &JsonValue {
        &self.request
    }
    pub fn projected_outputs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.projected_outputs
    }
}
pub struct ItemAdoptionPublication {
    transaction_id: String,
    manifest_sha256: Option<String>,
    publication: JsonValue,
    receipt: Option<JsonValue>,
    deposit: JsonValue,
    replayed: bool,
    materializations: Option<JsonValue>,
}
impl ItemAdoptionPublication {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }
    pub fn manifest_sha256(&self) -> Option<&str> {
        self.manifest_sha256.as_deref()
    }
    pub fn publication(&self) -> &JsonValue {
        &self.publication
    }
    pub fn receipt(&self) -> Option<&JsonValue> {
        self.receipt.as_ref()
    }
    pub fn deposit(&self) -> &JsonValue {
        &self.deposit
    }
    pub fn replayed(&self) -> bool {
        self.replayed
    }
    pub fn materializations(&self) -> Option<&JsonValue> {
        self.materializations.as_ref()
    }
    pub fn metadata_committed(&self) -> bool {
        self.receipt.is_some()
    }
}

fn prepare_request(
    owner: &ItemOwner,
    before: &BTreeMap<String, Vec<u8>>,
    snapshot: &PublicationSnapshot,
    observation: &serde_json::Value,
    recorded_at: &str,
) -> SourceCommandResult<JsonValue> {
    let edition = cmd::parse(
        before
            .get("edition.json")
            .ok_or(SourceCommandError::Conflict("Item parent absent"))?,
    )?;
    let mut refs = cmd::array(&edition, "exemplar_claim_refs")?.to_vec();
    if refs.len() >= 128 || refs.iter().any(|v| v.as_str() == Some(&owner.claim_id)) {
        return Err(SourceCommandError::Conflict("Item Claim append invalid"));
    }
    refs.push(cmd::string(&owner.claim_id));
    let mut request = owner.request.clone();
    for (key, value) in [
        ("operation", cmd::string(OPERATION)),
        ("command_id", cmd::string("preview:uncommitted")),
        (
            "fields",
            cmd::object(vec![("exemplar_claim_refs", JsonValue::Array(refs))]),
        ),
        (
            "expected_configuration",
            cmd::string(&owner.configuration_digest),
        ),
        (
            "expected_source",
            cmd::reference(&edition, "record_id", "record_version")?,
        ),
        (
            "expected_revision",
            cmd::string(&crate::source_revisions::revision(before)?),
        ),
        (
            "expected_publication",
            snapshot
                .token
                .as_ref()
                .map_or(JsonValue::Null, |v| cmd::string(v)),
        ),
        ("inventory", foundation_value(&observation["inventory"])?),
        (
            "inventory_limitation",
            foundation_value(&observation["limitation"])?,
        ),
        ("fixity_verified_at", cmd::string(recorded_at)),
    ] {
        cmd::set(&mut request, key, value)?;
    }
    Ok(request)
}

fn unavailable_grammar(
    owner: &ItemOwner,
    before: &BTreeMap<String, Vec<u8>>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<tos_validation::native_compound::EditionItemGrammarObservation> {
    let edition_raw = before
        .get("edition.json")
        .ok_or(SourceCommandError::Conflict("Item parent absent"))?;
    let item_raw = cmd::canonical(cmd::field(&owner.request, "record")?)?;
    let claim_raw = cmd::canonical(cmd::field(&owner.request, "claim")?)?;
    let mut grammar = tos_validation::native_compound::inspect_edition_item_grammar(
        cut,
        worker,
        edition_raw,
        &item_raw,
        &claim_raw,
        limits,
        cancelled,
    )
    .map_err(item_error)?;
    let mut revised = cmd::parse(edition_raw)?;
    cmd::set(
        &mut revised,
        "exemplar_claim_refs",
        cmd::field(cmd::field(&owner.request, "fields")?, "exemplar_claim_refs")?.clone(),
    )?;
    let version = cmd::integer(&revised, "record_version")?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid("Item parent version overflow"))?;
    cmd::set(&mut revised, "record_version", cmd::number(version))?;
    let revised_raw = cmd::canonical(&revised)?;
    let mut delta_limits = limits;
    delta_limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(grammar.bytes_read)
        .ok_or(SourceCommandError::Invalid(
            "Item unavailable grammar read budget",
        ))?;
    delta_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(grammar.returned_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item unavailable grammar state budget",
        ))?;
    let delta = tos_validation::biblio_rules::inspect_bibliographic_delta(
        tos_validation::biblio_rules::BiblioDeltaInput {
            parent_path: owner.edition_path.as_str(),
            endpoint_path: owner.item_path.as_str(),
            claim_path: &format!(
                "{}/source-claims.jsonl",
                owner.item_path.as_str().rsplit_once('/').unwrap().0
            ),
            parent_before_raw: edition_raw,
            parent_after_raw: &revised_raw,
            endpoint_raw: &item_raw,
            claim_raw: &claim_raw,
        },
        delta_limits,
        cancelled,
        worker,
    )
    .map_err(item_error)?;
    let required = format!(
        "{}@{}:exemplified_by",
        tos_validation::biblio_rules::BIBLIOGRAPHIC_DELTA_RULE_ID,
        tos_validation::biblio_rules::BIBLIOGRAPHIC_DELTA_RULE_VERSION
    );
    if !delta.issues.is_empty()||delta.issue_sink_truncated||!delta.checked_profiles.contains(&required)
        ||delta.skipped_profiles.len()!=1||!delta.skipped_profiles.contains("whole-compound-plan-forms-dependency-byte-custody-current-lineage-and-permission-fence") {
        return Err(SourceCommandError::Conflict("Item explicit append/backlink mechanics refused"));
    }
    let supplied_bytes = [
        edition_raw.as_slice(),
        revised_raw.as_slice(),
        item_raw.as_slice(),
        claim_raw.as_slice(),
    ]
    .iter()
    .try_fold(0u64, |n, r| n.checked_add(r.len() as u64))
    .ok_or(SourceCommandError::Invalid(
        "Item delta input budget overflow",
    ))?;
    grammar.bytes_read = grammar
        .bytes_read
        .checked_add(supplied_bytes)
        .filter(|n| *n <= limits.max_total_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item whole grammar/delta read budget",
        ))?;
    let retained = delta
        .reads
        .iter()
        .try_fold(0usize, |n, row| {
            n.checked_add(format!("{row:?}").len() + 64)
        })
        .ok_or(SourceCommandError::Invalid(
            "Item retained delta observation state overflow",
        ))?;
    grammar.returned_state_bytes = grammar
        .returned_state_bytes
        .checked_add(retained)
        .filter(|n| *n <= limits.max_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item whole grammar/delta state budget",
        ))?;
    grammar.reads.extend(delta.reads);
    Ok(grammar)
}

pub fn prepare_isolated_item_adoption_from_proposal(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPreparation> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let mut owner = ItemOwner::select_proposal(fs, ctx, limits.deadline, cancelled)?;
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
    owner.new_directories(fs, limits.deadline, cancelled, None)?;
    let config = serde_value(&owner.configuration)?;
    let observation = deposit::observe(&config, fs.uid, limits.deadline, cancelled, &mut || {
        current_grant(fs, ctx, &owner.configuration, limits.deadline, cancelled)?;
        snapshot.verify_current(fs, limits.deadline, cancelled)
    })?;
    owner.request = prepare_request(&owner, &before, &snapshot, &observation, &ctx.recorded_at)?;
    let remaining = remaining(&catalog, limits)?;
    let (projected_outputs, authorization, reads) = if observation["inventory"].is_null() {
        let grammar = unavailable_grammar(&owner, &before, cut, worker, remaining, cancelled)?;
        let dependencies = dependency_bindings(&catalog, &grammar.grammar_digests, ctx)?;
        cmd::set(
            &mut owner.request,
            "expected_dependencies",
            cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
        )?;
        (
            BTreeMap::new(),
            owner.authorization(dependencies)?,
            grammar.reads,
        )
    } else {
        let mut claim_raw = cmd::canonical(cmd::field(&owner.request, "claim")?)?;
        claim_raw.push(b'\n');
        let mut authorization_error = None;
        let scope = serde_value(&owner.scope()?)?;
        let core = tos_validation::native_compound::prepare_edition_item_preview_bytes(
            cut,
            worker,
            &scope,
            &claim_raw,
            &before,
            &ctx.recorded_at,
            GENERATOR,
            remaining,
            cancelled,
            |grammar| {
                let result = (|| {
                    let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                    cmd::set(
                        &mut owner.request,
                        "expected_dependencies",
                        cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
                    )?;
                    let authorization = serde_value(&owner.authorization(dependencies)?)?;
                    let mut raw = cmd::canonical(&owner.request)?;
                    raw.push(b'\n');
                    Ok((raw, authorization))
                })();
                result.map_err(|e| {
                    authorization_error = Some(e);
                    ItemRefusal::Source("Item preview authorization rejected".to_owned())
                })
            },
        );
        let core = core.map_err(|e| authorization_error.unwrap_or_else(|| item_error(e)))?;
        let authorization = foundation_value(core.authorization())?;
        let reads = core.reads().to_vec();
        (
            core.into_prepared_outputs().map_err(item_error)?,
            authorization,
            reads,
        )
    };
    let mut all_reads = catalog.native_reads;
    all_reads.extend(reads);
    let projected = projected_outputs
        .iter()
        .map(|(p, r)| (p.clone(), r.as_slice()))
        .collect::<Vec<_>>();
    selected_reads_current(
        fs,
        cut,
        &all_reads,
        &BTreeMap::new(),
        &projected,
        WorkControlRead::Ready(&snapshot),
        limits.max_total_bytes,
        limits.deadline,
        cancelled,
    )?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    current_grant(fs, ctx, &owner.configuration, limits.deadline, cancelled)?;
    software_current(fs, ctx, limits.deadline, cancelled)?;
    work_dependencies_current(
        fs,
        cmd::field(&authorization, "dependency_bindings")?,
        limits.deadline,
        cancelled,
    )?;
    complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    let projected_result = if projected_outputs.is_empty() {
        cmd::object(vec![
            ("prepared_claim", JsonValue::Null),
            ("prepared_forms", JsonValue::Null),
            ("prepared_materializations", JsonValue::Null),
        ])
    } else {
        let edition_home = owner.edition_path.as_str().rsplit_once('/').unwrap().0;
        let item_home = owner.item_path.as_str().rsplit_once('/').unwrap().0;
        let parent = projected_outputs
            .iter()
            .filter_map(|(p, r)| {
                p.strip_prefix(&format!("{edition_home}/"))
                    .filter(|name| !name.contains('/'))
                    .map(|n| (n.to_owned(), r.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let child = projected_outputs
            .iter()
            .filter_map(|(p, r)| {
                p.strip_prefix(&format!("{item_home}/"))
                    .filter(|name| !name.contains('/'))
                    .map(|n| (n.to_owned(), r.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let edition = cmd::parse(
            parent
                .get("edition.json")
                .ok_or(SourceCommandError::Conflict("Item prepared Edition absent"))?,
        )?;
        let item = cmd::parse(
            child
                .get("item.json")
                .ok_or(SourceCommandError::Conflict("Item prepared Item absent"))?,
        )?;
        let claim = cmd::parse(
            child
                .get("source-claims.jsonl")
                .ok_or(SourceCommandError::Conflict("Item prepared Claim absent"))?,
        )?;
        let mut forms = Vec::new();
        for (kind, map, name) in [
            ("edition", &parent, "edition.human-forms.json".to_owned()),
            ("item", &child, "item.human-forms.json".to_owned()),
            (
                "claim",
                &child,
                format!(
                    "source-claims.{}.human-forms.json",
                    Digest256::of_bytes(owner.claim_id.as_bytes()).to_hex()
                ),
            ),
        ] {
            let set = cmd::parse(map.get(&name).ok_or(SourceCommandError::Conflict(
                "Item prepared form set absent",
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
        cmd::object(vec![
            (
                "prepared_edition",
                cmd::reference(&edition, "record_id", "record_version")?,
            ),
            (
                "prepared_item",
                cmd::reference(&item, "record_id", "record_version")?,
            ),
            (
                "prepared_claim",
                cmd::reference(&claim, "claim_id", "claim_version")?,
            ),
            ("prepared_forms", cmd::object(forms)),
            (
                "prepared_materializations",
                compound_views(&parent, &child, &owner.claim_id)?,
            ),
        ])
    };
    Ok(ItemAdoptionPreparation {
        request: owner.request,
        projected_outputs,
        projected_result,
    })
}

pub fn execute_isolated_item_adoption_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPublication> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let owner = ItemOwner::select(fs, ctx, limits.deadline, cancelled)?;
    if work_transaction::read_pending(fs, limits.deadline, cancelled)?.is_some()
        || work_transaction::retained_item_orphan(
            fs,
            &owner.transaction_id()?,
            limits.deadline,
            cancelled,
        )?
        .is_some()
    {
        return recover_item_selected(
            fs, ctx, cut, software, components, worker, true, limits, cancelled,
        );
    }
    execute_item_selected(
        fs, ctx, owner, cut, software, components, worker, false, limits, cancelled,
    )
}
fn execute_item_selected(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    owner: ItemOwner,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    recovery: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPublication> {
    let grant = cmd::parse(&ctx.configuration_raw)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let prior_id =
        original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    if cmd::field(&owner.request, "expected_publication")?.as_str() != snapshot.token.as_deref()
        && !(cmd::field(&owner.request, "expected_publication")? == &JsonValue::Null
            && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "Item requested publication changed",
        ));
    }
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
    // As in the maintained owner, grammar and append/backlink mechanics are
    // checked before copying the separately granted File. Real serialization
    // then consumes the genuine deposit receipt, never a preview marker.
    let grammar = unavailable_grammar(
        &owner,
        &before,
        cut,
        worker,
        remaining(&catalog, limits)?,
        cancelled,
    )?;
    let dependencies = dependency_bindings(&catalog, &grammar.grammar_digests, ctx)?;
    if cmd::record_digest(&dependencies)?.to_prefixed()
        != cmd::text(&owner.request, "expected_dependencies")?
    {
        return Err(SourceCommandError::Conflict(
            "Item current preparation dependencies changed",
        ));
    }
    current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
    software_current(fs, ctx, limits.deadline, cancelled)?;
    work_dependencies_current(fs, &dependencies, limits.deadline, cancelled)?;
    let config = serde_value(&grant)?;
    let existing_stage = deposit::read_stage(
        &config,
        &owner.transaction_id()?,
        fs.uid,
        limits.deadline,
        cancelled,
    )?;
    if existing_stage.is_none() {
        owner.new_directories(fs, limits.deadline, cancelled, None)?;
    }
    let stage = deposit::ensure_deposit(
        &config,
        &serde_value(&owner.request)?,
        &owner.transaction_id()?,
        fs.uid,
        recovery,
        limits.deadline,
        cancelled,
        &mut || {
            current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
            snapshot.verify_current(fs, limits.deadline, cancelled)?;
            fence.verify(limits.deadline, cancelled)
        },
    )?;
    if cmd::field(&owner.request, "inventory")? == &JsonValue::Null {
        worker
            .finish(limits.deadline, cancelled)
            .map_err(item_error)?;
        current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
        snapshot.verify_current(fs, limits.deadline, cancelled)?;
        return Ok(ItemAdoptionPublication {
            transaction_id: owner.transaction_id()?,
            manifest_sha256: None,
            publication: JsonValue::Null,
            receipt: None,
            deposit: foundation_value(&deposit::public_state(&stage)?)?,
            replayed: false,
            materializations: None,
        });
    }
    let directories = owner.new_directories(fs, limits.deadline, cancelled, Some(&stage))?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let receipt_raw = cmd::canonical(&foundation_value(&deposit::public_receipt(&stage)?)?)?;
    let mut core_limits = remaining(&catalog, limits)?;
    core_limits.max_total_bytes = core_limits
        .max_total_bytes
        .checked_sub(grammar.bytes_read)
        .ok_or(SourceCommandError::Invalid("Item pre-copy grammar budget"))?;
    core_limits.max_state_bytes = core_limits
        .max_state_bytes
        .checked_sub(grammar.returned_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item pre-copy grammar state budget",
        ))?;
    let authorization = owner.authorization(dependencies.clone())?;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_edition_item_bytes(
        cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &ctx.recorded_at,
        &receipt_raw,
        GENERATOR,
        core_limits,
        cancelled,
        |actual_grammar| {
            let result = (|| {
                let actual = dependency_bindings(&catalog, actual_grammar, ctx)?;
                if !cmd::same(&actual, &dependencies)? {
                    return Err(SourceCommandError::Conflict(
                        "Item serialization grammar differs from checked copy basis",
                    ));
                }
                serde_value(&authorization)
            })();
            result.map_err(|e| {
                authorization_error = Some(e);
                ItemRefusal::Source("Item owner authorization rejected".to_owned())
            })
        },
    );
    let core = core.map_err(|e| authorization_error.unwrap_or_else(|| item_error(e)))?;
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "Item native transaction identity differs",
        ));
    }
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(p, r)| (p.as_str(), *r))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_item_adoption(
        &owner.request,
        &owner.event_id,
        owner.item_path.as_str().rsplit_once('/').unwrap().0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_edition_item_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Item finished byte identity differs",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let materializations = compound_views(&finished.parent, &finished.child, &owner.claim_id)?;
    let mut read_observations = catalog.native_reads;
    read_observations.extend(grammar.reads);
    read_observations.extend(finished.reads);
    let plan = owner.plan(
        authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    let selected = selected_sides(&plan)?;
    after_cut_budget(cut, &selected)?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let old = cmd::parse(
        before
            .get("edition.json")
            .ok_or(SourceCommandError::Conflict("Item parent absent"))?,
    )?;
    let stored_archive = work_transaction::item_archive(
        fs,
        owner.edition_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        true,
    )?;
    if stored_archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Item archive differs from native recipe",
        ));
    }
    let guard = WorkApplicationGuard {
        snapshot,
        prior_id,
        selected,
        read_observations,
        stored_archive,
        authorization,
    };
    let payload_identity =
        deposit::deposited_identity(&config, &stage, fs.uid, limits.deadline, cancelled)?;
    let applied = fence.apply(
        plan,
        &guard.snapshot,
        |summary, extent| {
            current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
            deposit::stage_current(
                &config,
                &stage,
                &payload_identity,
                fs.uid,
                limits.deadline,
                cancelled,
            )?;
            if extent.full_membership {
                deposit::verify_deposit(
                    &config,
                    &stage,
                    fs.uid,
                    limits.deadline,
                    cancelled,
                    &mut || current_grant(fs, ctx, &grant, limits.deadline, cancelled),
                )?;
            }
            guard.check(fs, ctx, cut, summary, extent, limits, cancelled)?;
            current_grant(fs, ctx, &grant, limits.deadline, cancelled)
        },
        limits.deadline,
        cancelled,
    )?;
    if !applied.committed {
        return Err(SourceCommandError::Conflict(
            "Item metadata publication rolled back",
        ));
    }
    let mut deposit_state = deposit::public_state(&stage)?;
    deposit_state["metadata_committed"] = serde_json::Value::Bool(true);
    Ok(ItemAdoptionPublication {
        transaction_id: applied.transaction_id,
        manifest_sha256: Some(applied.manifest_sha256),
        publication: applied.publication,
        receipt: Some(receipt),
        deposit: foundation_value(&deposit_state)?,
        replayed: false,
        materializations: Some(materializations),
    })
}

pub fn replay_isolated_item_adoption_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner = ItemOwner::select(fs, original_ctx, limits.deadline, cancelled)?;
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
            "Item original replay basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Item original revision differs",
        ));
    }
    let edition_home = owner.edition_path.as_str().rsplit_once('/').unwrap().0;
    let item_home = owner.item_path.as_str().rsplit_once('/').unwrap().0;
    let mut parent = BTreeMap::new();
    let mut child = BTreeMap::new();
    for file in &retained.files {
        let path = file.path.as_str();
        let (home, name) = path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Item retained selected path"))?;
        let after = file.after.as_ref().ok_or(SourceCommandError::Conflict(
            "Item retained selected output absent",
        ))?;
        if home == edition_home {
            parent.insert(name.to_owned(), after.clone());
        } else if home == item_home {
            child.insert(name.to_owned(), after.clone());
        } else {
            return Err(SourceCommandError::Conflict(
                "Item retained selected path outside scope",
            ));
        }
    }
    let materializations = compound_views(&parent, &child, &owner.claim_id)?;
    let expected = owner.plan(
        retained.authorization.clone(),
        &before,
        parent,
        child,
        retained.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &retained)? {
        return Err(SourceCommandError::Conflict(
            "Item retained plan differs from original cut",
        ));
    }
    let receipt_path = format!("{item_home}/edition-item-receipt.json");
    let receipt_raw = retained
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict("Item retained receipt absent"))?;
    let receipt = cmd::parse(receipt_raw)?;
    let claims_path = relative(&format!("{item_home}/source-claims.jsonl"))?;
    let claims_raw = current_cut
        .read_member(
            current_cut.current().revision(),
            &claims_path,
            2_097_152,
            limits.deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Unsupported("current Item Claim custody incomplete"))?
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
                return Err(SourceCommandError::Conflict("duplicate current Item Claim"));
            }
        }
    }
    let current_claim =
        current_claim.ok_or(SourceCommandError::Conflict("current Item Claim absent"))?;
    let observation = tos_validation::native_compound::verify_edition_item_from_cut(
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
    {
        return Err(SourceCommandError::Conflict(
            "Item committed native lineage differs",
        ));
    }
    let config = serde_value(&owner.configuration)?;
    let stage = deposit::read_stage(&config, &transaction_id, fs.uid, limits.deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict(
            "Item replay private deposit stage absent",
        ))?;
    let deposited_receipt = retained
        .files
        .iter()
        .find(|f| f.path.as_str() == format!("{item_home}/item-deposit-receipt.json"))
        .and_then(|f| f.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "Item replay public deposit receipt absent",
        ))?;
    if !cmd::same(
        &cmd::parse(deposited_receipt)?,
        &foundation_value(&deposit::public_receipt(&stage)?)?,
    )? {
        return Err(SourceCommandError::Conflict(
            "Item replay genuine deposit receipt differs",
        ));
    }
    let payload_identity =
        deposit::deposited_identity(&config, &stage, fs.uid, limits.deadline, cancelled)?;
    selected_reads_current(
        fs,
        current_cut,
        &observation.reads,
        &BTreeMap::new(),
        &[],
        WorkControlRead::Ready(&snapshot),
        limits.max_total_bytes,
        limits.deadline,
        cancelled,
    )?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    current_grant(
        fs,
        original_ctx,
        &owner.configuration,
        limits.deadline,
        cancelled,
    )?;
    deposit::stage_current(
        &config,
        &stage,
        &payload_identity,
        fs.uid,
        limits.deadline,
        cancelled,
    )?;
    deposit::verify_deposit(
        &config,
        &stage,
        fs.uid,
        limits.deadline,
        cancelled,
        &mut || {
            current_grant(
                fs,
                original_ctx,
                &owner.configuration,
                limits.deadline,
                cancelled,
            )
        },
    )?;
    fence.verify(limits.deadline, cancelled)?;
    complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let (current_manifest, _, _, _) =
        work_transaction::inspect_committed(fs, &transaction_id, limits.deadline, cancelled)?;
    if current_manifest != manifest_sha256 {
        return Err(SourceCommandError::Conflict("Item retained replay changed"));
    }
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(ItemAdoptionPublication {
        transaction_id,
        manifest_sha256: Some(manifest_sha256),
        publication: terminal,
        receipt: Some(receipt),
        deposit: {
            let mut v = deposit::public_state(&stage)?;
            v["metadata_committed"] = serde_json::Value::Bool(true);
            foundation_value(&v)?
        },
        replayed: true,
        materializations: Some(materializations),
    })
}

fn recovery_owner(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    stage: &serde_json::Value,
    pending: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemOwner> {
    let current = cmd::parse(&ctx.configuration_raw)?;
    let original = foundation_value(&stage["binding"]["configuration"])?;
    let request = foundation_value(&stage["binding"]["request"])?;
    let owner = ItemOwner::select_values(
        fs,
        ctx,
        false,
        Some((&original, &request)),
        limits.deadline,
        cancelled,
    )?;
    let keys = original
        .as_object()
        .ok_or(SourceCommandError::Invalid("Item retained configuration"))?
        .iter()
        .map(|(k, _)| {
            k.as_str()
                .ok_or(SourceCommandError::Invalid("Item configuration key"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    cmd::exact_keys(&current, &keys)?;
    let renewable = [
        "principal_id",
        "maker_type",
        "authority_ref",
        "expires_at",
        "allowed_operations",
        "payload_authority_ref",
        "payload_expires_at",
    ];
    for key in keys {
        if renewable.contains(&key) || (pending && key.starts_with("allowed_")) {
            continue;
        }
        if !cmd::same(cmd::field(&current, key)?, cmd::field(&original, key)?)? {
            return Err(SourceCommandError::Denied(
                "Item recovery changes frozen identity or custody",
            ));
        }
    }
    if !matches!(
        cmd::text(&current, "maker_type")?,
        "human" | "software" | "model"
    ) || !cmd::nonblank(cmd::text(&current, "principal_id")?)
        || !cmd::nonblank(cmd::text(&current, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied("Item current recovery actor"));
    }
    let operations = cmd::array(&current, "allowed_operations")?;
    if operations.is_empty()
        || operations.len() > 2
        || !operations.iter().any(|v| v.as_str() == Some(RECOVERY))
        || operations
            .iter()
            .any(|v| !matches!(v.as_str(), Some(OPERATION | RECOVERY)))
    {
        return Err(SourceCommandError::Denied("Item recovery not delegated"));
    }
    for (field, allowed) in [
        ("forms", "allowed_edition_form_ids"),
        ("item_forms", "allowed_item_form_ids"),
        ("claim_forms", "allowed_claim_form_ids"),
    ] {
        selected_forms(&owner.request, field, &allowed_forms(&current, allowed)?)?;
    }
    current_grant(fs, ctx, &current, limits.deadline, cancelled)?;
    deposit::validate_config(&serde_value(&current)?, fs.uid)?;
    Ok(owner)
}

pub fn recover_isolated_item_adoption_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPublication> {
    recover_item_selected(
        fs,
        ctx,
        original_cut,
        software,
        components,
        worker,
        false,
        limits,
        cancelled,
    )
}
fn recover_item_selected(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exact_retry: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ItemAdoptionPublication> {
    ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let request = cmd::parse(&ctx.request_raw)?;
    let grant = cmd::parse(&ctx.configuration_raw)?;
    let (transaction_id, rollback) = if exact_retry {
        let selected = ItemOwner::select(fs, ctx, limits.deadline, cancelled)?;
        (selected.transaction_id()?, false)
    } else {
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
        let rollback = match cmd::text(&request, "decision")? {
            "resume" => false,
            "rollback" => true,
            _ => return Err(SourceCommandError::Invalid("Item recovery decision")),
        };
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")? != RECOVERY
            || cmd::text(&request, "expected_configuration")?
                != cmd::record_digest(&grant)?.to_prefixed()
        {
            return Err(SourceCommandError::Denied(
                "Item recovery current selection",
            ));
        }
        (cmd::text(&request, "transaction_id")?.to_owned(), rollback)
    };
    let transaction_id = transaction_id.as_str();
    let config = serde_value(&grant)?;
    current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
    let stage = deposit::read_stage(&config, transaction_id, fs.uid, limits.deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict(
        "Item retained byte stage absent",
    ))?;
    let pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?;
    let orphan = if pending.is_none() {
        work_transaction::retained_item_orphan(fs, transaction_id, limits.deadline, cancelled)?
    } else {
        None
    };
    let owner = if exact_retry {
        if foundation_value(&stage["binding"]["request"])? != request
            || foundation_value(&stage["binding"]["configuration"])? != grant
        {
            return Err(SourceCommandError::Conflict(
                "Item exact retry changed original grant or request",
            ));
        }
        ItemOwner::select(fs, ctx, limits.deadline, cancelled)?
    } else {
        recovery_owner(fs, ctx, &stage, pending.is_some(), limits, cancelled)?
    };
    if owner.transaction_id()? != transaction_id {
        return Err(SourceCommandError::Conflict(
            "Item recovery original identity differs",
        ));
    }
    if pending.is_none() && (orphan.is_none() || rollback) {
        // A terminal acquired Item is never undone by byte recovery.
        if fs
            .root_path
            .join(owner.item_path.as_str())
            .symlink_metadata()
            .is_ok()
        {
            return Err(SourceCommandError::Conflict(
                "Item metadata already exists; byte recovery cannot undo it",
            ));
        }
        if rollback {
            worker
                .finish(limits.deadline, cancelled)
                .map_err(item_error)?;
            let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
            fence.verify(limits.deadline, cancelled)?;
            let deposited = deposit::rollback_retained(
                &config,
                transaction_id,
                fs.uid,
                limits.deadline,
                cancelled,
                &mut || current_grant(fs, ctx, &grant, limits.deadline, cancelled),
            )?;
            return Ok(ItemAdoptionPublication {
                transaction_id: transaction_id.to_owned(),
                manifest_sha256: None,
                publication: JsonValue::Null,
                receipt: None,
                deposit: foundation_value(&deposited)?,
                replayed: false,
                materializations: None,
            });
        }
        return execute_item_selected(
            fs,
            ctx,
            owner,
            original_cut,
            software,
            components,
            worker,
            true,
            limits,
            cancelled,
        );
    }
    let ready_snapshot = if pending.is_none() {
        Some(PublicationSnapshot::select(fs, limits.deadline, cancelled)?)
    } else {
        None
    };
    let (plan, base_publication) = if let Some(pending) = &pending {
        (pending.plan.clone(), pending.base_publication.clone())
    } else {
        orphan.ok_or(SourceCommandError::Conflict(
            "Item orphan selection vanished",
        ))?
    };
    if pending
        .as_ref()
        .is_some_and(|p| cmd::text(&p.state, "transaction_id").ok() != Some(transaction_id))
        || plan.transaction_id != transaction_id
        || cmd::field(&base_publication, "token")?
            != cmd::field(&owner.request, "expected_publication")?
        || !cmd::same(
            &plan.authorization,
            &owner
                .authorization(cmd::field(&plan.authorization, "dependency_bindings")?.clone())?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Item pending exact original authority differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Item recovery original revision differs",
        ));
    }
    let old = cmd::parse(
        before
            .get("edition.json")
            .ok_or(SourceCommandError::Conflict("Item predecessor absent"))?,
    )?;
    let archive = work_transaction::item_archive(
        fs,
        owner.edition_path.as_str(),
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
        || {
            if let Some(pending) = &pending {
                work_transaction::still_pending(fs, &pending.state, limits.deadline, cancelled)
            } else {
                ready_snapshot
                    .as_ref()
                    .unwrap()
                    .verify_current(fs, limits.deadline, cancelled)
            }
        },
        limits.deadline,
        cancelled,
    )?;
    let item_home = owner.item_path.as_str().rsplit_once('/').unwrap().0;
    let selected_after = |name: &str| -> SourceCommandResult<&[u8]> {
        plan.files
            .iter()
            .find(|f| f.path.as_str() == format!("{item_home}/{name}"))
            .and_then(|f| f.after.as_deref())
            .ok_or(SourceCommandError::Conflict(
                "Item pending native capture absent",
            ))
    };
    let receipt = cmd::parse(selected_after("edition-item-receipt.json")?)?;
    let byte_receipt = selected_after("item-deposit-receipt.json")?;
    if !cmd::same(
        &cmd::parse(byte_receipt)?,
        &foundation_value(&deposit::public_receipt(&stage)?)?,
    )? {
        return Err(SourceCommandError::Conflict(
            "Item pending genuine deposit receipt changed",
        ));
    }
    let scope = serde_value(&owner.scope()?)?;
    let request_raw = cmd::canonical(&owner.request)?;
    let authorization =
        owner.authorization(cmd::field(&plan.authorization, "dependency_bindings")?.clone())?;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_edition_item_bytes(
        original_cut,
        worker,
        &scope,
        &request_raw,
        &before,
        cmd::text(&receipt, "recorded_at")?,
        byte_receipt,
        GENERATOR,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                if !cmd::same(
                    &dependencies,
                    cmd::field(&authorization, "dependency_bindings")?,
                )? {
                    return Err(SourceCommandError::Conflict(
                        "Item pending dependencies changed",
                    ));
                }
                serde_value(&authorization)
            })();
            result.map_err(|e| {
                authorization_error = Some(e);
                ItemRefusal::Source("Item recovery authorization rejected".to_owned())
            })
        },
    );
    let core = core.map_err(|e| authorization_error.unwrap_or_else(|| item_error(e)))?;
    if core.transaction_id() != transaction_id || core.archive_path() != archive.path {
        return Err(SourceCommandError::Conflict(
            "Item pending native recipe identity differs",
        ));
    }
    let finished = tos_validation::native_compound::finish_edition_item_bytes(
        core,
        selected_after("source-create-environment.json")?,
        selected_after("source-create-provenance.jsonl")?,
        worker,
    )
    .map_err(item_error)?;
    let materializations = compound_views(&finished.parent, &finished.child, &owner.claim_id)?;
    let expected = owner.plan(
        authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        plan.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &plan)? {
        return Err(SourceCommandError::Conflict(
            "Item pending retained plan differs from native recipe",
        ));
    }
    let selected = selected_sides(&plan)?;
    let mut reads = catalog.native_reads;
    reads.extend(finished.reads);
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    if let Some(pending) = &pending {
        let held = work_transaction::read_pending(fs, limits.deadline, cancelled)?
            .ok_or(SourceCommandError::Conflict("Item pending vanished"))?;
        if !cmd::same(&held.state, &pending.state)?
            || !cmd::same(&held.base_publication, &base_publication)?
            || !same_selected_plan(&held.plan, &plan)?
        {
            return Err(SourceCommandError::Conflict(
                "Item pending changed before recovery lock",
            ));
        }
    } else {
        let (held, base) =
            work_transaction::retained_item_orphan(fs, transaction_id, limits.deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict("Item orphan vanished"))?;
        if !same_selected_plan(&held, &plan)? || !cmd::same(&base, &base_publication)? {
            return Err(SourceCommandError::Conflict(
                "Item orphan changed before recovery lock",
            ));
        }
    }
    let identity =
        deposit::deposited_identity(&config, &stage, fs.uid, limits.deadline, cancelled)?;
    let renewal = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_item_adoption_recovery_authorization_v1"),
        ),
        ("principal_id", cmd::field(&grant, "principal_id")?.clone()),
        (
            "authority_ref",
            cmd::field(&grant, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::string(&cmd::record_digest(&grant)?.to_prefixed()),
        ),
        ("transaction_id", cmd::string(transaction_id)),
        (
            "decision",
            cmd::string(if rollback { "rollback" } else { "resume" }),
        ),
    ]);
    let guard = |summary: &JsonValue, extent: work_transaction::WorkGuard<'_>| {
        current_grant(fs, ctx, &grant, limits.deadline, cancelled)?;
        if !cmd::same(cmd::field(summary, "authorization")?, &authorization)? {
            return Err(SourceCommandError::Conflict(
                "Item recovery authorization changed",
            ));
        }
        deposit::stage_current(
            &config,
            &stage,
            &identity,
            fs.uid,
            limits.deadline,
            cancelled,
        )?;
        if extent.full_membership {
            deposit::verify_deposit(
                &config,
                &stage,
                fs.uid,
                limits.deadline,
                cancelled,
                &mut || current_grant(fs, ctx, &grant, limits.deadline, cancelled),
            )?;
        }
        physical_current(
            fs,
            ctx,
            original_cut,
            &selected,
            cmd::field(&authorization, "dependency_bindings")?,
            ready_snapshot.as_ref(),
            &archive,
            prior_id.as_deref().zip(publication_token),
            &extent,
            limits.deadline,
            cancelled,
        )?;
        let control = if let Some(state) = extent.pending_state {
            WorkControlRead::Pending(state)
        } else {
            WorkControlRead::Ready(ready_snapshot.as_ref().ok_or(SourceCommandError::Conflict(
                "Item recovery ready snapshot absent",
            ))?)
        };
        selected_reads_current(
            fs,
            original_cut,
            &reads,
            &selected,
            &[],
            control,
            limits.max_total_bytes,
            limits.deadline,
            cancelled,
        )?;
        current_grant(fs, ctx, &grant, limits.deadline, cancelled)
    };
    let result = if let Some(pending) = &pending {
        fence.recover(
            pending,
            rollback,
            if exact_retry { None } else { Some(renewal) },
            guard,
            limits.deadline,
            cancelled,
        )?
    } else {
        fence.apply_retained_item(
            plan,
            ready_snapshot.as_ref().unwrap(),
            if exact_retry { None } else { Some(renewal) },
            guard,
            limits.deadline,
            cancelled,
        )?
    };
    let deposited = if rollback {
        deposit::rollback_retained(
            &config,
            transaction_id,
            fs.uid,
            limits.deadline,
            cancelled,
            &mut || current_grant(fs, ctx, &grant, limits.deadline, cancelled),
        )?
    } else {
        let mut v = deposit::public_state(&stage)?;
        v["metadata_committed"] = serde_json::Value::Bool(true);
        v
    };
    Ok(ItemAdoptionPublication {
        transaction_id: result.transaction_id,
        manifest_sha256: Some(result.manifest_sha256),
        publication: result.publication,
        receipt: if result.committed {
            Some(receipt)
        } else {
            None
        },
        deposit: foundation_value(&deposited)?,
        replayed: false,
        materializations: if result.committed {
            Some(materializations)
        } else {
            None
        },
    })
}

fn compound_views(
    parent: &BTreeMap<String, Vec<u8>>,
    child: &BTreeMap<String, Vec<u8>>,
    claim_id: &str,
) -> SourceCommandResult<JsonValue> {
    let selected =
        |map: &BTreeMap<String, Vec<u8>>, name: &str| -> SourceCommandResult<JsonValue> {
            cmd::parse(map.get(name).ok_or(SourceCommandError::Conflict(
                "Item materialization buffer absent",
            ))?)
        };
    let edition = selected(parent, "edition.json")?;
    let item = selected(child, "item.json")?;
    let claim = selected(child, "source-claims.jsonl")?;
    Ok(cmd::object(vec![
        (
            "edition",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &edition,
                &selected(parent, "edition.human-forms.json")?,
            )?),
        ),
        (
            "item",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &item,
                &selected(child, "item.human-forms.json")?,
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
pub(crate) fn current_result_fields(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let config = cmd::parse(&ctx.configuration_raw)?;
    current_grant(fs, ctx, &config, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let path = relative(cmd::text(&config, "edition_source_path")?)?;
    let home = path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Item result parent path"))?
        .0;
    let parent = walk(&fs.root, home, fs.uid)?;
    let mut package = BTreeMap::new();
    for name in [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ] {
        if let Some(raw) =
            work_transaction::read_at(&parent, name, fs.uid, 2_097_152, limits.deadline, cancelled)?
        {
            package.insert(name.to_owned(), raw);
        }
    }
    let edition_raw = package
        .get("edition.json")
        .ok_or(SourceCommandError::Conflict("Item result parent absent"))?;
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
            .ok_or(SourceCommandError::Invalid("Item result grant"))?
            .iter()
            .filter(|(k, _)| k.as_str().is_some_and(|k| SCOPE_KEYS.contains(&k)))
            .cloned()
            .collect(),
    );
    let mut profiles = Vec::new();
    let entity = serde_value(&cmd::parse(&checked_current_source(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let relation = serde_value(&cmd::parse(&checked_current_source(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let corpus = serde_value(&cmd::parse(&checked_current_source(
        fs,
        cut,
        "ToS/contracts/corpus-record.schema.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    for kind in ["edition", "item"] {
        let matching = entity["types"]
            .as_array()
            .ok_or(SourceCommandError::Invalid("Item entity registry"))?
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
                "Item descriptor mapping ambiguous",
            ));
        }
        let value = serde_json::json!({"type_id":matching[0]["type_id"],"record_type":kind,"schema_ref":"ToS/contracts/corpus-record.schema.json","schema_version":corpus["properties"]["schema_version"]["const"],"source_basename":format!("{kind}.json")});
        profiles.push((kind, foundation_value(&value)?));
    }
    let matching = relation["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("Item relation registry"))?
        .iter()
        .filter(|entry| {
            entry["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == "exemplified_by"
                        && row["scope"] == "claim-predicate"
                })
            })
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Item relation descriptor ambiguous",
        ));
    }
    let profile = &matching[0]["source_claim_profile"];
    let routes = profile["schemas"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("Item relation schema routes"))?
        .iter()
        .filter(|row| row["schema_version"] == "tos_source_relation_claim_v1")
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Item Claim descriptor ambiguous",
        ));
    }
    profiles.push(("exemplified_by",foundation_value(&serde_json::json!({"relation_type_id":matching[0]["relation_type_id"],"predicate":"exemplified_by","reader":profile["reader"],"schema_ref":routes[0]["schema_ref"],"schema_version":routes[0]["schema_version"],"assertion_layers":profile["assertion_layers"],"source_basename":"source-claims.jsonl"}))?));
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
    current_grant(fs, ctx, &config, limits.deadline, cancelled)?;
    Ok(result)
}
