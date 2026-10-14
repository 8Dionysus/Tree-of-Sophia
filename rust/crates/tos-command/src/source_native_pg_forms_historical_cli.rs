//! Native HumanForms and historical Claim mutations over a bounded PG/STO
//! source read epoch. Owner kernels still prepare every semantic byte.

use super::pg_claims as shared;
use crate::source_cohort::revision::{
    PG_AGENT_INDEX_KINDS, PgRecordRevisionIdentity, SourceRevisionReadSession,
    SourceRevisionReadSet,
};
use crate::source_cohort::{self, IndexRows, PreparedPgSourceRevision, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use crate::source_creation_store::{
    CreationFilesystem, LegacyArchiveReader, LegacyPackage, finish_creation_worker,
};
use crate::{DurableError, DurablePgCoordinator, DurableResult, source_forms};
use postgres::Transaction;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_source_store::SoftwareCaptureReader;

const MAX_CONTEXT_MEMBERS: usize = 4_096;
const MAX_CONTEXT_BYTES: u64 = 33_554_432;
const MAX_CONTEXT_MEMBER_BYTES: u64 = 8_388_608;
const ARCHIVE_PREFIX: &str = "ToS/source-witnesses/.record-revisions/";

struct InvocationBudgets {
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
}

fn budgets(invocation: &Value) -> SourceCommandResult<InvocationBudgets> {
    let value = &invocation["budgets"];
    super::exact(
        value,
        &[
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let max_members = usize::try_from(super::capped(
        value,
        "max_members",
        MAX_CONTEXT_MEMBERS as u64,
    )?)
    .map_err(|_| SourceCommandError::Invalid("PG HumanForms member budget"))?;
    let max_total_bytes = super::capped(value, "max_total_bytes", MAX_CONTEXT_BYTES)?;
    let max_member_bytes = super::capped(value, "max_member_bytes", MAX_CONTEXT_MEMBER_BYTES)?;
    let _ = super::capped(value, "max_schema_receipts", 128)?;
    let _ = super::capped(value, "max_schema_receipt_bytes", 262_144)?;
    let _ = super::capped(value, "worker_cpu_seconds", 3)?;
    let _ = super::capped(value, "worker_address_space_bytes", 1_073_741_824)?;
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid("PG HumanForms empty budget"));
    }
    Ok(InvocationBudgets {
        max_members,
        max_total_bytes,
        max_member_bytes,
    })
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(SourceCommandError::Denied(
            "PG HumanForms cancelled or expired",
        ));
    }
    Ok(())
}

struct PgFormReads<'session, 'db> {
    session: &'session mut SourceRevisionReadSession<'db>,
    members: BTreeMap<String, cmd::SourceFile>,
    directories: BTreeMap<String, Vec<(String, bool)>>,
    total_bytes: u64,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &'session AtomicBool,
}

impl PgFormReads<'_, '_> {
    fn read_flat_directory(&mut self, directory: &str) -> SourceCommandResult<Vec<String>> {
        active(self.deadline, self.cancelled)?;
        let directory = RelativePath::parse(directory)
            .map_err(|_| SourceCommandError::Invalid("PG HumanForms package directory"))?;
        if !self.directories.contains_key(directory.as_str()) {
            let children = self
                .session
                .list_immediate_children(&directory, self.deadline, self.cancelled)
                .map_err(shared::source_error)?;
            self.directories
                .insert(directory.as_str().to_owned(), children);
        }
        let children = self.directories[directory.as_str()].clone();
        if children.iter().any(|(_, is_directory)| *is_directory) {
            return Err(SourceCommandError::Conflict(
                "PG historical Claim package is not flat",
            ));
        }
        let mut selected = Vec::with_capacity(children.len());
        for (name, _) in children {
            let path = format!("{}/{name}", directory.as_str());
            self.read_required(&path)?;
            selected.push(path);
        }
        Ok(selected)
    }

    fn resolve_historical_identities(
        &mut self,
        tokens: &BTreeSet<String>,
    ) -> SourceCommandResult<super::legacy_claim::engine::HistoricalIdentityIndex> {
        let mut entries = BTreeMap::new();
        for token in tokens {
            active(self.deadline, self.cancelled)?;
            let resolution = self
                .session
                .lookup_source_identity(token, self.deadline, self.cancelled)
                .map_err(shared::source_error)?;
            if let Some((kind, path)) = &resolution {
                let location = path.as_str();
                if !location.starts_with("ToS/source-witnesses/")
                    || location.split('/').any(|part| {
                        [
                            "owner-local",
                            "private",
                            "payload",
                            "local-content",
                            ".record-revisions",
                        ]
                        .contains(&part)
                    })
                {
                    return Err(SourceCommandError::Denied(
                        "historical Claim identity source outside public witness namespace",
                    ));
                }
                let basename = location.rsplit('/').next().unwrap_or(location);
                let supported_path = match kind.as_str() {
                    "metadata" => location.ends_with(".json"),
                    "event" => basename.ends_with(".jsonl") && basename.contains("provenance"),
                    "anchor" => basename.ends_with(".jsonl") && basename.contains("anchor"),
                    "form" => location.ends_with(".human-forms.json"),
                    "claim" => true,
                    _ => false,
                };
                if !supported_path {
                    return Err(SourceCommandError::Conflict(
                        "historical Claim identity index path kind differs",
                    ));
                }
                if kind != "claim" {
                    let raw = self.read_required(location)?;
                    if kind == "form"
                        && !form_ids(&raw)
                            .map_err(|_| {
                                SourceCommandError::Conflict(
                                    "historical Claim form identity index source is invalid",
                                )
                            })?
                            .contains(token)
                    {
                        return Err(SourceCommandError::Conflict(
                            "historical Claim form identity index source differs",
                        ));
                    }
                }
            }
            entries.insert(token.clone(), resolution);
        }
        Ok(super::legacy_claim::engine::HistoricalIdentityIndex { entries })
    }

    fn read_optional(&mut self, path: &str) -> SourceCommandResult<Option<Vec<u8>>> {
        active(self.deadline, self.cancelled)?;
        let path = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("PG HumanForms current member path"))?;
        let name = path.as_str().rsplit_once('/').map(|(_, name)| name).ok_or(
            SourceCommandError::Invalid("PG HumanForms current member parent"),
        )?;
        let parent = path
            .as_str()
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(SourceCommandError::Invalid(
                "PG HumanForms current member parent",
            ))?;
        if let Some(member) = self.members.get(path.as_str()) {
            return Ok(Some(member.raw.clone()));
        }
        if !self.directories.contains_key(parent) {
            let parent_path = RelativePath::parse(parent)
                .map_err(|_| SourceCommandError::Invalid("PG HumanForms current directory"))?;
            let children = self
                .session
                .list_immediate_children(&parent_path, self.deadline, self.cancelled)
                .map_err(shared::source_error)?;
            self.directories.insert(parent.to_owned(), children);
        }
        if !self.directories[parent]
            .iter()
            .any(|(child, is_directory)| child == name && !*is_directory)
        {
            return Ok(None);
        }
        if self.members.len() >= self.max_members {
            return Err(SourceCommandError::Invalid(
                "PG HumanForms selected member count",
            ));
        }
        let member = self
            .session
            .read_current_member(&path, self.max_member_bytes, self.deadline, self.cancelled)
            .map_err(shared::source_error)?;
        let size = member.raw.len() as u64;
        self.total_bytes = self
            .total_bytes
            .checked_add(size)
            .filter(|total| *total <= self.max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "PG HumanForms selected byte budget",
            ))?;
        let raw = member.raw;
        self.members.insert(
            path.as_str().to_owned(),
            cmd::SourceFile {
                path,
                raw: raw.clone(),
            },
        );
        Ok(Some(raw))
    }

    fn read_required(&mut self, path: &str) -> SourceCommandResult<Vec<u8>> {
        self.read_optional(path)?
            .ok_or(SourceCommandError::Conflict(
                "PG HumanForms selected source member absent",
            ))
    }

    fn files(&self) -> Vec<cmd::SourceFile> {
        self.members.values().cloned().collect()
    }

    fn directories(&self) -> BTreeSet<String> {
        self.directories.keys().cloned().collect()
    }
}

