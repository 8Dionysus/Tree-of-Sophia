//! Native owner-local TextLayer/TextUnit commands over the selected PG source
//! epoch and the private-owner package broker. Text meanings remain in the
//! maintained owner entries; this module binds their exact byte reads and
//! complete prepared package to the durable private transaction.

use super::pg_claims as pg;
use crate::source_cohort::revision::{
    SourceRevisionReadSession, SourceRevisionReadSet, SourceRevisionSelection,
};
use crate::source_cohort_private_text::{
    PreparedPrivateTextPackageV1, PrivateTextPackageCapabilityV1, private_text_source_binding,
};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_native_text_read::{LocalTextReadSelection, PG_PRIVATE_SCHEMA};
use crate::source_sign_native::{
    NativeReadKind, ResolvedDerivedTextSource, ResolvedInitialTextSource, ResolvedOwnerTextLayer,
    ResolvedOwnerTextPacket, SignNativeRead, resolve_derived_owner_text_source,
    resolve_initial_owner_text_source, resolve_owner_text_layer, resolve_owner_text_packet,
};
use crate::source_text_identity;
use crate::source_text_layer_derived_entry as derived;
use crate::source_text_layer_entry as initial;
use crate::source_text_owner::{
    OwnerTextContext, OwnerTextDerivedSelection, OwnerTextInitialLayerSelection,
    OwnerTextUnitSelection, read_absolute,
};
use crate::source_text_unit_entry as unit;
use crate::{
    DurableError, DurablePgCoordinator,
    durable_private_packages::{
        PrivateOwnerPackageFamilyV1, PrivateOwnerPackageSnapshotV1, PrivateOwnerReadSession,
        PrivateOwnerScopeV1,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutSchemaExecutor;

const MAX_SCHEMA_FILES: usize = 14;
const MAX_SCHEMA_BYTES: usize = 16_777_216;
const MAX_SCHEMA_FILE_BYTES: u64 = 4_194_304;
const MAX_TEXT_MEMBERS: u64 = 4_096;
const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXT_MEMBER_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";

fn invocation_budgets(invocation: &Value) -> SourceCommandResult<(usize, u64, u64)> {
    let budgets = &invocation["budgets"];
    super::exact(
        budgets,
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
    let max_members = usize::try_from(super::capped(budgets, "max_members", MAX_TEXT_MEMBERS)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL private Text member budget"))?;
    let max_total_bytes = super::capped(budgets, "max_total_bytes", MAX_TEXT_BYTES)?;
    let max_member_bytes = super::capped(budgets, "max_member_bytes", MAX_TEXT_MEMBER_BYTES)?;
    let _ = super::capped(budgets, "max_schema_receipts", 128)?;
    let _ = super::capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = super::capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = super::capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok((max_members, max_total_bytes, max_member_bytes))
}

#[derive(Clone)]
struct SelectedEpoch {
    domain: String,
    job_id: String,
    job_fence: u64,
    contract: Digest256,
    head_seq: u64,
    epoch: u64,
    source_generation: u64,
    source_revision: Digest256,
    source_audit_generation: u64,
    rights_version: u64,
    rule_version: u64,
}

fn epoch_from_selection(selected: &SourceRevisionSelection) -> SelectedEpoch {
    SelectedEpoch {
        domain: selected.cohort.domain().to_owned(),
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        contract: selected.contract,
        head_seq: selected.head_seq,
        epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
    }
}

fn epoch(session: &SourceRevisionReadSession<'_>) -> SelectedEpoch {
    epoch_from_selection(session.selection())
}

fn same_epoch(a: &SelectedEpoch, b: &SelectedEpoch) -> bool {
    a.domain == b.domain
        && a.job_id == b.job_id
        && a.job_fence == b.job_fence
        && a.contract == b.contract
        && a.head_seq == b.head_seq
        && a.epoch == b.epoch
        && a.source_generation == b.source_generation
        && a.source_revision == b.source_revision
        && a.source_audit_generation == b.source_audit_generation
        && a.rights_version == b.rights_version
        && a.rule_version == b.rule_version
}

fn read_member(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    max_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Text source path"))?;
    session
        .read_current_member(&path, max_bytes, deadline, cancelled)
        .map(|member| member.raw)
        .map_err(pg::source_error)
}

pub(crate) fn selected_contracts(
    session: &mut SourceRevisionReadSession<'_>,
    family: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    // Text validation uses a closed route-specific contract set. Load these
    // exact paths through the selected PG epoch; never enumerate the full
    // contract tree on an ordinary create/derive operation.
    let names: &[&str] = match family {
        "tos_local_text_layer_create_owner_v1"
        | "tos_local_text_layer_derive_owner_v1"
        | "tos_local_text_layer_record_owner_ocr_v1"
        | "tos_local_text_layer_record_owner_page_ocr_v1" => &[
            "owner-local-source-context.schema.json",
            "corpus-record.schema.json",
            "source-item-manifest.schema.json",
            "rights-record.schema.json",
            "source-text-layer.schema.json",
            "source-anchor-v2.schema.json",
            "provenance-event-v2.schema.json",
            "native-text-layer-binding.schema.json",
        ],
        "tos_local_text_unit_create_owner_v1" | "tos_local_text_unit_create_owner_v2" => &[
            "owner-local-source-context.schema.json",
            "corpus-record.schema.json",
            "source-item-manifest.schema.json",
            "rights-record.schema.json",
            "source-text-layer.schema.json",
            "source-anchor-v2.schema.json",
            "provenance-event-v2.schema.json",
            "native-text-layer-binding.schema.json",
            "native-text-unit-binding.schema.json",
            "source-text-unit-packet-v1.schema.json",
        ],
        "tos_native_pg_private_text_read_v1" => &[
            "owner-local-source-context.schema.json",
            "corpus-record.schema.json",
            "source-item-manifest.schema.json",
            "rights-record.schema.json",
            "source-text-layer.schema.json",
            "source-anchor-v2.schema.json",
            "provenance-event-v2.schema.json",
            "native-text-layer-binding.schema.json",
            "native-text-unit-binding.schema.json",
            "source-text-unit-packet-v1.schema.json",
            "native-pg-private-text-read.schema.json",
            "occurrence-description-record.schema.json",
            "source-metadata-record.schema.json",
            "semantic-description-record.schema.json",
        ],
        _ => {
            return Err(SourceCommandError::Denied(
                "PostgreSQL private Text owner family",
            ));
        }
    };
    if names.len() > MAX_SCHEMA_FILES {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Text schema file budget",
        ));
    }
    for name in names {
        let path = format!("ToS/contracts/{name}");
        let raw = read_member(session, &path, MAX_SCHEMA_FILE_BYTES, deadline, cancelled)?;
        total = total
            .checked_add(raw.len())
            .filter(|n| *n <= MAX_SCHEMA_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL Text schema byte budget",
            ))?;
        files.insert(path, raw);
    }
    if !files.contains_key(CONTEXT_SCHEMA) {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Text owner context schema absent",
        ));
    }
    Ok(files)
}

fn collect_private_target_refs(value: &JsonValue, prefix: &str, out: &mut BTreeSet<String>) {
    if let Some(text) = value.as_str() {
        if text.starts_with(prefix) {
            if let Some((target, name)) = text.rsplit_once('/') {
                if !name.is_empty() && RelativePath::parse(target).is_ok() {
                    out.insert(target.to_owned());
                }
            }
        }
    } else if let Some(values) = value.as_array() {
        for value in values {
            collect_private_target_refs(value, prefix, out);
        }
    } else if let Some(values) = value.as_object() {
        for (_, value) in values {
            collect_private_target_refs(value, prefix, out);
        }
    }
}

fn hydrate_private_snapshot(
    snapshot: PrivateOwnerPackageSnapshotV1,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    private_generation: u64,
    max_total_bytes: u64,
    max_member_bytes: u64,
    observed_total_bytes: &mut u64,
    private_members: &mut BTreeMap<String, Vec<u8>>,
    dependency_members: &mut BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<()> {
    if snapshot.scope != *scope
        || snapshot.target_ref != target_ref
        || snapshot.family != PrivateOwnerPackageFamilyV1::Text
        || snapshot.private_audit_generation != private_generation
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text dependency generation or family changed",
        ));
    }
    for (name, bytes) in snapshot.members {
        let size = bytes.len() as u64;
        *observed_total_bytes = (*observed_total_bytes)
            .checked_add(size)
            .filter(|total| *total <= max_total_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL private Text dependency byte budget",
            ))?;
        if size > max_member_bytes {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL private Text dependency member budget",
            ));
        }
        let reference = format!("{target_ref}/{name}");
        if private_members
            .insert(reference.clone(), bytes.clone())
            .is_some()
            || dependency_members.insert(reference, bytes).is_some()
        {
            return Err(SourceCommandError::Conflict(
                "duplicate PostgreSQL private Text dependency",
            ));
        }
    }
    Ok(())
}

fn source_files(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    files
        .iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(path).map_err(|_| {
                    SourceCommandError::Invalid("PostgreSQL Text selected source path")
                })?,
                raw: raw.clone(),
            })
        })
        .collect()
}

struct SessionTextRead<'s, 'a> {
    session: &'s mut SourceRevisionReadSession<'a>,
    private_members: &'s BTreeMap<String, Vec<u8>>,
    context: &'s OwnerTextContext,
    selected_members: BTreeMap<String, Vec<u8>>,
    missing_private_reference: Option<String>,
}

impl SignNativeRead for SessionTextRead<'_, '_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let confidential = self.context.is_owner_private_reference(reference);
        let raw = if confidential {
            match self.private_members.get(reference).cloned() {
                Some(raw) => raw,
                None => {
                    self.missing_private_reference
                        .get_or_insert_with(|| reference.to_owned());
                    return Err(SourceCommandError::Conflict(
                        "PostgreSQL private Text dependency unavailable",
                    ));
                }
            }
        } else if let Some(selected) = self.selected_members.get(reference) {
            selected.clone()
        } else {
            read_member(
                self.session,
                reference,
                max_bytes as u64,
                deadline,
                cancelled,
            )?
        };
        if raw.len() > max_bytes {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL Text selected member byte budget",
            ));
        }
        if self
            .selected_members
            .get(reference)
            .is_some_and(|old| old != &raw)
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text selected member changed during read",
            ));
        }
        self.selected_members
            .insert(reference.to_owned(), raw.clone());
        Ok(raw)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text read deadline",
            ));
        }
        Ok(())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        Ok(self.context.is_owner_private_reference(reference))
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        Ok(Some(
            self.context.snapshot(deadline, cancelled)?.to_prefixed(),
        ))
    }
}

#[derive(Clone)]
struct PgPrivateTextPackagePin {
    revision: u64,
    package_digest: Digest256,
    command_id: String,
}

/// Read current public source members through the selected source epoch and
/// private members lazily from the exact package revisions named by the
/// independent PG disclosure profile. Metadata and rights are resolved
/// before the native reader asks for content bytes.
struct PgPrivateTextRead<'s, 'a, 'db> {
    session: &'s mut SourceRevisionReadSession<'a>,
    private_session: &'s mut PrivateOwnerReadSession<'db>,
    pins: &'s BTreeMap<String, PgPrivateTextPackagePin>,
    context: &'s OwnerTextContext,
    selected_members: BTreeMap<String, Vec<u8>>,
}

impl SignNativeRead for PgPrivateTextRead<'_, '_, '_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if let Some(raw) = self.selected_members.get(reference) {
            if raw.len() > max_bytes {
                return Err(SourceCommandError::Unsupported(
                    "PG private Text selected member byte budget",
                ));
            }
            return Ok(raw.clone());
        }
        let raw = if self.context.is_owner_private_reference(reference) {
            let target_ref = self.context.private_package_target_ref(reference)?;
            let pin = self
                .pins
                .get(&target_ref)
                .ok_or(SourceCommandError::Denied(
                    "PG private Text package is not explicitly selected",
                ))?;
            let member_name = reference
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid(
                    "PG private Text package member path",
                ))?;
            let selected = self
                .private_session
                .read_selected_member_revision(
                    &target_ref,
                    pin.revision,
                    pin.package_digest,
                    &pin.command_id,
                    member_name,
                    max_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(pg::source_error)?
                .ok_or(SourceCommandError::Conflict(
                    "PG private Text selected package member absent",
                ))?;
            if selected.private_audit_generation != self.private_session.private_audit_generation()
            {
                return Err(SourceCommandError::Conflict(
                    "PG private Text package generation changed",
                ));
            }
            selected.bytes
        } else {
            read_member(
                self.session,
                reference,
                max_bytes as u64,
                deadline,
                cancelled,
            )?
        };
        if raw.len() > max_bytes {
            return Err(SourceCommandError::Unsupported(
                "PG private Text selected member byte budget",
            ));
        }
        self.selected_members
            .insert(reference.to_owned(), raw.clone());
        Ok(raw)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
            return Err(SourceCommandError::Conflict(
                "PG private Text read deadline",
            ));
        }
        Ok(())
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        Ok(self.context.is_owner_private_reference(reference))
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        Ok(Some(
            self.context.snapshot(deadline, cancelled)?.to_prefixed(),
        ))
    }
}