fn read_schema_closure(
    reads: &mut PgFormReads<'_, '_>,
    roots: impl IntoIterator<Item = String>,
) -> SourceCommandResult<()> {
    let mut pending = roots.into_iter().collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        if visited.len() > 128 {
            return Err(SourceCommandError::Invalid(
                "PG HumanForms schema resource count",
            ));
        }
        let raw = reads.read_required(&path)?;
        if raw.len() > 4 * 1_048_576 {
            return Err(SourceCommandError::Invalid(
                "PG HumanForms schema member bytes",
            ));
        }
        crate::source_revisions::collect_schema_refs(&cmd::parse(&raw)?, &path, &mut pending, 0)?;
    }
    Ok(())
}

fn parse_source_path(config: &tos_foundation::JsonValue) -> SourceCommandResult<RelativePath> {
    let path = cmd::text(config, "source_path")?;
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PG HumanForms source path"))?;
    if !path.as_str().starts_with("ToS/") {
        return Err(SourceCommandError::Denied(
            "PG HumanForms source path outside ToS",
        ));
    }
    Ok(path)
}

fn package_at_home(members: &[cmd::SourceFile], home: &str) -> SourceCommandResult<LegacyPackage> {
    let prefix = format!("{home}/");
    let mut package = LegacyPackage::new();
    let mut total = 0usize;
    for member in members
        .iter()
        .filter(|member| member.path.as_str().starts_with(&prefix))
    {
        let name = &member.path.as_str()[prefix.len()..];
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(SourceCommandError::Conflict(
                "PG historical Claim home is not a flat selected package",
            ));
        }
        if member.raw.len() > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "PG historical Claim package member budget",
            ));
        }
        total = total
            .checked_add(member.raw.len())
            .filter(|bytes| *bytes <= 8_388_608)
            .ok_or(SourceCommandError::Invalid(
                "PG historical Claim package budget",
            ))?;
        if package
            .insert(name.to_owned(), member.raw.clone())
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate PG historical Claim package member",
            ));
        }
    }
    if package.is_empty() || package.len() > 64 {
        return Err(SourceCommandError::Conflict(
            "PG historical Claim package absent or oversized",
        ));
    }
    Ok(package)
}

struct PgArchiveReader<'a> {
    members: &'a [cmd::SourceFile],
}

impl LegacyArchiveReader for PgArchiveReader<'_> {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        active(deadline, cancelled)?;
        let suffix = reference
            .strip_prefix(ARCHIVE_PREFIX)
            .filter(|suffix| {
                !suffix.is_empty()
                    && !suffix.contains('/')
                    && !suffix.starts_with('.')
                    && !suffix.contains('\\')
            })
            .ok_or(SourceCommandError::Invalid(
                "PG historical Claim archive reference",
            ))?;
        let _ = suffix;
        let prefix = format!("{reference}/");
        let mut package = LegacyPackage::new();
        let mut total = 0usize;
        for member in self
            .members
            .iter()
            .filter(|member| member.path.as_str().starts_with(&prefix))
        {
            active(deadline, cancelled)?;
            let name = &member.path.as_str()[prefix.len()..];
            if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
                return Err(SourceCommandError::Conflict(
                    "PG historical Claim archive is not flat",
                ));
            }
            if member.raw.len() > 8_388_608 {
                return Err(SourceCommandError::Invalid(
                    "PG historical Claim archive member budget",
                ));
            }
            total = total
                .checked_add(member.raw.len())
                .filter(|bytes| *bytes <= 16_777_216)
                .ok_or(SourceCommandError::Invalid(
                    "PG historical Claim archive byte budget",
                ))?;
            if package
                .insert(name.to_owned(), member.raw.clone())
                .is_some()
            {
                return Err(SourceCommandError::Invalid(
                    "duplicate PG historical Claim archive member",
                ));
            }
            if package.len() > 65 {
                return Err(SourceCommandError::Invalid(
                    "PG historical Claim archive member count",
                ));
            }
        }
        if package.is_empty() {
            return Err(SourceCommandError::Conflict(
                "PG historical Claim retained archive absent",
            ));
        }
        Ok(package)
    }
}

fn package_changes(
    context: &CommandContext,
    home: &str,
    before: &LegacyPackage,
    after: &LegacyPackage,
    archive: Option<(&str, &LegacyPackage)>,
) -> SourceCommandResult<Vec<cmd::SourceChange>> {
    let mut changes = Vec::new();
    let names = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in names {
        let old = before.get(&name);
        let new = after.get(&name);
        if old == new {
            continue;
        }
        let path = RelativePath::parse(&format!("{home}/{name}"))
            .map_err(|_| SourceCommandError::Invalid("PG historical Claim package path"))?;
        let selected = context.file(&path)?;
        if selected != old.map(Vec::as_slice) {
            return Err(SourceCommandError::Conflict(
                "PG historical Claim package differs from current cut",
            ));
        }
        if let Some(new) = new {
            changes.push(cmd::SourceChange {
                path,
                before: old.map(|raw| Digest256::of_bytes(raw)),
                after: Some(new.clone()),
            });
        } else {
            return Err(SourceCommandError::Conflict(
                "PG historical Claim owner attempted package deletion",
            ));
        }
    }
    if let Some((reference, files)) = archive {
        if !reference.starts_with(ARCHIVE_PREFIX) {
            return Err(SourceCommandError::Denied(
                "PG historical Claim archive escaped retained namespace",
            ));
        }
        for (name, raw) in files {
            if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
                return Err(SourceCommandError::Invalid(
                    "PG historical Claim archive package name",
                ));
            }
            let path = RelativePath::parse(&format!("{reference}/{name}"))
                .map_err(|_| SourceCommandError::Invalid("PG historical Claim archive path"))?;
            if context.file(&path)?.is_some() {
                return Err(SourceCommandError::Conflict(
                    "PG historical Claim archive path already exists",
                ));
            }
            changes.push(cmd::SourceChange {
                path,
                before: None,
                after: Some(raw.clone()),
            });
        }
    }
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    if changes.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(SourceCommandError::Invalid(
            "duplicate PG historical Claim package delta",
        ));
    }
    Ok(changes)
}

fn form_ids(raw: &[u8]) -> DurableResult<BTreeSet<String>> {
    let value = cmd::parse(raw).map_err(DurableError::Source)?;
    source_forms::projection_form_ids(&value)
        .map_err(DurableError::Source)?
        .into_iter()
        .map(|id| {
            id.as_str()
                .map(str::to_owned)
                .ok_or(DurableError::Corrupt("PG Forms form identity is malformed"))
        })
        .collect()
}

pub(crate) fn pg_rederive_revision_indexes(
    context: &CommandContext,
    writes: &[crate::source_cohort::SourceRevisionWrite],
) -> DurableResult<BTreeSet<(String, String, String)>> {
    let mut indexes = BTreeSet::new();
    for write in writes {
        let path = RelativePath::parse(&write.subject)
            .map_err(|_| DurableError::Corrupt("PG Forms/Historical recovery path"))?;
        let predecessor = context.file(&path).map_err(DurableError::Source)?;
        match (write.expected_predecessor, predecessor) {
            (Some((_, digest)), Some(raw)) if Digest256::of_bytes(raw) == digest => {}
            (None, None) => {}
            _ => {
                return Err(DurableError::Conflict(
                    "PG Forms/Historical recovery predecessor differs from retained readset",
                ));
            }
        }
        if write.expected_predecessor.is_none() {
            indexes.insert((
                "path".to_owned(),
                write.subject.clone(),
                write.subject.clone(),
            ));
        }
        let is_live_form = write.subject.ends_with(".human-forms.json")
            && !write
                .subject
                .starts_with("ToS/source-witnesses/.record-revisions/");
        if is_live_form {
            let successor_ids = form_ids(&write.exact_bytes)?;
            let predecessor_ids = predecessor.map(form_ids).transpose()?.unwrap_or_default();
            for id in successor_ids.difference(&predecessor_ids) {
                indexes.insert(("form".to_owned(), id.clone(), write.subject.clone()));
            }
        }
    }
    Ok(indexes)
}