fn pg_private_text_read(
    request: &JsonValue,
    profile: &LocalTextReadSelection,
    context: &OwnerTextContext,
    scope: &PrivateOwnerScopeV1,
    selection: &pg::SourceSelection,
    store: &SegmentStore,
    first_epoch: &SelectedEpoch,
    schema_members: &BTreeMap<String, Vec<u8>>,
    db: &mut DurablePgCoordinator,
    worker: &mut super::CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    cmd::exact_keys(
        request,
        &[
            "schema_version",
            "operation",
            "source_path",
            "revision",
            "package_sha256",
            "command_id",
            "binding_ref",
            "binding",
            "max_return_bytes",
        ],
    )?;
    if cmd::text(request, "schema_version")? != "tos_local_native_private_text_read_request_v1"
        || cmd::text(request, "operation")? != "native-text.read-private"
        || cmd::text(profile.config(), "source_domain")? != selection.domain.as_str()
        || cmd::text(profile.config(), "source_contract_sha256")?
            != first_epoch.contract.to_hex().as_str()
    {
        return Err(SourceCommandError::Denied(
            "PG private Text read profile or request source binding differs",
        ));
    }
    profile.verify(deadline, cancelled)?;
    let owner_context = cmd::field(profile.config(), "owner_context")?;
    let context_path =
        crate::source_text_owner::normalized_absolute(cmd::text(owner_context, "path")?)?;
    let selected_context_path = crate::source_text_owner::normalized_absolute(
        context
            .configuration_path()
            .to_str()
            .ok_or(SourceCommandError::Invalid(
                "PG private Text context path UTF8",
            ))?,
    )?;
    if context_path != selected_context_path
        || cmd::text(owner_context, "sha256")?
            != context.configuration_sha256(deadline, cancelled)?.as_str()
    {
        return Err(SourceCommandError::Denied(
            "PG private Text read context binding differs",
        ));
    }

    let source_path = cmd::text(request, "source_path")?;
    let packet_ref = context.private_package_target_ref(source_path)?;
    if !source_path.ends_with("/source-text-unit.v1.json")
        || cmd::text(cmd::field(request, "binding")?, "packet_ref")? != source_path
    {
        return Err(SourceCommandError::Denied(
            "PG private Text read requires the exact private TextUnit packet",
        ));
    }
    let binding = cmd::field(request, "binding")?.clone();
    let selected_packages = profile.selected_pg_packages(&binding, None, deadline, cancelled)?;
    let mut pins = BTreeMap::new();
    let mut selected_packet_pin = false;
    for package in selected_packages {
        let target_ref = cmd::text(&package, "target_ref")?.to_owned();
        if !target_ref.starts_with(context.private_prefix())
            || RelativePath::parse(&target_ref).is_err()
            || pins.contains_key(&target_ref)
        {
            return Err(SourceCommandError::Denied(
                "PG private Text read package pin is duplicated or outside owner scope",
            ));
        }
        let revision = cmd::integer(&package, "revision")?;
        if revision == 0 {
            return Err(SourceCommandError::Invalid(
                "PG private Text read package revision",
            ));
        }
        let package_digest = Digest256::from_hex(cmd::text(&package, "package_sha256")?)
            .map_err(|_| SourceCommandError::Invalid("PG private Text package pin digest"))?;
        let command_id = cmd::text(&package, "command_id")?.to_owned();
        if command_id.is_empty() || command_id.len() > 1024 || command_id.contains('\0') {
            return Err(SourceCommandError::Invalid(
                "PG private Text read package command ID",
            ));
        }
        if target_ref == packet_ref
            && revision == cmd::integer(request, "revision")?
            && package_digest.to_hex().as_str() == cmd::text(request, "package_sha256")?
            && command_id.as_str() == cmd::text(request, "command_id")?
        {
            selected_packet_pin = true;
        }
        pins.insert(
            target_ref,
            PgPrivateTextPackagePin {
                revision,
                package_digest,
                command_id,
            },
        );
    }
    if !selected_packet_pin || pins.is_empty() || pins.len() > 32 {
        return Err(SourceCommandError::Denied(
            "PG private Text read exact packet revision is not selected",
        ));
    }
    let binding_ref = cmd::text(request, "binding_ref")?;
    if !binding_ref.starts_with("ToS/source-witnesses/")
        || !binding_ref.ends_with("/occurrence.json")
        || RelativePath::parse(binding_ref).is_err()
    {
        return Err(SourceCommandError::Denied(
            "PG private Text read binding reference",
        ));
    }
    let max_return_bytes = usize::try_from(cmd::integer(request, "max_return_bytes")?)
        .map_err(|_| SourceCommandError::Invalid("PG private Text return budget"))?;

    let result = db
        .with_private_owner_read_session(scope, deadline, cancelled, |private_session, _, _| {
            if private_session.scope() != scope {
                return Err(DurableError::Conflict(
                    "PG private Text read owner scope changed",
                ));
            }
            let mut source_db = DurablePgCoordinator::from_configured_client(
                crate::source_projection_cli::connect_selected_source_pg16()
                    .map_err(|_| DurableError::Conflict("selected Text PostgreSQL read"))?,
            );
            source_db.with_source_revision_session(
                store,
                &selection.domain,
                &selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
                |session| {
                    if !same_epoch(first_epoch, &epoch(session)) {
                        return Err(DurableError::Conflict(
                            "PG private Text read source epoch changed",
                        ));
                    }
                    let current_schemas = selected_contracts(
                        session,
                        "tos_native_pg_private_text_read_v1",
                        deadline,
                        cancelled,
                    )
                    .map_err(source_command_error)?;
                    if &current_schemas != schema_members {
                        return Err(DurableError::Conflict(
                            "PG private Text read contracts changed",
                        ));
                    }
                    let mut reader = PgPrivateTextRead {
                        session,
                        private_session,
                        pins: &pins,
                        context,
                        selected_members: BTreeMap::new(),
                    };
                    let current_binding =
                        crate::source_sign_native::validate_current_occurrence_binding(
                            &mut reader,
                            worker,
                            binding_ref,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                    if cmd::canonical(&current_binding).map_err(DurableError::Source)?
                        != cmd::canonical(&binding).map_err(DurableError::Source)?
                    {
                        return Err(DurableError::Conflict(
                            "PG private Text read binding changed",
                        ));
                    }
                    crate::source_sign_native::read_private_disclosed_unit(
                        &mut reader,
                        worker,
                        &binding,
                        profile,
                        max_return_bytes,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)
                },
            )
        })
        .map_err(pg::source_error)?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("PG private Text read worker FINAL"))?;
    profile.verify(deadline, cancelled)?;
    if context.configuration_sha256(deadline, cancelled)?.as_str()
        != cmd::text(owner_context, "sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "PG private Text read context changed",
        ));
    }
    let (value, readset): (JsonValue, SourceRevisionReadSet) = result;
    if !same_epoch(first_epoch, &epoch_from_selection(&readset.selection)) {
        return Err(SourceCommandError::Conflict(
            "PG private Text read result source epoch changed",
        ));
    }
    pg::value(&value)
}