fn expected_live_form_path(
    config: &tos_foundation::JsonValue,
    schema: &str,
) -> SourceCommandResult<String> {
    let source = cmd::text(config, "source_path")?;
    if matches!(
        schema,
        "tos_local_claim_form_owner_v1"
            | "tos_local_claim_form_owner_v2"
            | "tos_local_historical_claim_form_owner_v1"
            | "tos_local_historical_claim_revision_owner_v1"
    ) {
        let stem = source
            .strip_suffix(".jsonl")
            .ok_or(SourceCommandError::Invalid("PG Claim form source suffix"))?
            .rsplit_once('/')
            .map(|(_, stem)| stem)
            .ok_or(SourceCommandError::Invalid("PG Claim form source parent"))?;
        let id = cmd::text(config, "claim_id")?;
        let parent = source
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(SourceCommandError::Invalid("PG Claim form source parent"))?;
        Ok(format!(
            "{parent}/{stem}.{}.human-forms.json",
            Digest256::of_bytes(id.as_bytes()).to_hex()
        ))
    } else {
        let stem = source
            .strip_suffix(".json")
            .ok_or(SourceCommandError::Invalid("PG HumanForms source suffix"))?;
        Ok(format!("{stem}.human-forms.json"))
    }
}

fn expected_family_tag(context: &CommandContext, operation: &str) -> DurableResult<String> {
    let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
    let schema = cmd::text(&config, "schema_version").map_err(DurableError::Source)?;
    let valid = match schema {
        "tos_local_source_command_owner_v1"
        | "tos_local_canonical_form_owner_v1"
        | "tos_local_claim_form_owner_v1"
        | "tos_local_claim_form_owner_v2" => operation == "apply",
        "tos_local_historical_claim_form_owner_v1" => operation == "apply",
        "tos_local_historical_claim_revision_owner_v1" => operation == "claim.revise",
        _ => false,
    };
    if !valid {
        return Err(DurableError::Refused(
            "PG HumanForms or historical Claim mutation family differs",
        ));
    }
    let (family, suffix) = if schema.starts_with("tos_local_historical_claim_") {
        ("tos-pg-historical-claim-v1", operation)
    } else {
        ("tos-pg-human-forms-v1", operation)
    };
    Ok(format!("{family}:{schema}:{suffix}"))
}

fn exact_human_form_context(
    session: &mut SourceRevisionReadSession<'_>,
    config: &tos_foundation::JsonValue,
    owner_schema: &str,
    request_raw: &[u8],
    owner_raw: &[u8],
    recorded_at: String,
    uid: u32,
    software_files: Vec<cmd::SourceFile>,
    software_members: usize,
    software_bytes: u64,
    budgets: &InvocationBudgets,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(CommandContext, Vec<cmd::SourceFile>, PreparedReadsetBinding)> {
    let identity = session.selection().identity();
    let base_revision = session.selection().cohort.initial_revision();
    let max_members = budgets
        .max_members
        .checked_sub(software_members)
        .filter(|members| *members > 0)
        .ok_or(SourceCommandError::Invalid(
            "PG HumanForms exact member budget",
        ))?;
    let max_total_bytes = budgets
        .max_total_bytes
        .checked_sub(software_bytes)
        .filter(|bytes| *bytes > 0)
        .ok_or(SourceCommandError::Invalid(
            "PG HumanForms exact byte budget",
        ))?;
    let mut reads = PgFormReads {
        session,
        members: BTreeMap::new(),
        directories: BTreeMap::new(),
        total_bytes: 0,
        max_members,
        max_total_bytes,
        max_member_bytes: budgets.max_member_bytes,
        deadline,
        cancelled,
    };
    let source_path = parse_source_path(config)?;
    let source_raw = reads.read_required(source_path.as_str())?;
    let claim_owner = matches!(
        owner_schema,
        "tos_local_claim_form_owner_v1" | "tos_local_claim_form_owner_v2"
    );
    let source = if claim_owner {
        source_forms::select_form_claim(&source_raw, cmd::text(config, "claim_id")?)?
    } else {
        cmd::parse(&source_raw)?
    };
    let version = cmd::text(&source, "schema_version")?;
    let mut schema_roots = BTreeSet::from(["ToS/contracts/human-form-set.schema.json".to_owned()]);
    if let Some(path) = source_forms::selected_source_contract(owner_schema, version)? {
        schema_roots.insert(path.to_owned());
    }
    if claim_owner {
        let claim_raw = cmd::canonical(&source)?;
        let relation_registry =
            reads.read_required("ToS/doctrine/semantic-interchange/relation-types.v1.json")?;
        let initial = [(
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            relation_registry.as_slice(),
        )];
        let limits = tos_validation::item_rules::ItemLimits {
            max_member_bytes: usize::try_from(budgets.max_member_bytes)
                .map_err(|_| SourceCommandError::Invalid("PG HumanForms Claim member limit"))?,
            max_total_bytes,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline,
        };
        let profile_paths = tos_validation::record_rules::local_claim_form_dependency_paths(
            &claim_raw, &initial, limits, cancelled,
        )
        .map_err(|_| SourceCommandError::Invalid("PG HumanForms Claim dependency planning"))?;
        for path in profile_paths {
            let _ = reads.read_required(&path)?;
            if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
                schema_roots.insert(path);
            }
        }
    }
    let form_path = expected_live_form_path(config, owner_schema)?;
    let _ = reads.read_optional(&form_path)?;
    read_schema_closure(&mut reads, schema_roots)?;
    let authored_members = reads.files();
    let mut files = authored_members.clone();
    files.extend(software_files);
    let context = CommandContext {
        base_revision,
        configuration_raw: owner_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at,
        effective_uid: u64::from(uid),
        files,
    };
    shared::check_context_budget(
        &context,
        budgets.max_members,
        budgets.max_total_bytes,
        budgets.max_member_bytes,
    )?;
    let binding = PreparedReadsetBinding::Exact {
        identity,
        directories: reads.directories().into_iter().collect(),
        identity_tokens: BTreeSet::new(),
    };
    Ok((context, authored_members, binding))
}

fn historical_metadata_schema_roots(
    entity_registry_raw: &[u8],
    metadata_members: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<BTreeSet<String>> {
    let registry: Value = serde_json::from_slice(entity_registry_raw)
        .map_err(|_| SourceCommandError::Invalid("PG historical Claim entity registry JSON"))?;
    let types =
        registry
            .get("types")
            .and_then(Value::as_array)
            .ok_or(SourceCommandError::Invalid(
                "PG historical Claim entity registry types",
            ))?;
    let mut roots = BTreeSet::from(["ToS/contracts/corpus-record.schema.json".to_owned()]);
    for (path, raw) in metadata_members {
        let record: Value = serde_json::from_slice(raw)
            .map_err(|_| SourceCommandError::Invalid("PG historical endpoint JSON"))?;
        let schema_version = record.get("schema_version").and_then(Value::as_str).ok_or(
            SourceCommandError::Invalid("PG historical endpoint schema version"),
        )?;
        let basename = path.rsplit('/').next().unwrap_or(path);
        let fixed = match (basename, schema_version) {
            ("artifact-witness.json", "tos_artifact_source_witness_v1") => {
                Some("ToS/contracts/artifact-source-witness.schema.json")
            }
            ("artifact-witness.json", "tos_artifact_source_witness_v2") => {
                Some("ToS/contracts/artifact-source-witness-v2.schema.json")
            }
            ("composite-witness.json", "tos_scholarly_composite_witness_v1") => {
                Some("ToS/contracts/scholarly-composite-witness.schema.json")
            }
            _ => None,
        };
        if let Some(schema) = fixed {
            roots.insert(schema.to_owned());
            continue;
        }
        let record_type = record.get("record_type").and_then(Value::as_str).ok_or(
            SourceCommandError::Invalid("PG historical endpoint record type"),
        )?;
        let descriptor = types
            .iter()
            .filter_map(|item| item.get("source_record_profile"))
            .find(|profile| profile.get("record_type").and_then(Value::as_str) == Some(record_type))
            .ok_or(SourceCommandError::Unsupported(
                "PG historical endpoint source record profile",
            ))?;
        let routes = descriptor.get("schemas").and_then(Value::as_array).ok_or(
            SourceCommandError::Invalid("PG historical endpoint schema routes"),
        )?;
        let route = routes
            .iter()
            .find(|route| {
                route.get("schema_version").and_then(Value::as_str) == Some(schema_version)
            })
            .ok_or(SourceCommandError::Unsupported(
                "PG historical endpoint schema route",
            ))?;
        roots.insert(
            route
                .get("schema_ref")
                .and_then(Value::as_str)
                .ok_or(SourceCommandError::Invalid(
                    "PG historical endpoint schema reference",
                ))?
                .to_owned(),
        );
        for dependency in route
            .get("schema_dependencies")
            .and_then(Value::as_array)
            .ok_or(SourceCommandError::Invalid(
                "PG historical endpoint schema dependencies",
            ))?
        {
            roots.insert(
                dependency
                    .as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "PG historical endpoint schema dependency",
                    ))?
                    .to_owned(),
            );
        }
    }
    Ok(roots)
}