enum SelectedGrant {
    Initial(OwnerTextInitialLayerSelection),
    Derived(OwnerTextDerivedSelection),
    Unit(OwnerTextUnitSelection),
}

enum PreparedPrivateTextOperation {
    Commit(PreparedPrivateTextPackageV1),
    InitialPreview(initial::NativeInitialTextLayerPreview),
    DerivedPreview(derived::NativeDerivedTextLayerPreview),
    Preview(unit::NativeFirstTextUnitPreview),
}

impl SelectedGrant {
    fn config(&self) -> &JsonValue {
        match self {
            Self::Initial(grant) => &grant.config,
            Self::Derived(grant) => &grant.config,
            Self::Unit(grant) => &grant.config,
        }
    }

    fn raw(&self) -> &[u8] {
        match self {
            Self::Initial(grant) => &grant.raw,
            Self::Derived(grant) => &grant.raw,
            Self::Unit(grant) => &grant.raw,
        }
    }

    fn operation(&self) -> &str {
        match self {
            Self::Initial(_) => "text-layer.create",
            Self::Derived(grant) => &grant.operation,
            Self::Unit(_) => "text-unit.create",
        }
    }

    fn delegated_ids(&self) -> SourceCommandResult<Vec<String>> {
        match self {
            Self::Initial(grant) => initial::selected_ids_for_pg(grant),
            Self::Derived(grant) => derived::selected_ids_for_pg(grant),
            Self::Unit(grant) => unit::identity_ids_for_pg(grant),
        }
    }
}