fn exact_historical_context(
    session: &mut SourceRevisionReadSession<'_>,
    config: &tos_foundation::JsonValue,
    owner_raw: &[u8],
    request_raw: &[u8],
    recorded_at: String,
    uid: u32,
    software_files: Vec<cmd::SourceFile>,
    software_members: usize,
    software_bytes: u64,
    budgets: &InvocationBudgets,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    CommandContext,
    Vec<cmd::SourceFile>,
    PreparedReadsetBinding,
    LegacyPackage,
    super::legacy_claim::engine::HistoricalIdentityIndex,
)> {
    let identity = session.selection().identity();
    let base_revision = session.selection().cohort.initial_revision();
    let max_members = budgets
        .max_members
        .checked_sub(software_members)
        .filter(|members| *members > 0)
        .ok_or(SourceCommandError::Invalid(
            "PG historical Claim exact member budget",
        ))?;
    let max_total_bytes = budgets
        .max_total_bytes
        .checked_sub(software_bytes)
        .filter(|bytes| *bytes > 0)
        .ok_or(SourceCommandError::Invalid(
            "PG historical Claim exact byte budget",
        ))?;
    let mut reads = PgFormReads {
        session,
        members: BTreeMap::new(),
        directories: BTreeMap::new(),
        total_bytes: 0,
        max_members,
        max_total_bytes,
        max_member_bytes: budgets.max_member_bytes,
        deadline,
        cancelled,
    };
    let source_path = parse_source_path(config)?;
    let home = source_path
        .as_str()
        .strip_suffix("/historical-claims.jsonl")
        .ok_or(SourceCommandError::Invalid(
            "PG historical Claim source home",
        ))?;
    let _ = reads.read_flat_directory(home)?;
    if reads.read_optional(source_path.as_str())?.is_none() {
        return Err(SourceCommandError::Conflict(
            "PG historical Claim source stream absent",
        ));
    }
    let current = package_at_home(&reads.files(), home)?;
    let config_owner = cmd::parse(owner_raw)?;
    let operation = cmd::text(&cmd::parse(request_raw)?, "operation")?.to_owned();
    let all_required_tokens =
        super::legacy_claim::engine::required_identity_tokens(owner_raw, &current, &operation)?;
    let identity_tokens = all_required_tokens
        .iter()
        .filter(|token| !token.starts_with("ToS/"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let identity_index = reads.resolve_historical_identities(&identity_tokens)?;

    let archive_refs = super::legacy_claim::engine::required_archives(&current, &config_owner)?;
    for reference in archive_refs {
        let _ = reads.read_flat_directory(&reference)?;
    }

    let mut schema_roots = super::legacy_claim::engine::pg_required_contract_paths()
        .into_iter()
        .filter(|path| path.ends_with(".schema.json"))
        .collect::<BTreeSet<_>>();
    let entity_registry_path = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
    let _ = reads.read_required(entity_registry_path)?;
    let relation_registry_path = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
    let _ = reads.read_required(relation_registry_path)?;
    for path in super::legacy_claim::engine::pg_required_contract_paths() {
        let _ = reads.read_required(&path)?;
    }
    schema_roots.insert("ToS/contracts/semantic-entity-type-registry.schema.json".to_owned());
    schema_roots.insert("ToS/contracts/semantic-relation-type-registry.schema.json".to_owned());

    let mut metadata_members = BTreeMap::new();
    for (_, resolution) in &identity_index.entries {
        let Some((kind, path)) = resolution else {
            continue;
        };
        if kind == "metadata" {
            let raw = reads.read_required(path.as_str())?;
            metadata_members.insert(path.as_str().to_owned(), raw);
        }
    }
    let entity_registry_raw = reads.read_required(entity_registry_path)?;
    schema_roots.extend(historical_metadata_schema_roots(
        &entity_registry_raw,
        &metadata_members,
    )?);
    for reference in all_required_tokens {
        if reference.starts_with("ToS/") {
            let parsed = RelativePath::parse(&reference)
                .map_err(|_| SourceCommandError::Invalid("PG historical evidence path"))?;
            let path = parsed.as_str();
            if path
                .split('/')
                .any(|part| ["payload", "local-content", "owner-local", "private"].contains(&part))
            {
                return Err(SourceCommandError::Denied(
                    "historical Claim evidence addresses private metadata",
                ));
            }
            let _ = reads.read_required(path)?;
        }
    }
    read_schema_closure(&mut reads, schema_roots)?;

    let authored_members = reads.files();
    let mut files = authored_members.clone();
    files.extend(software_files);
    let context = CommandContext {
        base_revision,
        configuration_raw: owner_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at,
        effective_uid: u64::from(uid),
        files,
    };
    shared::check_context_budget(
        &context,
        budgets.max_members,
        budgets.max_total_bytes,
        budgets.max_member_bytes,
    )?;
    let binding = PreparedReadsetBinding::Exact {
        identity,
        directories: reads.directories().into_iter().collect(),
        identity_tokens,
    };
    Ok((context, authored_members, binding, current, identity_index))
}

/// Typed owner preparation for exact current form changes and historical
/// Claim package successors. The source owner remains the only byte producer.
pub(crate) struct PreparedPgFormsHistoricalCommand {
    context: CommandContext,
    proposal: PreparedCommand,
    readset_binding: PreparedReadsetBinding,
    family_tag: String,
    indexes: IndexRows,
    inserted_form_ids: BTreeSet<String>,
    owner_contributions: BTreeMap<String, tos_foundation::JsonValue>,
}

enum PreparedReadsetBinding {
    Exact {
        identity: PgRecordRevisionIdentity,
        directories: BTreeSet<String>,
        identity_tokens: BTreeSet<String>,
    },
}

impl PreparedPgFormsHistoricalCommand {
    fn new(
        context: CommandContext,
        proposal: PreparedCommand,
        readset_binding: PreparedReadsetBinding,
    ) -> DurableResult<Self> {
        let request = cmd::parse(&context.request_raw).map_err(DurableError::Source)?;
        let operation = cmd::text(&request, "operation").map_err(DurableError::Source)?;
        let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
        let schema = cmd::text(&config, "schema_version").map_err(DurableError::Source)?;
        let family_tag = expected_family_tag(&context, operation)?;
        if proposal.base_revision != context.base_revision
            || proposal.operation != operation
            || proposal.replayed && !proposal.changes.is_empty()
            || (!proposal.replayed && !["apply", "claim.revise"].contains(&operation))
        {
            return Err(DurableError::Conflict(
                "PG HumanForms proposal operation or source differs",
            ));
        }
        let source_path = parse_source_path(&config).map_err(DurableError::Source)?;
        let source_path = source_path.as_str();
        let historical_home = if schema.starts_with("tos_local_historical_claim_") {
            Some(
                source_path
                    .strip_suffix("/historical-claims.jsonl")
                    .ok_or(DurableError::Invalid("PG historical Claim source home"))?,
            )
        } else {
            None
        };
        let allowed_live_form = if schema == "tos_local_historical_claim_revision_owner_v1" {
            None
        } else {
            Some(expected_live_form_path(&config, schema).map_err(DurableError::Source)?)
        };
        let expected_historical_live_form = if schema.starts_with("tos_local_historical_claim_") {
            Some(expected_live_form_path(&config, schema).map_err(DurableError::Source)?)
        } else {
            None
        };
        let mut changes = BTreeMap::new();
        for change in &proposal.changes {
            let path = change.path.as_str();
            let allowed = if let Some(home) = historical_home {
                let expected_form = expected_historical_live_form.as_deref();
                let in_home = path == source_path
                    || path == format!("{home}/claim-revision-history.json")
                    || expected_form == Some(path);
                in_home
                    || (operation == "claim.revise"
                        && path.starts_with(ARCHIVE_PREFIX)
                        && path[ARCHIVE_PREFIX.len()..].contains('/'))
            } else {
                allowed_live_form.as_deref() == Some(path)
            };
            if !allowed || change.after.is_none() {
                return Err(DurableError::Refused(
                    "PG HumanForms write escaped exact owner closure",
                ));
            }
            if changes.insert(path.to_owned(), change).is_some() {
                return Err(DurableError::Invalid("duplicate PG HumanForms write path"));
            }
        }
        if !proposal.replayed {
            match operation {
                "apply" => {
                    let expected = allowed_live_form.as_deref().ok_or(DurableError::Invalid(
                        "PG HumanForms adjacent target absent",
                    ))?;
                    if changes.len() != 1 || !changes.contains_key(expected) {
                        return Err(DurableError::Conflict(
                            "PG HumanForms proposal is not one adjacent form set",
                        ));
                    }
                }
                "claim.revise" => {
                    let expected_archive = expected_historical_archive(&context, &config)?;
                    let archive_prefix = format!("{expected_archive}/");
                    let history_path = format!(
                        "{}/claim-revision-history.json",
                        historical_home.unwrap_or("")
                    );
                    let authored = context
                        .files
                        .iter()
                        .filter(|file| file.path.as_str().starts_with("ToS/"))
                        .cloned()
                        .collect::<Vec<_>>();
                    let current = package_at_home(
                        &authored,
                        historical_home
                            .ok_or(DurableError::Invalid("PG historical Claim home absent"))?,
                    )
                    .map_err(DurableError::Source)?;
                    let expected_archive_paths = current
                        .values()
                        .map(|raw| format!("{}.blob", Digest256::of_bytes(raw).to_hex()))
                        .chain(std::iter::once("manifest.json".to_owned()))
                        .collect::<BTreeSet<_>>();
                    let actual_archive_paths = changes
                        .keys()
                        .filter_map(|path| path.strip_prefix(&archive_prefix).map(str::to_owned))
                        .collect::<BTreeSet<_>>();
                    if !changes.contains_key(source_path)
                        || !changes.contains_key(&history_path)
                        || changes.keys().any(|path| {
                            path.starts_with(ARCHIVE_PREFIX) && !path.starts_with(&archive_prefix)
                        })
                        || actual_archive_paths != expected_archive_paths
                    {
                        // The exact package writer below validates optional form-set members.
                        return Err(DurableError::Conflict(
                            "PG historical Claim successor closure differs",
                        ));
                    }
                }
                _ => {
                    return Err(DurableError::Refused(
                        "PG HumanForms mutation operation absent",
                    ));
                }
            }
        }
        let mut indexes = IndexRows::new();
        let mut inserted_form_ids = BTreeSet::new();
        for change in &proposal.changes {
            if change.before.is_none() {
                indexes.insert((
                    "path".into(),
                    change.path.as_str().into(),
                    change.path.as_str().into(),
                ));
            }
            if !change.path.as_str().starts_with(ARCHIVE_PREFIX)
                && change.path.as_str().ends_with(".human-forms.json")
            {
                let after = change.after.as_ref().ok_or(DurableError::Invalid(
                    "PG HumanForms form set successor absent",
                ))?;
                let successor_ids = form_ids(after)?;
                let predecessor_raw = context.file(&change.path).map_err(DurableError::Source)?;
                if change.before.is_some() != predecessor_raw.is_some() {
                    return Err(DurableError::Conflict(
                        "PG HumanForms predecessor presence differs from selected source",
                    ));
                }
                let predecessor_ids = match predecessor_raw {
                    Some(raw) => form_ids(raw)?,
                    None => BTreeSet::new(),
                };
                for id in successor_ids {
                    if !predecessor_ids.contains(&id) && !inserted_form_ids.insert(id.clone()) {
                        return Err(DurableError::Conflict(
                            "PG HumanForms form identity repeats in successor package",
                        ));
                    }
                    if !predecessor_ids.contains(&id) {
                        indexes.insert(("form".into(), id, change.path.as_str().into()));
                    }
                }
            }
        }
        let command_id = cmd::text(&request, "command_id").map_err(DurableError::Source)?;
        let request_canonical_digest =
            cmd::record_digest(&request).map_err(DurableError::Source)?;
        let owner_contributions = pg_owner_contributions_for_attempt(
            &context,
            &proposal,
            schema,
            operation,
            command_id,
            request_canonical_digest,
            Digest256::of_bytes(&context.configuration_raw),
            proposal.configuration_canonical_sha256,
            &family_tag,
        )?;
        Ok(Self {
            context,
            proposal,
            readset_binding,
            family_tag,
            indexes,
            inserted_form_ids,
            owner_contributions,
        })
    }
}

pub(crate) fn pg_owner_contributions_for_attempt(
    context: &CommandContext,
    proposal: &PreparedCommand,
    owner_schema: &str,
    operation: &str,
    command_id: &str,
    request_canonical_digest: Digest256,
    configuration_raw_digest: Digest256,
    configuration_canonical_digest: Digest256,
    family_tag: &str,
) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
    let (tag_schema, tag_operation, handler) = match family_tag {
        "tos-pg-human-forms-v1:tos_local_source_command_owner_v1:apply" => (
            "tos_local_source_command_owner_v1",
            "apply",
            "public-source-forms",
        ),
        "tos-pg-human-forms-v1:tos_local_canonical_form_owner_v1:apply" => (
            "tos_local_canonical_form_owner_v1",
            "apply",
            "canonical-node-forms",
        ),
        "tos-pg-human-forms-v1:tos_local_claim_form_owner_v1:apply" => (
            "tos_local_claim_form_owner_v1",
            "apply",
            "public-source-forms",
        ),
        "tos-pg-human-forms-v1:tos_local_claim_form_owner_v2:apply" => (
            "tos_local_claim_form_owner_v2",
            "apply",
            "public-source-forms",
        ),
        "tos-pg-historical-claim-v1:tos_local_historical_claim_form_owner_v1:apply" => (
            "tos_local_historical_claim_form_owner_v1",
            "apply",
            "legacy-historical-claim-forms",
        ),
        "tos-pg-historical-claim-v1:tos_local_historical_claim_revision_owner_v1:claim.revise" => (
            "tos_local_historical_claim_revision_owner_v1",
            "claim.revise",
            "legacy-historical-claim-revision",
        ),
        _ => {
            return Err(DurableError::Refused(
                "PG HumanForms recovery family tag is unsupported",
            ));
        }
    };
    let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
    if owner_schema != tag_schema
        || operation != tag_operation
        || cmd::text(&config, "schema_version").map_err(DurableError::Source)? != tag_schema
        || proposal.operation != operation
        || proposal.handler_id != handler
        || proposal.base_revision != context.base_revision
        || proposal.request_canonical_sha256 != request_canonical_digest
        || Digest256::of_bytes(&context.configuration_raw) != configuration_raw_digest
        || proposal.configuration_raw_sha256 != configuration_raw_digest
        || cmd::record_digest(&config).map_err(DurableError::Source)?
            != configuration_canonical_digest
        || proposal.configuration_canonical_sha256 != configuration_canonical_digest
        || !(1..=256).contains(&command_id.chars().count())
    {
        return Err(DurableError::Conflict(
            "PG HumanForms recovery owner binding differs",
        ));
    }
    if proposal.replayed || proposal.changes.is_empty() {
        if !proposal.changes.is_empty() {
            return Err(DurableError::Conflict(
                "PG HumanForms replay contains source writes",
            ));
        }
        return Ok(BTreeMap::new());
    }
    let historical_claim_owner = if operation == "claim.revise" {
        Some(
            super::legacy_claim::engine::pg_revision_owner_contribution_for_attempt(
                context,
                proposal,
                command_id,
                request_canonical_digest,
                configuration_raw_digest,
                configuration_canonical_digest,
                family_tag,
            )
            .map_err(DurableError::Source)?,
        )
    } else {
        None
    };
    pg_owner_contributions_with_historical_owner(
        context,
        proposal,
        owner_schema,
        operation,
        historical_claim_owner,
    )
}

fn pg_owner_contributions_with_historical_owner(
    context: &CommandContext,
    proposal: &PreparedCommand,
    owner_schema: &str,
    operation: &str,
    historical_claim_owner: Option<tos_foundation::JsonValue>,
) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
    if proposal.changes.is_empty() {
        return Ok(BTreeMap::new());
    }
    let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
    let source_path = cmd::text(&config, "source_path").map_err(DurableError::Source)?;
    let source_path = source_path.to_owned();
    let source_raw = proposal
        .changes
        .iter()
        .find(|change| change.path.as_str() == source_path)
        .and_then(|change| change.after.as_deref())
        .or_else(|| {
            context
                .files
                .iter()
                .find(|file| file.path.as_str() == source_path)
                .map(|file| file.raw.as_slice())
        })
        .ok_or(DurableError::Conflict(
            "PG HumanForms owner contribution source is absent",
        ))?;
    let source = match owner_schema {
        "tos_local_claim_form_owner_v1" | "tos_local_claim_form_owner_v2" => {
            let claim_id = cmd::text(&config, "claim_id").map_err(DurableError::Source)?;
            source_forms::select_form_claim(source_raw, claim_id).map_err(DurableError::Source)?
        }
        "tos_local_historical_claim_form_owner_v1"
        | "tos_local_historical_claim_revision_owner_v1" => {
            let claim_id = cmd::text(&config, "claim_id").map_err(DurableError::Source)?;
            super::legacy_claim::engine::pg_selected_claim(source_raw, claim_id)
                .map_err(DurableError::Source)?
        }
        _ => cmd::parse(source_raw).map_err(DurableError::Source)?,
    };
    let source_profiles = pg_source_profiles(context)?;
    let mut contributions = BTreeMap::new();
    for change in &proposal.changes {
        let path = change.path.as_str();
        let raw = change.after.as_deref().ok_or(DurableError::Conflict(
            "PG HumanForms contribution contains a deletion",
        ))?;
        let contribution = if path == source_path {
            let owner = historical_claim_owner
                .clone()
                .ok_or(DurableError::Conflict(
                    "PG HumanForms source stream contribution is unsupported",
                ))?;
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_pg_historical_claim_projection_contribution_v1"),
                ),
                ("claim_owner", owner),
                ("source_profiles", source_profiles.clone()),
                ("events", cmd::object(Vec::new())),
                ("anchors", cmd::object(Vec::new())),
            ])
        } else if path.ends_with(".human-forms.json") {
            let form = source_forms::pg_form_projection_contribution(&source, source_raw, raw)
                .map_err(DurableError::Source)?;
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_pg_human_form_projection_contribution_v1"),
                ),
                ("form", form),
                ("source_profiles", source_profiles.clone()),
                ("events", cmd::object(Vec::new())),
                ("anchors", cmd::object(Vec::new())),
            ])
        } else {
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_pg_historical_package_member_contribution_v1"),
                ),
                ("path", cmd::string(path)),
                (
                    "raw_sha256",
                    cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                ),
                ("source_profiles", source_profiles.clone()),
            ])
        };
        if contributions
            .insert(path.to_owned(), contribution)
            .is_some()
        {
            return Err(DurableError::Conflict(
                "duplicate PG HumanForms owner contribution path",
            ));
        }
    }
    Ok(contributions)
}

fn pg_source_profiles(context: &CommandContext) -> DurableResult<tos_foundation::JsonValue> {
    let mut profiles = cmd::object(Vec::new());
    for file in &context.files {
        let path = file.path.as_str();
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            cmd::set(
                &mut profiles,
                path,
                cmd::string(&Digest256::of_bytes(&file.raw).to_hex()),
            )
            .map_err(DurableError::Source)?;
        }
    }
    Ok(profiles)
}

fn expected_historical_archive(
    context: &CommandContext,
    config: &tos_foundation::JsonValue,
) -> DurableResult<String> {
    let id = cmd::text(config, "claim_id").map_err(DurableError::Source)?;
    let source = cmd::text(config, "source_path").map_err(DurableError::Source)?;
    let home = source
        .strip_suffix("/historical-claims.jsonl")
        .ok_or(DurableError::Invalid("PG historical Claim source home"))?;
    let current = package_at_home(
        context
            .files
            .iter()
            .filter(|member| member.path.as_str().starts_with("ToS/"))
            .cloned()
            .collect::<Vec<_>>()
            .as_slice(),
        home,
    )
    .map_err(DurableError::Source)?;
    let revision = crate::source_revisions::revision(&current).map_err(DurableError::Source)?;
    let hash = revision
        .strip_prefix("sha256:")
        .ok_or(DurableError::Invalid(
            "PG historical Claim package revision",
        ))?;
    Ok(format!(
        "{ARCHIVE_PREFIX}{}-{hash}",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    ))
}

impl PreparedPgSourceRevision for PreparedPgFormsHistoricalCommand {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> PgRecordRevisionIdentity {
        match &self.readset_binding {
            PreparedReadsetBinding::Exact { identity, .. } => *identity,
        }
    }