fn select_grant(
    context: &OwnerTextContext,
    family: &str,
    grant_path: &Path,
    worker: &dyn CutSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(SelectedGrant, String)> {
    match family {
        "tos_local_text_layer_create_owner_v1" => {
            let grant =
                OwnerTextInitialLayerSelection::select(context, grant_path, deadline, cancelled)?;
            let contracts = initial::selected_contracts(context, worker, deadline, cancelled)?;
            let owner_configuration =
                initial::selected_configuration(context, &grant, &contracts, deadline, cancelled)?;
            Ok((SelectedGrant::Initial(grant), owner_configuration))
        }
        "tos_local_text_layer_derive_owner_v1"
        | "tos_local_text_layer_record_owner_ocr_v1"
        | "tos_local_text_layer_record_owner_page_ocr_v1" => {
            let grant =
                OwnerTextDerivedSelection::select(context, grant_path, deadline, cancelled)?;
            let contracts = derived::contracts(context, worker, deadline, cancelled)?;
            let owner_configuration =
                derived::selected_configuration(context, &grant, &contracts, deadline, cancelled)?;
            Ok((SelectedGrant::Derived(grant), owner_configuration))
        }
        "tos_local_text_unit_create_owner_v1" | "tos_local_text_unit_create_owner_v2" => {
            let grant = OwnerTextUnitSelection::select(context, grant_path, deadline, cancelled)?;
            let packet_mode = family.ends_with("v1");
            let contracts =
                unit::selected_contracts(context, worker, packet_mode, deadline, cancelled)?;
            let owner_configuration =
                unit::configuration(context, &grant, &contracts, deadline, cancelled)?;
            Ok((SelectedGrant::Unit(grant), owner_configuration))
        }
        _ => Err(SourceCommandError::Denied(
            "PostgreSQL private Text owner family",
        )),
    }
}

struct ResolvedSelectedSource {
    selected_members: BTreeMap<String, Vec<u8>>,
    missing_private_reference: Option<String>,
}

fn resolve_selected_source(
    grant: &SelectedGrant,
    context: &OwnerTextContext,
    session: &mut SourceRevisionReadSession<'_>,
    schema_members: &BTreeMap<String, Vec<u8>>,
    private_members: &BTreeMap<String, Vec<u8>>,
    worker: &mut dyn CutSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedSelectedSource> {
    let mut reader = SessionTextRead {
        session,
        private_members,
        context,
        selected_members: schema_members.clone(),
        missing_private_reference: None,
    };
    let result = match grant {
        SelectedGrant::Initial(grant) => {
            resolve_initial_owner_text_source(&mut reader, worker, grant, deadline, cancelled)
                .map(|_resolved: ResolvedInitialTextSource| ())
        }
        SelectedGrant::Derived(grant) => resolve_derived_owner_text_source(
            &mut reader,
            worker,
            &grant.config,
            deadline,
            cancelled,
        )
        .map(|_resolved: ResolvedDerivedTextSource| ()),
        SelectedGrant::Unit(grant) => {
            if cmd::text(&grant.config, "schema_version")? == "tos_local_text_unit_create_owner_v1"
            {
                resolve_owner_text_packet(
                    &mut reader,
                    worker,
                    cmd::field(&grant.config, "source_binding")?,
                    deadline,
                    cancelled,
                )
                .map(|_resolved: ResolvedOwnerTextPacket| ())
            } else {
                resolve_owner_text_layer(
                    &mut reader,
                    worker,
                    cmd::field(&grant.config, "source_binding")?,
                    deadline,
                    cancelled,
                )
                .map(|_resolved: ResolvedOwnerTextLayer| ())
            }
        }
    };
    if let Err(error) = result {
        if reader.missing_private_reference.is_none() {
            return Err(error);
        }
    }
    Ok(ResolvedSelectedSource {
        selected_members: reader.selected_members,
        missing_private_reference: reader.missing_private_reference,
    })
}

fn source_command_error(error: SourceCommandError) -> DurableError {
    DurableError::Source(error)
}

fn prepare_package(
    grant: SelectedGrant,
    context: OwnerTextContext,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    request: &JsonValue,
    retained: Option<&BTreeMap<String, Vec<u8>>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut dyn CutSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPrivateTextPackageV1> {
    match grant {
        SelectedGrant::Initial(grant) => initial::prepare_pg_initial_text_package(
            context,
            publication,
            grant,
            request,
            retained,
            software,
            components,
            worker,
            deadline,
            cancelled,
        )
        .map(PreparedPrivateTextPackageV1::Initial),
        SelectedGrant::Derived(grant) => derived::prepare_pg_derived_text_package(
            context,
            publication,
            grant,
            request,
            retained,
            software,
            components,
            worker,
            deadline,
            cancelled,
        )
        .map(PreparedPrivateTextPackageV1::Derived),
        SelectedGrant::Unit(grant) => unit::prepare_pg_text_unit_package(
            context,
            publication,
            grant,
            request,
            retained,
            software,
            components,
            worker,
            deadline,
            cancelled,
        )
        .map(PreparedPrivateTextPackageV1::TextUnit),
    }
}

/// Execute a real private Text package command against an existing selected
/// source cohort. It never opens a local private package as a source of truth;
/// protected owner configuration/grants and acquired payload roots remain
/// owner-local inputs selected by the existing Text contracts.
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let parsed_invocation = cmd::parse(invocation_raw)?;
    cmd::exact_keys(
        &parsed_invocation,
        &[
            "schema_version",
            "owner_context",
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
    let _ = invocation_path;
    if super::text(invocation, "schema_version")? != "tos_local_pg_private_text_invocation_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private Text invocation profile",
        ));
    }
    let selection = pg::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = invocation_budgets(invocation)?;
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL private Text empty budget",
        ));
    }
    let _ = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let executable_digest = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text executable identity",
        ));
    }
    let (software, components) = pg::software_selection(invocation, deadline, cancelled)?;
    let software_members = components.members().collect::<Vec<_>>();
    if software_members.len() > max_members
        || software_members
            .iter()
            .any(|member| member.size_bytes > max_member_bytes)
        || software_members
            .iter()
            .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
            .is_none_or(|sum| sum > max_total_bytes)
    {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL private Text software budget",
        ));
    }
    let request_raw = pg::read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let operation = cmd::text(&request, "operation")?.to_owned();
    let context_path = super::absolute(super::text(invocation, "owner_context")?)?;
    let grant_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let raw = read_absolute(&grant_path, uid, true, 1_048_576, deadline, cancelled)?;
    let family_hint = cmd::parse(&raw)?;
    let family = cmd::text(&family_hint, "schema_version")?.to_owned();
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Text source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected Text PostgreSQL"))?,
    );

    let ((first_epoch, schema_members), _) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let epoch = epoch(session);
                if epoch.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Text source contract differs",
                    ));
                }
                let schemas = selected_contracts(session, &family, deadline, cancelled)
                    .map_err(source_command_error)?;
                Ok((epoch, schemas))
            },
        )
        .map_err(pg::source_error)?;
    let schema_bytes = schema_members
        .values()
        .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
        .ok_or(SourceCommandError::Invalid(
            "PostgreSQL private Text schema budget",
        ))?;
    if schema_members.len() > max_members
        || schema_bytes > max_total_bytes
        || schema_members
            .values()
            .any(|raw| raw.len() as u64 > max_member_bytes)
    {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL private Text schema budget",
        ));
    }
    let schema_files = source_files(&schema_members)?;
    let mut worker = super::revisions::selected_schema_from_context(
        invocation,
        &schema_files,
        SourceRevision(first_epoch.source_revision),
        deadline,
        cancelled,
    )?;
    let context_schema = schema_members
        .get(CONTEXT_SCHEMA)
        .ok_or(SourceCommandError::Conflict(
            "selected Text context schema absent",
        ))?;
    let (context, _) = OwnerTextContext::select_pg(
        &context_path,
        context_schema,
        &mut worker,
        deadline,
        cancelled,
    )?;
    // Contract reads below must route through the exact schema set selected
    // from the PG source epoch, never the local filesystem projection.
    context.install_pg_selected_members(schema_members.clone())?;
    let owner_context_binding = context.private_owner_scope_binding(deadline, cancelled)?;
    let scope = PrivateOwnerScopeV1::for_context(&selection.domain, owner_context_binding)
        .map_err(pg::source_error)?;
    if family == "tos_native_pg_private_text_read_v1" {
        let schema_raw =
            schema_members
                .get(PG_PRIVATE_SCHEMA)
                .ok_or(SourceCommandError::Conflict(
                    "PG private Text read schema absent",
                ))?;
        let profile = LocalTextReadSelection::load_pg_private_profile(
            &grant_path,
            context.public_root(),
            schema_raw,
            &mut worker,
            deadline,
            cancelled,
        )?;
        let profile_context = cmd::field(profile.config(), "owner_context")?;
        if cmd::text(profile.config(), "source_domain")? != selection.domain.as_str()
            || cmd::text(profile.config(), "source_contract_sha256")?
                != first_epoch.contract.to_hex().as_str()
            || crate::source_text_owner::normalized_absolute(cmd::text(profile_context, "path")?)?
                .as_path()
                != context.configuration_path()
            || cmd::text(profile_context, "sha256")?
                != context.configuration_sha256(deadline, cancelled)?.as_str()
        {
            return Err(SourceCommandError::Denied(
                "PG private Text read profile source or owner context differs",
            ));
        }
        if operation == "describe" {
            cmd::exact_keys(&request, &["operation"])?;
            worker
                .finish(deadline, cancelled)
                .map_err(|_| SourceCommandError::Denied("PG private Text read describe FINAL"))?;
            profile.verify(deadline, cancelled)?;
            if context.private_owner_scope_binding(deadline, cancelled)? != owner_context_binding
                || read_absolute(
                    invocation_path,
                    uid,
                    true,
                    invocation_raw.len(),
                    deadline,
                    cancelled,
                )? != invocation_raw
                || runtime_observation.current_digest(deadline, cancelled)? != executable_digest
            {
                return Err(SourceCommandError::Conflict(
                    "PG private Text read description inputs changed",
                ));
            }
            return Ok(json!({
                "schema_version":"tos_native_pg_private_text_description_v1",
                "authentication":"local-unix-account",
                "supported_operations":["native-text.read-private"],
                "command_operations":["describe","native-text.read-private"],
                "grants_admission":false,
                "content_disclosure":"withheld"
            }));
        }
        if operation != "native-text.read-private" {
            return Err(SourceCommandError::Denied(
                "PG private Text read operation is not selected",
            ));
        }
        let result = pg_private_text_read(
            &request,
            &profile,
            &context,
            &scope,
            &selection,
            &store,
            &first_epoch,
            &schema_members,
            &mut db,
            &mut worker,
            deadline,
            cancelled,
        )?;
        if context.private_owner_scope_binding(deadline, cancelled)? != owner_context_binding
            || read_absolute(&grant_path, uid, true, raw.len(), deadline, cancelled)? != raw
            || read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )? != invocation_raw
            || runtime_observation.current_digest(deadline, cancelled)? != executable_digest
        {
            return Err(SourceCommandError::Conflict(
                "PG private Text read invocation or authority changed",
            ));
        }
        return Ok(result);
    }
    let (grant, owner_configuration) =
        select_grant(&context, &family, &grant_path, &worker, deadline, cancelled)?;
    let grant_raw = grant.raw().to_vec();
    if cmd::text(grant.config(), "schema_version")? != family {
        return Err(SourceCommandError::Conflict("Text owner family changed"));
    }
    if operation == "describe" {
        cmd::exact_keys(&request, &["operation"])?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("PostgreSQL Text describe worker FINAL"))?;
        // FINAL settles the selected schema-work record; it does not freeze
        // the protected local delegation. Re-select the grant at the
        // disclosure boundary, matching the maintained native Text route.
        let (current_grant, current_configuration) =
            select_grant(&context, &family, &grant_path, &worker, deadline, cancelled)?;
        if current_grant.raw() != grant.raw() || current_configuration != owner_configuration {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text owner grant changed",
            ));
        }
        if context.private_owner_scope_binding(deadline, cancelled)? != owner_context_binding {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text context changed",
            ));
        }
        if read_absolute(
            invocation_path,
            uid,
            true,
            invocation_raw.len(),
            deadline,
            cancelled,
        )? != invocation_raw
            || runtime_observation.current_digest(deadline, cancelled)? != executable_digest
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text invocation or executable changed",
            ));
        }
        let command_operations = json!(["describe", "prepare-create", grant.operation()]);
        return Ok(json!({
            "schema_version":"tos_native_pg_private_text_description_v1",
            "authentication":"local-unix-account",
            "owner_configuration":owner_configuration,
            "supported_operations":[grant.operation()],
            "command_operations":command_operations,
            "grants_admission":false,
            "content_disclosure":"withheld"
        }));
    }
    let preparing_text = operation == "prepare-create";
    if operation != grant.operation() && !preparing_text {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Text independently selected operation",
        ));
    }

    let source_path = cmd::text(grant.config(), "source_path")?.to_owned();
    let target_ref = context.private_package_target_ref(&source_path)?;
    let target_current = db
        .read_private_owner_current(&scope, &target_ref)
        .map_err(pg::source_error)?;
    if target_current
        .as_ref()
        .is_some_and(|snapshot| snapshot.family != PrivateOwnerPackageFamilyV1::Text)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text target family differs",
        ));
    }
    let empty_tokens = BTreeSet::new();
    let identity_baseline = db
        .read_private_text_identity_matches(
            &scope,
            &empty_tokens,
            None,
            &BTreeMap::new(),
            deadline,
            cancelled,
        )
        .map_err(pg::source_error)?;
    let private_generation = identity_baseline.private_audit_generation;
    if target_current
        .as_ref()
        .is_some_and(|snapshot| snapshot.private_audit_generation != private_generation)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text target changed during identity selection",
        ));
    }

    // Hydrate only packages named by this protected grant. Private Text
    // identity checks use the scoped point index below, not a package census.
    let mut dependency_targets = BTreeSet::new();
    collect_private_target_refs(
        grant.config(),
        context.private_prefix(),
        &mut dependency_targets,
    );
    // `source_path` names the package being created as well as a predecessor
    // when revising it. A new target has no current snapshot to hydrate, so it
    // is an expected absence rather than a missing dependency.
    dependency_targets.remove(&target_ref);
    if let Some(snapshot) = &target_current {
        dependency_targets.insert(snapshot.target_ref.clone());
    }
    let mut private_members = BTreeMap::new();
    let mut dependency_members = BTreeMap::new();
    if dependency_targets.len() > 64 {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private Text dependency target budget",
        ));
    }
    let mut loaded_targets = BTreeSet::new();
    let mut private_dependency_bytes = 0u64;
    for target in dependency_targets {
        let snapshot = if target_current
            .as_ref()
            .is_some_and(|current| current.target_ref == target)
        {
            target_current.clone()
        } else {
            db.read_private_owner_current(&scope, &target)
                .map_err(pg::source_error)?
        }
        .ok_or(SourceCommandError::Conflict(
            "PostgreSQL private Text dependency absent",
        ))?;
        let target = snapshot.target_ref.clone();
        hydrate_private_snapshot(
            snapshot,
            &scope,
            &target,
            private_generation,
            max_total_bytes,
            max_member_bytes,
            &mut private_dependency_bytes,
            &mut private_members,
            &mut dependency_members,
        )?;
        loaded_targets.insert(target);
    }

    // Derived layers may refer to older private predecessor packages through
    // the predecessor metadata itself, not only through the protected grant.
    // Discover each exact missing owner-local member, close that PG session,
    // and hydrate only its private package before trying the selection again.
    let selected_source_for_identity = loop {
        let (resolved, _selection_readset) = db
            .with_source_revision_session(
                &store,
                &selection.domain,
                &selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
                |session| {
                    if !same_epoch(&first_epoch, &epoch(session)) {
                        return Err(DurableError::Conflict(
                            "PostgreSQL Text source epoch changed",
                        ));
                    }
                    let selected_schemas =
                        selected_contracts(session, &family, deadline, cancelled)
                            .map_err(source_command_error)?;
                    if selected_schemas != schema_members {
                        return Err(DurableError::Conflict("PostgreSQL Text contracts changed"));
                    }
                    resolve_selected_source(
                        &grant,
                        &context,
                        session,
                        &schema_members,
                        &private_members,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)
                },
            )
            .map_err(pg::source_error)?;
        let Some(reference) = resolved.missing_private_reference else {
            break resolved.selected_members;
        };
        let target = context.private_package_target_ref(&reference)?;
        if loaded_targets.contains(&target) {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL private Text package lacks a referenced member",
            ));
        }
        if loaded_targets.len() >= 64 {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL private Text dependency target budget",
            ));
        }
        let snapshot = db
            .read_private_owner_current(&scope, &target)
            .map_err(pg::source_error)?
            .ok_or(SourceCommandError::Conflict(
                "PostgreSQL private Text transitive dependency absent",
            ))?;
        let member = reference
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Invalid(
                "PostgreSQL private Text dependency member",
            ))?;
        if !snapshot.members.contains_key(member) {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL private Text package lacks a referenced member",
            ));
        }
        hydrate_private_snapshot(
            snapshot,
            &scope,
            &target,
            private_generation,
            max_total_bytes,
            max_member_bytes,
            &mut private_dependency_bytes,
            &mut private_members,
            &mut dependency_members,
        )?;
        loaded_targets.insert(target);
    };

    // Reuse only owner-private metadata members selected by the maintained
    // semantic resolver as exact predecessor ancestry. The broker excludes a
    // contribution only when both this logical path and its digest match.
    let private_ancestry = selected_source_for_identity
        .iter()
        .filter(|(path, _)| {
            context.is_owner_private_reference(path)
                && source_text_identity::is_candidate(path.rsplit('/').next().unwrap_or(path))
        })
        .map(|(path, bytes)| (path.clone(), Digest256::of_bytes(bytes)))
        .collect::<BTreeMap<_, _>>();
    let delegated_ids = grant.delegated_ids()?;
    let delegated_tokens = delegated_ids.iter().cloned().collect::<BTreeSet<_>>();
    let private_identity = db
        .read_private_text_identity_matches(
            &scope,
            &delegated_tokens,
            Some(&target_ref),
            &private_ancestry,
            deadline,
            cancelled,
        )
        .map_err(pg::source_error)?;
    if private_identity.private_audit_generation != private_generation {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text identity generation changed",
        ));
    }
    if !private_identity.matches.is_empty() {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Text identity occupied",
        ));
    }

    let ((prepared, dependency_members, publication), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if !same_epoch(&first_epoch, &epoch(session)) {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Text source epoch changed",
                    ));
                }
                let selected_schemas = selected_contracts(session, &family, deadline, cancelled)
                    .map_err(source_command_error)?;
                if selected_schemas != schema_members {
                    return Err(DurableError::Conflict("PostgreSQL Text contracts changed"));
                }
                let observations =
                    session.lookup_text_identity_owners(&delegated_tokens, deadline, cancelled)?;
                if observations.len() != delegated_tokens.len()
                    || observations.keys().ne(delegated_tokens.iter())
                {
                    return Err(DurableError::Corrupt(
                        "PostgreSQL public Text identity point result shape",
                    ));
                }
                if observations
                    .values()
                    .any(|observation| !observation.owners.is_empty())
                {
                    return Err(DurableError::Source(SourceCommandError::Conflict(
                        "native Text delegated identity already owned publicly",
                    )));
                }
                let owners_by_token = observations
                    .iter()
                    .map(|(token, observation)| (token.clone(), observation.owners.clone()))
                    .collect::<BTreeMap<_, _>>();
                let generations_by_token = observations
                    .iter()
                    .map(|(token, observation)| (token.clone(), observation.generation))
                    .collect::<BTreeMap<_, _>>();
                let public_identity_digest = source_text_identity::point_identity_snapshot_digest(
                    session.selection(),
                    &delegated_tokens,
                    &owners_by_token,
                    &generations_by_token,
                    &BTreeMap::new(),
                )
                .map_err(DurableError::Source)?;
                let identity_digest =
                    crate::source_cohort_private_text::private_text_identity_predicate_digest(
                        public_identity_digest,
                        private_generation,
                        &delegated_tokens,
                        &target_ref,
                        &private_ancestry,
                    )?;
                let publication = {
                    let input = super::pg_growth::PgGrowthCurrentInput::new(session, 1, 8192)
                        .map_err(|_| DurableError::Invalid("PostgreSQL Text publication input"))?;
                    context
                        .install_pg_publication(&input, deadline, cancelled)
                        .map_err(DurableError::Source)?
                };
                let owner_target = context
                    .private_package_target_for_owner_route(&source_path)
                    .map_err(DurableError::Source)?;
                let selected_source = resolve_selected_source(
                    &grant,
                    &context,
                    session,
                    &schema_members,
                    &private_members,
                    &mut worker,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                if selected_source.missing_private_reference.is_some() {
                    return Err(DurableError::Conflict(
                        "PostgreSQL private Text dependency changed after selection",
                    ));
                }
                let selected_source_members = selected_source.selected_members;
                if selected_source_members != selected_source_for_identity {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Text selected ancestry changed",
                    ));
                }
                let selected_source_bytes = selected_source_members
                    .values()
                    .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64));
                if selected_source_members.len() > max_members
                    || selected_source_bytes.is_none_or(|sum| sum > max_total_bytes)
                    || selected_source_members
                        .values()
                        .any(|raw| raw.len() as u64 > max_member_bytes)
                {
                    return Err(DurableError::Source(SourceCommandError::Unsupported(
                        "PostgreSQL private Text selected source budget",
                    )));
                }
                context
                    .install_pg_identity_snapshot(identity_digest)
                    .map_err(DurableError::Source)?;
                context
                    .extend_pg_selected_members(selected_source_members.clone())
                    .map_err(DurableError::Source)?;
                let retained = target_current.as_ref().map(|snapshot| &snapshot.members);
                let prepared = if preparing_text {
                    let exclude = target_current.as_ref().map(|_| owner_target.as_path());
                    match grant {
                        SelectedGrant::Initial(grant) => {
                            cmd::exact_keys(&request, &["operation"])
                                .map_err(DurableError::Source)?;
                            initial::prepare_pg_initial_text_preview(
                                context,
                                publication.clone(),
                                grant,
                                &software,
                                &components,
                                &mut worker,
                                exclude,
                                deadline,
                                cancelled,
                            )
                            .map(PreparedPrivateTextOperation::InitialPreview)
                        }
                        SelectedGrant::Derived(grant) => {
                            cmd::exact_keys(&request, &["operation"])
                                .map_err(DurableError::Source)?;
                            derived::prepare_pg_derived_text_preview(
                                context,
                                publication.clone(),
                                grant,
                                &software,
                                &components,
                                &mut worker,
                                exclude,
                                deadline,
                                cancelled,
                            )
                            .map(PreparedPrivateTextOperation::DerivedPreview)
                        }
                        SelectedGrant::Unit(grant) => unit::prepare_pg_text_unit_preview(
                            context,
                            publication.clone(),
                            grant,
                            &request,
                            exclude,
                            &software,
                            &components,
                            &mut worker,
                            deadline,
                            cancelled,
                        )
                        .map(PreparedPrivateTextOperation::Preview),
                    }
                } else {
                    prepare_package(
                        grant,
                        context,
                        publication.clone(),
                        &request,
                        retained,
                        &software,
                        &components,
                        &mut worker,
                        deadline,
                        cancelled,
                    )
                    .map(PreparedPrivateTextOperation::Commit)
                }
                .map_err(DurableError::Source)?;
                let mut dependency_members = dependency_members.clone();
                for (path, bytes) in &schema_members {
                    dependency_members.insert(path.clone(), bytes.clone());
                }
                for (path, bytes) in &selected_source_members {
                    dependency_members.insert(path.clone(), bytes.clone());
                }
                let dependency_bytes = dependency_members
                    .values()
                    .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64));
                if dependency_members.len() > max_members
                    || dependency_bytes.is_none_or(|sum| sum > max_total_bytes)
                    || dependency_members
                        .values()
                        .any(|raw| raw.len() as u64 > max_member_bytes)
                {
                    return Err(DurableError::Source(SourceCommandError::Unsupported(
                        "PostgreSQL private Text dependency selection budget",
                    )));
                }
                Ok((prepared, dependency_members, publication))
            },
        )
        .map_err(pg::source_error)?;

    let preview = match prepared {
        PreparedPrivateTextOperation::InitialPreview(preview) => Some((
            preview.owner_configuration,
            preview.expected_dependencies,
            preview.source_path,
            JsonValue::Null,
            Some(JsonValue::Array(vec![preview.native_binding])),
        )),
        PreparedPrivateTextOperation::DerivedPreview(preview) => Some((
            preview.owner_configuration,
            preview.expected_dependencies,
            preview.source_path,
            JsonValue::Null,
            None,
        )),
        PreparedPrivateTextOperation::Preview(preview) => Some((
            preview.owner_configuration,
            preview.expected_dependencies,
            preview.source_path,
            preview.prepared_files,
            Some(preview.native_bindings),
        )),
        PreparedPrivateTextOperation::Commit(prepared) => {
            let prepared_replayed = prepared.replayed();
            let source_binding =
                private_text_source_binding(&readset, &dependency_members, private_generation)
                    .map_err(pg::source_error)?;
            let mut capability = PrivateTextPackageCapabilityV1::from_prepared(
                prepared,
                scope,
                target_current,
                source_binding,
                &request,
                &request_raw,
                &software,
                &components,
                invocation_path,
                invocation_raw,
                uid,
                &runtime_observation,
                executable_digest,
                deadline,
                cancelled,
            )
            .map_err(pg::source_error)?;
            let replay = db
                .commit_private_owner_package(&mut capability)
                .map_err(pg::source_error)?;
            return Ok(json!({
                "schema_version":"tos_native_pg_private_text_result_v1",
                "authentication":"local-unix-account",
                "command_id":cmd::text(&request, "command_id")?,
                "commit_seq":replay.commit_seq,
                "package_revision":replay.package_revision,
                "package_sha256":replay.package_digest.to_prefixed(),
                "receipt_sha256":Digest256::of_bytes(&replay.response_bytes).to_prefixed(),
                "replayed":prepared_replayed || replay.replayed,
                "grants_admission":false,
                "content_disclosure":"withheld"
            }));
        }
    };
    if let Some((
        owner_configuration,
        expected_dependencies,
        source_path,
        prepared_files,
        native_bindings,
    )) = preview
    {
        // A preview has no durable commit to carry the PG readset lease. Reopen
        // the selected source epoch and recheck its source, rights, and rule
        // generations before returning the two owner-computed digests.
        db.with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if !same_epoch(&first_epoch, &epoch(session)) {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Text source epoch changed during preparation",
                    ));
                }
                let current_schemas = selected_contracts(session, &family, deadline, cancelled)
                    .map_err(source_command_error)?;
                if current_schemas != schema_members {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Text contracts changed during preparation",
                    ));
                }
                let input = super::pg_growth::PgGrowthCurrentInput::new(session, 1, 8192)
                    .map_err(|_| DurableError::Invalid("PostgreSQL Text publication input"))?;
                publication
                    .verify_from_input(&input, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                Ok(())
            },
        )
        .map_err(pg::source_error)?;
        if read_absolute(&grant_path, uid, true, grant_raw.len(), deadline, cancelled)? != grant_raw
            || read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )? != invocation_raw
            || runtime_observation.current_digest(deadline, cancelled)? != executable_digest
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Text preparation inputs changed",
            ));
        }
        let prepared_files = pg::value(&prepared_files)?;
        let mut response = json!({
            "schema_version":"tos_native_pg_text_preparation_v1",
            "authentication":"local-unix-account",
            "owner_configuration":owner_configuration,
            "expected_dependencies":expected_dependencies,
            "source_path":source_path,
            "prepared_files":prepared_files,
            "grants_admission":false,
            "content_disclosure":"withheld"
        });
        if let Some(bindings) = native_bindings {
            response["native_bindings"] = pg::value(&bindings)?;
        }
        return Ok(response);
    }
    Err(SourceCommandError::Invalid(
        "PostgreSQL Text prepared operation result",
    ))
}