    fn family_tag(&self) -> &str {
        &self.family_tag
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
        if writes.iter().any(|write| {
            self.proposal
                .changes
                .iter()
                .find(|change| change.path.as_str() == write.subject)
                .is_none_or(|change| change.after.as_deref() != Some(write.exact_bytes.as_slice()))
        }) {
            return Err(DurableError::Conflict(
                "PG HumanForms owner contributions differ from exact writes",
            ));
        }
        Ok(self.owner_contributions.clone())
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        match &self.readset_binding {
            PreparedReadsetBinding::Exact {
                identity,
                directories,
                identity_tokens,
            } => {
                if *identity != PgRecordRevisionIdentity::from_selection(&readset.selection) {
                    return Err(DurableError::Conflict(
                        "PG HumanForms exact selection identity differs",
                    ));
                }
                for file in self
                    .context
                    .files
                    .iter()
                    .filter(|file| file.path.as_str().starts_with("ToS/"))
                {
                    let Some(observation) = readset.observations.get(file.path.as_str()) else {
                        return Err(DurableError::Conflict(
                            "PG HumanForms exact member was not observed",
                        ));
                    };
                    if observation.member.path != file.path || observation.member.raw != file.raw {
                        return Err(DurableError::Conflict(
                            "PG HumanForms exact member differs from readset",
                        ));
                    }
                }
                for directory in directories {
                    if !readset.reads.reads.iter().any(|read| {
                        matches!(read, crate::PredicateRead::Generation { predicate, .. }
                            if predicate.kind == crate::PredicateKind::Range
                                && predicate.owner == source_cohort::OWNER
                                && predicate.scope == "source-home"
                                && predicate.token == *directory
                                && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
                    }) {
                        return Err(DurableError::Conflict("PG HumanForms directory predicate was not observed"));
                    }
                }
                for token in identity_tokens {
                    for kind in PG_AGENT_INDEX_KINDS {
                        if !readset.reads.reads.iter().any(|read| {
                            matches!(read, crate::PredicateRead::Generation { predicate, .. }
                                if predicate.kind == crate::PredicateKind::Unique
                                    && predicate.owner == source_cohort::OWNER
                                    && predicate.scope == kind
                                    && predicate.token == *token
                                    && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
                        }) {
                            return Err(DurableError::Conflict(
                                "PG historical Claim identity generation was not observed",
                            ));
                        }
                    }
                }
            }
        }
        let request = cmd::parse(&self.context.request_raw).map_err(DurableError::Source)?;
        let operation = cmd::text(&request, "operation").map_err(DurableError::Source)?;
        let expected = expected_family_tag(&self.context, operation)?;
        if self.identity() != PgRecordRevisionIdentity::from_selection(&readset.selection)
            || self.family_tag != expected
            || self.context.base_revision != readset.selection.cohort.initial_revision()
            || self.proposal.base_revision != readset.selection.cohort.initial_revision()
        {
            return Err(DurableError::Conflict(
                "PG HumanForms semantic preparation epoch differs",
            ));
        }
        for id in &self.inserted_form_ids {
            if !readset.reads.reads.iter().any(|read| {
                matches!(read, crate::PredicateRead::Generation { predicate, .. }
                    if predicate.kind == crate::PredicateKind::Unique
                        && predicate.owner == source_cohort::OWNER
                        && predicate.scope == "form"
                        && predicate.token == *id
                        && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
            }) {
                return Err(DurableError::Conflict("PG HumanForms unique identity generation was not observed"));
            }
        }
        Ok(())
    }

    fn work_revision_evidence(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> DurableResult<Option<crate::source_pg_work_revision::CommitEvidence>> {
        Ok(None)
    }

    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<IndexRows> {
        if writes.len() != self.proposal.changes.len() {
            return Err(DurableError::Conflict(
                "PG HumanForms durable writes differ from owner proposal",
            ));
        }
        let changes = self
            .proposal
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change))
            .collect::<BTreeMap<_, _>>();
        for write in writes {
            let change = changes
                .get(write.subject.as_str())
                .ok_or(DurableError::Conflict(
                    "PG HumanForms durable write path differs",
                ))?;
            if change.after.as_deref() != Some(write.exact_bytes.as_slice())
                || change.before.map(|digest| digest)
                    != write.expected_predecessor.map(|(_, digest)| digest)
            {
                return Err(DurableError::Conflict(
                    "PG HumanForms durable write bytes differ",
                ));
            }
            let paths = tx.query(
                "SELECT path FROM cmd2_source_index WHERE domain=$1 AND kind='path' AND token=$2",
                &[&domain, &write.subject],
            )?;
            if write.expected_predecessor.is_none() {
                let archive = write.subject.starts_with(ARCHIVE_PREFIX);
                let live_form = !archive && write.subject.ends_with(".human-forms.json");
                let history = write.subject.starts_with("ToS/source-witnesses/")
                    && write.subject.ends_with("/claim-revision-history.json");
                if paths.len() != 0 || (!archive && !live_form && !history) {
                    return Err(DurableError::Refused(
                        "PG HumanForms path insertion outside exact owner package",
                    ));
                }
            } else if paths.len() != 1 || paths[0].get::<_, String>("path") != write.subject {
                return Err(DurableError::Conflict(
                    "PG HumanForms predecessor path index is absent or ambiguous",
                ));
            }
            let semantic_rows = tx.query(
                "SELECT kind,token,path FROM cmd2_source_index WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\" AND kind<>'path' ORDER BY kind COLLATE \"C\",token COLLATE \"C\" LIMIT 513",
                &[&domain, &write.subject],
            )?;
            if semantic_rows.len() > 512 {
                return Err(DurableError::Refused(
                    "PG HumanForms semantic index exceeds owner bound",
                ));
            }
            let live_form = !write.subject.starts_with(ARCHIVE_PREFIX)
                && write.subject.ends_with(".human-forms.json");
            let successor_ids = if live_form {
                form_ids(&write.exact_bytes)?
            } else {
                BTreeSet::new()
            };
            let mut stored_ids = BTreeSet::new();
            for row in &semantic_rows {
                if row.get::<_, String>("path") != write.subject
                    || row.get::<_, String>("kind") != "form"
                {
                    return Err(DurableError::Refused(
                        "PG HumanForms encountered unsupported semantic row",
                    ));
                }
                stored_ids.insert(row.get::<_, String>("token"));
            }
            if write.expected_predecessor.is_some() {
                if !stored_ids.is_subset(&successor_ids) {
                    return Err(DurableError::Conflict(
                        "PG HumanForms existing semantic identity changed",
                    ));
                }
            } else if !semantic_rows.is_empty() {
                return Err(DurableError::Corrupt(
                    "PG HumanForms inserted path already has semantic rows",
                ));
            }
        }
        Ok(self.indexes.clone())
    }
}

fn prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-forms-historical-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn validate_owner_config(
    config: &tos_foundation::JsonValue,
    schema: &str,
    store_root: &Path,
) -> SourceCommandResult<RelativePath> {
    let expected_schemas = [
        "tos_local_source_command_owner_v1",
        "tos_local_canonical_form_owner_v1",
        "tos_local_claim_form_owner_v1",
        "tos_local_claim_form_owner_v2",
        "tos_local_historical_claim_form_owner_v1",
        "tos_local_historical_claim_revision_owner_v1",
    ];
    if !expected_schemas.contains(&schema) {
        return Err(SourceCommandError::Denied(
            "PG HumanForms exact protected owner family",
        ));
    }
    if cmd::text(config, "source_root")? != store_root.to_string_lossy() {
        return Err(SourceCommandError::Denied(
            "PG HumanForms owner root differs",
        ));
    }
    parse_source_path(config)
}

/// Run a single exact PG HumanForms or historical Claim operation.
pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    super::exact(
        invocation,
        &[
            "schema_version",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "source_selection",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "budgets",
        ],
    )?;
    if super::text(invocation, "schema_version")? != "tos_local_pg_forms_historical_invocation_v1" {
        return Err(SourceCommandError::Unsupported(
            "PG HumanForms invocation profile",
        ));
    }
    let selection = shared::selected_source(invocation)?;
    let budgets = budgets(invocation)?;
    let _ = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let expected_executable = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native HumanForms executable identity",
        ));
    }
    let owner_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let config = cmd::parse(&owner_raw)?;
    let owner_schema = cmd::text(&config, "schema_version")?;
    let source_path = validate_owner_config(&config, owner_schema, &selection.store_root)?;
    let request_raw = shared::read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PG HumanForms request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    let allowed = match owner_schema {
        "tos_local_source_command_owner_v1"
        | "tos_local_canonical_form_owner_v1"
        | "tos_local_claim_form_owner_v1"
        | "tos_local_claim_form_owner_v2"
        | "tos_local_historical_claim_form_owner_v1" => {
            ["describe", "prepare", "apply"].contains(&operation)
        }
        "tos_local_historical_claim_revision_owner_v1" => [
            "describe",
            "prepare-revise",
            "claim.revise",
            "inspect-version",
        ]
        .contains(&operation),
        _ => false,
    };
    if !allowed {
        return Err(SourceCommandError::Denied("PG HumanForms owner operation"));
    }
    let (software, components) = shared::software_selection(invocation, deadline, cancelled)?;
    let (software_files, software_members, software_bytes) = shared::software_files(
        &software,
        &components,
        budgets.max_member_bytes,
        deadline,
        cancelled,
    )?;
    if software_members > budgets.max_members || software_bytes > budgets.max_total_bytes {
        return Err(SourceCommandError::Invalid(
            "PG HumanForms selected software budget",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected HumanForms source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected HumanForms PostgreSQL"))?,
    );
    let ((prepared, mut result, context), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected = session.selection().clone();
                if selected.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected HumanForms source contract differs",
                    ));
                }
                let (context, proposal, readset_binding) = if owner_schema
                    .starts_with("tos_local_historical_claim_")
                {
                    let (context, authored_members, binding, current, identity_index) =
                        exact_historical_context(
                            session,
                            &config,
                            &owner_raw,
                            &request_raw,
                            crate::source_serialization::instant().map_err(DurableError::Source)?,
                            uid,
                            software_files,
                            software_members,
                            software_bytes,
                            &budgets,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                    owner_fs
                        .current_context(&context, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    context
                        .check_selected_software_inputs(&software, &components, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    let mut worker = super::revisions::selected_schema_from_context(
                        invocation,
                        &context.files,
                        context.base_revision,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    let home = source_path
                        .as_str()
                        .strip_suffix("/historical-claims.jsonl")
                        .ok_or(DurableError::Invalid("PG historical Claim source home"))?;
                    let mut archives = PgArchiveReader {
                        members: &authored_members,
                    };
                    let plan = super::legacy_claim::engine::prepare_from_indexed_members(
                        &context,
                        context.base_revision,
                        &authored_members,
                        &identity_index,
                        &current,
                        &mut archives,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    if plan.reads.iter().any(|read| {
                        !context
                            .files
                            .iter()
                            .any(|selected| selected.path == read.path && selected.raw == read.raw)
                    }) {
                        return Err(DurableError::Refused(
                            "historical Claim dependency outside selected PG closure",
                        ));
                    }
                    let mut changes = Vec::new();
                    if let Some(after) = &plan.files {
                        changes = package_changes(
                            &context,
                            home,
                            &current,
                            after,
                            plan.archive
                                .as_ref()
                                .map(|(reference, files)| (reference.as_str(), files)),
                        )
                        .map_err(DurableError::Source)?;
                    }
                    let handler = if owner_schema == "tos_local_historical_claim_revision_owner_v1"
                    {
                        "legacy-historical-claim-revision"
                    } else {
                        "legacy-historical-claim-forms"
                    };
                    let proposal = context
                        .plan(handler, plan.response.clone(), changes, plan.replayed)
                        .map_err(DurableError::Source)?;
                    finish_creation_worker(&mut worker, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    (context, proposal, binding)
                } else {
                    let (context, authored_members, binding) = exact_human_form_context(
                        session,
                        &config,
                        owner_schema,
                        &request_raw,
                        &owner_raw,
                        crate::source_serialization::instant().map_err(DurableError::Source)?,
                        uid,
                        software_files,
                        software_members,
                        software_bytes,
                        &budgets,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    owner_fs
                        .current_context(&context, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    context
                        .check_selected_software_inputs(&software, &components, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    let mut worker = super::revisions::selected_schema_from_context(
                        invocation,
                        &context.files,
                        context.base_revision,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    let proposal = source_forms::run_form_command_from_pg_members(
                        &context,
                        context.base_revision,
                        &authored_members,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    finish_creation_worker(&mut worker, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    (context, proposal, binding)
                };
                let result = shared::value(&proposal.response).map_err(DurableError::Source)?;
                let mut prepared = None;
                if matches!(operation, "apply" | "claim.revise") {
                    let typed = PreparedPgFormsHistoricalCommand::new(
                        context.clone(),
                        proposal,
                        readset_binding,
                    )?;
                    for id in &typed.inserted_form_ids {
                        session.observe_unique_index("form", id, deadline, cancelled)?;
                    }
                    prepared = Some(typed);
                }
                Ok((prepared, result, context))
            },
        )
        .map_err(shared::source_error)?;
    owner_fs.current_context(&context, deadline, cancelled)?;
    let source_work = readset.work;
    result["source_read_work"] = json!({
        "scope":if owner_schema.starts_with("tos_local_historical_claim_") { "exact historical home and retained archives, required identity-index points and selected endpoint/evidence/schema closure; original corpus witness and software capture excluded" } else { "exact owner source, adjacent form set, selected source/form schema closure, and parent directory predicates; software capture excluded" },
        "exact_source_members_read":source_work.exact_source_members_read,
        "exact_source_bytes_read":source_work.exact_source_bytes_read,
        "historical_source_members_read":source_work.historical_source_members_read,
        "historical_source_bytes_read":source_work.historical_source_bytes_read,
        "custody_metadata_bytes_read":source_work.custody_metadata_bytes_read,
        "segment_bytes_read":source_work.segment_bytes_read,
        "committed_member_rows_examined":source_work.committed_member_rows_examined,
        "metadata_member_lookups":source_work.metadata_member_lookups,
        "directory_index_rows_examined":source_work.directory_index_rows_examined,
        "unique_index_lookups":source_work.unique_index_lookups,
    });
    if let Some(prepared) = prepared {
        let proposal = prepared.proposal();
        let request = cmd::parse(&request_raw)?;
        let command_id = cmd::text(&request, "command_id")?;
        let prepare_id = prepare_id(&selection.domain, command_id);
        let attempt = if proposal.replayed {
            db.recover_source_revision_attempt(
                &store,
                &selection.domain,
                &prepare_id,
                command_id,
                Digest256::of_bytes(&request_raw),
                proposal.configuration_raw_sha256,
                proposal.configuration_canonical_sha256,
                prepared.family_tag(),
                false,
                &selection.job_id,
                selection.job_fence,
                None,
                deadline,
                cancelled,
            )
            .map_err(shared::source_error)?
        } else {
            db.register_source_revision(
                &store,
                &prepared,
                &readset,
                &prepare_id,
                deadline,
                cancelled,
            )
            .map_err(shared::source_error)?
        };
        let owner_check = || {
            if super::read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?
                != invocation_raw
            {
                return Err(DurableError::Conflict("PG HumanForms invocation changed"));
            }
            let (current_fs, current_raw) =
                CreationFilesystem::select_protected_native_owner_at_root(
                    &owner_path,
                    &selection.store_root,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
            if current_raw != owner_raw {
                return Err(DurableError::Conflict(
                    "PG HumanForms owner configuration changed",
                ));
            }
            current_fs
                .current_context(prepared.context(), deadline, cancelled)
                .map_err(DurableError::Source)?;
            context
                .check_selected_software_inputs(&software, &components, deadline, cancelled)
                .map_err(DurableError::Source)?;
            if runtime_observation
                .current_digest(deadline, cancelled)
                .map_err(DurableError::Source)?
                != expected_executable
            {
                return Err(DurableError::Conflict(
                    "native HumanForms executable changed",
                ));
            }
            Ok(())
        };
        let (receipt, _) = db
            .commit_source_revision(
                &store,
                &attempt,
                selection.contract,
                &owner_check,
                deadline,
                cancelled,
            )
            .map_err(shared::source_error)?;
        result["replayed"] = json!(receipt.replayed);
        result["durable_commit"] = shared::commit_result(&receipt);
    }
    drop(owner_fs);
    Ok(json!({
        "schema_version":"tos_local_pg_forms_historical_result_v1",
        "result":result,
        "grants_admission":false
    }))
}
