//! Dedicated PG-backed native translation Alignment route. Alignment keeps
//! its own owner grant and package family while sharing the private package
//! broker and the bounded Text identity index.

use super::pg_claims as pg;
use crate::durable_private_packages::{
    PrivateOwnerPackageFamilyV1, PrivateOwnerPackageHeaderV1, PrivateOwnerPackageSnapshotV1,
    PrivateOwnerReadSession, PrivateOwnerScopeV1, PrivateOwnerSourceBindingV1,
    private_owner_dependency_digest, private_owner_readset_digest,
};
use crate::source_cohort::revision::SourceRevisionReadSession;
use crate::source_cohort_private_alignment::PrivateAlignmentPackageCapabilityV1;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_sign_native::{NativeReadKind, SignNativeRead, resolve_owner_alignment};
use crate::source_text_alignment_entry as alignment;
use crate::source_text_owner::{OwnerTextAlignmentSelection, OwnerTextContext, read_absolute};
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const INVOCATION_SCHEMA: &str = "tos_local_pg_private_alignment_invocation_v1";
const OWNER_SCHEMA: &str = "tos_local_native_alignment_owner_v1";
const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const MAX_SCHEMA_FILES: usize = 12;
const MAX_SCHEMA_BYTES: usize = 16_777_216;
const MAX_SCHEMA_FILE_BYTES: u64 = 4_194_304;
const MAX_MEMBERS: usize = 4_096;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 32 * 1024 * 1024;
const MAX_HISTORY_RECORDS: usize = 64;
const MAX_HISTORY_BYTES: usize = 8_388_608;
const MAX_PACKAGE_BYTES: usize = 12 * 1024 * 1024;

const ALIGNMENT_SCHEMAS: [&str; 12] = [
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
    "native-translation-alignment-record-v1.schema.json",
    "translation-alignment-packet-v1.schema.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
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

fn epoch(session: &SourceRevisionReadSession<'_>) -> SelectedEpoch {
    let selected = session.selection();
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

fn same_epoch(left: &SelectedEpoch, right: &SelectedEpoch) -> bool {
    left == right
}

fn read_member(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    max_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Alignment source path"))?;
    session
        .read_current_member(&path, max_bytes, deadline, cancelled)
        .map(|member| member.raw)
        .map_err(pg::source_error)
}

fn selected_contracts(
    session: &mut SourceRevisionReadSession<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for basename in ALIGNMENT_SCHEMAS {
        let path = format!("ToS/contracts/{basename}");
        let raw = read_member(session, &path, MAX_SCHEMA_FILE_BYTES, deadline, cancelled)?;
        total = total
            .checked_add(raw.len())
            .filter(|value| *value <= MAX_SCHEMA_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL Alignment schema byte budget",
            ))?;
        files.insert(path, raw);
    }
    if files.len() > MAX_SCHEMA_FILES || !files.contains_key(CONTEXT_SCHEMA) {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment schema set differs",
        ));
    }
    Ok(files)
}

fn source_files(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    files
        .iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Alignment schema path"))?,
                raw: raw.clone(),
            })
        })
        .collect()
}

#[derive(Default)]
struct PrivateHistorySelection {
    ancestry: BTreeMap<String, Digest256>,
    visited: BTreeSet<String>,
    bytes: usize,
}

fn load_alignment_history_ref(
    session: &mut PrivateOwnerReadSession<'_>,
    context: &OwnerTextContext,
    grant: &OwnerTextAlignmentSelection,
    private_generation: u64,
    selected: &JsonValue,
    ancestry: bool,
    dependency_members: &mut BTreeMap<String, Vec<u8>>,
    history: &mut PrivateHistorySelection,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let path = cmd::text(selected, "record_ref")?.to_owned();
    if !context.is_owner_private_reference(&path) {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Alignment history reference is not owner private",
        ));
    }
    if !history.visited.insert(path.clone()) {
        if ancestry {
            if let Some(raw) = dependency_members.get(&path) {
                history.ancestry.insert(path, Digest256::of_bytes(raw));
            }
        }
        return Ok(());
    }
    if history.visited.len() > MAX_HISTORY_RECORDS {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Alignment history record budget",
        ));
    }
    let (target, member) = path.rsplit_once('/').ok_or(SourceCommandError::Invalid(
        "PostgreSQL Alignment history path",
    ))?;
    context.snapshot(deadline, cancelled)?;
    grant.verify_current(context.account_uid(), deadline, cancelled)?;
    let header = session
        .current_header(target, deadline, cancelled)
        .map_err(pg::source_error)?
        .ok_or(SourceCommandError::Conflict(
            "PostgreSQL Alignment history package absent",
        ))?;
    if header.family != PrivateOwnerPackageFamilyV1::Alignment {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment history package generation or family differs",
        ));
    }
    context.snapshot(deadline, cancelled)?;
    grant.verify_current(context.account_uid(), deadline, cancelled)?;
    let selected_member = session
        .read_current_member(target, member, MAX_HISTORY_BYTES, deadline, cancelled)
        .map_err(pg::source_error)?
        .ok_or(SourceCommandError::Conflict(
            "PostgreSQL Alignment history member absent",
        ))?;
    if selected_member.private_audit_generation != private_generation
        || selected_member.family != PrivateOwnerPackageFamilyV1::Alignment
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment history member generation or family differs",
        ));
    }
    let raw = selected_member.bytes;
    history.bytes = history
        .bytes
        .checked_add(raw.len())
        .filter(|size| *size <= MAX_HISTORY_BYTES)
        .ok_or(SourceCommandError::Unsupported(
            "PostgreSQL Alignment history byte budget",
        ))?;
    let digest = Digest256::of_bytes(&raw);
    let body = cmd::parse(&raw)?;
    if cmd::text(selected, "sha256")? != digest.to_hex()
        || cmd::text(selected, "record_id")? != cmd::text(&body, "record_id")?
        || cmd::integer(selected, "record_version")? != cmd::integer(&body, "record_version")?
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment history reference binding differs",
        ));
    }
    if dependency_members.insert(path.clone(), raw).is_some() {
        return Err(SourceCommandError::Conflict(
            "duplicate PostgreSQL Alignment history member",
        ));
    }
    if ancestry {
        history.ancestry.insert(path.clone(), digest);
    }
    let predecessor = cmd::field(&body, "predecessor")?;
    if predecessor != &JsonValue::Null {
        load_alignment_history_ref(
            session,
            context,
            grant,
            private_generation,
            predecessor,
            true,
            dependency_members,
            history,
            deadline,
            cancelled,
        )?;
    }
    for competing in cmd::array(&body, "competing_records")? {
        load_alignment_history_ref(
            session,
            context,
            grant,
            private_generation,
            competing,
            false,
            dependency_members,
            history,
            deadline,
            cancelled,
        )?;
    }
    let _ = (deadline, cancelled);
    Ok(())
}

fn load_history_closure(
    session: &mut PrivateOwnerReadSession<'_>,
    context: &OwnerTextContext,
    grant: &OwnerTextAlignmentSelection,
    private_generation: u64,
    dependency_members: &mut BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivateHistorySelection> {
    let mut history = PrivateHistorySelection::default();
    let predecessor = cmd::field(&grant.config, "predecessor")?;
    if predecessor != &JsonValue::Null {
        load_alignment_history_ref(
            session,
            context,
            grant,
            private_generation,
            predecessor,
            true,
            dependency_members,
            &mut history,
            deadline,
            cancelled,
        )?;
    }
    for competitor in cmd::array(&grant.config, "competing_records")? {
        load_alignment_history_ref(
            session,
            context,
            grant,
            private_generation,
            competitor,
            false,
            dependency_members,
            &mut history,
            deadline,
            cancelled,
        )?;
    }
    Ok(history)
}

struct SessionAlignmentRead<'s, 'source, 'private> {
    session: &'s mut SourceRevisionReadSession<'source>,
    private_session: &'s mut PrivateOwnerReadSession<'private>,
    expected_epoch: &'s SelectedEpoch,
    private_generation: u64,
    context: &'s OwnerTextContext,
    grant: &'s OwnerTextAlignmentSelection,
    selected_members: BTreeMap<String, Vec<u8>>,
}

impl SignNativeRead for SessionAlignmentRead<'_, '_, '_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.context.snapshot(deadline, cancelled)?;
        self.grant
            .verify_current(self.context.account_uid(), deadline, cancelled)?;
        if !same_epoch(self.expected_epoch, &epoch(self.session)) {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment source epoch changed during read",
            ));
        }
        let private = self.context.is_owner_private_reference(reference);
        let raw = if private {
            let (target, member) =
                reference
                    .rsplit_once('/')
                    .ok_or(SourceCommandError::Invalid(
                        "PostgreSQL Alignment private member path",
                    ))?;
            let selected = self
                .private_session
                .read_current_member(target, member, max_bytes, deadline, cancelled)
                .map_err(pg::source_error)?;
            let Some(selected) = selected else {
                return Err(SourceCommandError::Conflict(
                    "PostgreSQL Alignment private dependency unavailable",
                ));
            };
            if selected.private_audit_generation != self.private_generation
                || !matches!(
                    selected.family,
                    PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
                )
            {
                return Err(SourceCommandError::Conflict(
                    "PostgreSQL Alignment private dependency generation or family differs",
                ));
            }
            selected.bytes
        } else if let Some(raw) = self.selected_members.get(reference) {
            raw.clone()
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
                "PostgreSQL Alignment selected member byte budget",
            ));
        }
        if self
            .selected_members
            .get(reference)
            .is_some_and(|previous| previous != &raw)
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment selected member changed during read",
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
        self.context.snapshot(deadline, cancelled)?;
        self.grant
            .verify_current(self.context.account_uid(), deadline, cancelled)?;
        if cancelled.load(std::sync::atomic::Ordering::Acquire)
            || Instant::now() >= deadline
            || !same_epoch(self.expected_epoch, &epoch(self.session))
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment read currentness changed",
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

struct SelectedAlignmentInputs {
    selected_members: BTreeMap<String, Vec<u8>>,
}

enum FinalAlignmentPreparation {
    Preview(alignment::NativeAlignmentPreview),
    Package(alignment::PreparedNativeAlignmentPackage),
}

fn resolve_selected_alignment(
    grant: &OwnerTextAlignmentSelection,
    context: &OwnerTextContext,
    session: &mut SourceRevisionReadSession<'_>,
    private_session: &mut PrivateOwnerReadSession<'_>,
    expected_epoch: &SelectedEpoch,
    schema_members: &BTreeMap<String, Vec<u8>>,
    private_generation: u64,
    worker: &mut dyn CutSchemaExecutor,
    verify_content: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedAlignmentInputs> {
    let mut reader = SessionAlignmentRead {
        session,
        private_session,
        expected_epoch,
        private_generation,
        context,
        grant,
        selected_members: schema_members.clone(),
    };
    let result = resolve_owner_alignment(
        &mut reader,
        worker,
        cmd::array(cmd::field(&grant.config, "native_bindings")?, "source")?,
        cmd::array(cmd::field(&grant.config, "native_bindings")?, "target")?,
        verify_content,
        deadline,
        cancelled,
    );
    result?;
    Ok(SelectedAlignmentInputs {
        selected_members: reader.selected_members,
    })
}

fn private_alignment_identity_digest(
    public_snapshot: Digest256,
    private_generation: u64,
    tokens: &BTreeSet<String>,
    target_ref: &str,
    allowed_ancestry: &BTreeMap<String, Digest256>,
) -> SourceCommandResult<Digest256> {
    if RelativePath::parse(target_ref).is_err()
        || !target_ref.starts_with("ToS/source-witnesses/owner-local/")
        || tokens.iter().any(|token| token.is_empty())
        || allowed_ancestry.keys().any(|path| {
            RelativePath::parse(path).is_err()
                || !path.starts_with("ToS/source-witnesses/owner-local/")
        })
    {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Alignment identity predicate",
        ));
    }
    let value = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_private_native_alignment_identity_predicate_v1"),
        ),
        (
            "public_snapshot_sha256",
            cmd::string(&public_snapshot.to_prefixed()),
        ),
        ("private_audit_generation", cmd::number(private_generation)),
        (
            "requested_tokens",
            JsonValue::Array(tokens.iter().map(|token| cmd::string(token)).collect()),
        ),
        ("excluded_target", cmd::string(target_ref)),
        (
            "allowed_private_ancestry",
            cmd::object(
                allowed_ancestry
                    .iter()
                    .map(|(path, digest)| (path.as_str(), cmd::string(&digest.to_prefixed())))
                    .collect(),
            ),
        ),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?))
}

fn alignment_identity_tokens(
    grant: &OwnerTextAlignmentSelection,
) -> SourceCommandResult<BTreeSet<String>> {
    [
        "record_id",
        "alignment_id",
        "claim_id",
        "provenance_event_id",
    ]
    .iter()
    .map(|key| cmd::text(&grant.config, key).map(str::to_owned))
    .collect::<SourceCommandResult<BTreeSet<_>>>()
}

fn alignment_source_binding(
    readset: &crate::source_cohort::revision::SourceRevisionReadSet,
    dependency_members: &BTreeMap<String, Vec<u8>>,
    private_owner_audit_generation: u64,
) -> crate::DurableResult<PrivateOwnerSourceBindingV1> {
    let selected = readset.selection();
    let dependencies = dependency_members
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    Ok(PrivateOwnerSourceBindingV1 {
        head_seq: selected.head_seq,
        source_epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
        contract_digest: selected.contract,
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        readset_digest: private_owner_readset_digest(readset)?,
        dependency_digest: private_owner_dependency_digest(&dependencies)?,
        private_owner_audit_generation,
    })
}

fn invocation_budgets(invocation: &Value) -> SourceCommandResult<()> {
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
    if super::capped(budgets, "max_members", MAX_MEMBERS as u64)? == 0
        || super::capped(budgets, "max_total_bytes", MAX_TOTAL_BYTES)? == 0
        || super::capped(budgets, "max_member_bytes", MAX_MEMBER_BYTES)? == 0
    {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Alignment empty budget",
        ));
    }
    let _ = super::capped(budgets, "max_schema_receipts", 128)?;
    let _ = super::capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = super::capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = super::capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok(())
}

fn budget_members(
    members: &BTreeMap<String, Vec<u8>>,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    error: &'static str,
) -> SourceCommandResult<()> {
    let total = members
        .values()
        .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64));
    if members.len() > max_members
        || total.is_none_or(|value| value > max_total_bytes)
        || members
            .values()
            .any(|raw| raw.len() as u64 > max_member_bytes)
    {
        return Err(SourceCommandError::Unsupported(error));
    }
    Ok(())
}

fn validate_alignment_target_header(
    header: &PrivateOwnerPackageHeaderV1,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
) -> SourceCommandResult<()> {
    if header.scope != *scope || header.target_ref != target_ref {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment target header scope differs",
        ));
    }
    if header.current.as_ref().is_some_and(|current| {
        current.family != PrivateOwnerPackageFamilyV1::Alignment || current.revision != 1
    }) {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private Alignment target family or immutable revision differs",
        ));
    }
    Ok(())
}

fn read_alignment_target_after_selection(
    session: &mut PrivateOwnerReadSession<'_>,
    target_ref: &str,
    selected_header: &PrivateOwnerPackageHeaderV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<PrivateOwnerPackageSnapshotV1>> {
    let current_header = session
        .current_header(target_ref, deadline, cancelled)
        .map_err(pg::source_error)?;
    let observed = PrivateOwnerPackageHeaderV1 {
        scope: session.scope().clone(),
        target_ref: target_ref.to_owned(),
        current: current_header,
        private_audit_generation: session.private_audit_generation(),
    };
    if observed != *selected_header {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment target changed after rights selection",
        ));
    }
    let snapshot = session
        .read_alignment_output_package(target_ref, MAX_PACKAGE_BYTES, deadline, cancelled)
        .map_err(pg::source_error)?;
    if snapshot.as_ref().is_some_and(|current| {
        current.scope != *session.scope()
            || current.target_ref != target_ref
            || current.family != PrivateOwnerPackageFamilyV1::Alignment
            || current.revision != 1
            || current.private_audit_generation != selected_header.private_audit_generation
            || selected_header
                .current
                .as_ref()
                .is_none_or(|header| current.package_digest != header.package_digest)
    }) {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment target changed after rights selection",
        ));
    }
    Ok(snapshot)
}

fn invocation_changed(
    invocation_path: &Path,
    invocation_raw: &[u8],
    uid: u32,
    runtime: &crate::source_serialization::ExecutableObservation,
    executable_digest: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
        || runtime.current_digest(deadline, cancelled)? != executable_digest
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment invocation or executable changed",
        ));
    }
    Ok(())
}

fn final_grant_current(
    context: &OwnerTextContext,
    grant_path: &Path,
    original: &OwnerTextAlignmentSelection,
    original_configuration: &str,
    worker: &CutWorkerSchemaExecutor,
    owner_context_binding: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let current = OwnerTextAlignmentSelection::select(context, grant_path, deadline, cancelled)?;
    let current_configuration =
        alignment::selected_configuration(context, &current, worker, deadline, cancelled)?;
    if current.raw != original.raw
        || current_configuration != original_configuration
        || context.private_owner_scope_binding(deadline, cancelled)? != owner_context_binding
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment owner configuration changed",
        ));
    }
    Ok(())
}

fn replay_identity_generation(
    session: &mut PrivateOwnerReadSession<'_>,
    target_ref: &str,
    selected_header: &PrivateOwnerPackageHeaderV1,
    private_generation: u64,
    request: &JsonValue,
    raw_request: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<u64>> {
    let metadata = session
        .command_metadata(
            target_ref,
            cmd::text(request, "command_id")?,
            deadline,
            cancelled,
        )
        .map_err(pg::source_error)?;
    if metadata.header != *selected_header
        || metadata.header.private_audit_generation != private_generation
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment replay history changed",
        ));
    }
    let command_id = cmd::text(request, "command_id")?;
    let request_digest = Digest256::of_bytes(&cmd::canonical(request)?);
    let raw_request_digest = Digest256::of_bytes(raw_request);
    let owner_configuration_digest =
        Digest256::from_prefixed(cmd::text(request, "expected_configuration")?).map_err(|_| {
            SourceCommandError::Invalid("PostgreSQL Alignment expected configuration")
        })?;
    let Some(receipt) = metadata.receipt else {
        return Ok(None);
    };
    let current = metadata.header.current.as_ref();
    if receipt.command_id != command_id
        || receipt.target_digest != Digest256::of_bytes(target_ref.as_bytes())
        || receipt.family != PrivateOwnerPackageFamilyV1::Alignment
        || receipt.owner_context_binding != session.scope().context_binding
        || receipt.request_digest != request_digest
        || receipt.raw_request_digest != raw_request_digest
        || receipt.owner_configuration_digest != owner_configuration_digest
        || receipt.private_audit_generation_after > private_generation
        || receipt.source_private_generation >= receipt.private_audit_generation_after
        || current.is_none_or(|current| {
            current.family != PrivateOwnerPackageFamilyV1::Alignment
                || current.revision != 1
                || current.package_digest != receipt.package_digest
                || current.command_id != command_id
                || current.commit_seq != receipt.commit_seq
        })
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment replay request binding differs",
        ));
    }
    Ok(Some(receipt.source_private_generation))
}

fn operation_list(operation: &str) -> &'static str {
    if operation == "alignment.create" {
        "prepare-create"
    } else {
        "prepare-revise"
    }
}

/// Execute one explicitly selected PG Alignment owner invocation. Source
/// members and public identity points are read through the held source epoch;
/// exact private package references come only from the owner-scoped broker.
#[allow(clippy::too_many_arguments)]
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
    if super::text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private Alignment invocation profile",
        ));
    }
    invocation_budgets(invocation)?;
    let selection = pg::selected_source(invocation)?;
    let _ = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let executable_digest = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    if runtime.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment executable identity",
        ));
    }
    let (software, components) = pg::software_selection(invocation, deadline, cancelled)?;
    let component_members = components.members().collect::<Vec<_>>();
    if component_members.len() > MAX_MEMBERS
        || component_members
            .iter()
            .any(|member| member.size_bytes > MAX_MEMBER_BYTES)
        || component_members
            .iter()
            .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
            .is_none_or(|sum| sum > MAX_TOTAL_BYTES)
    {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Alignment software budget",
        ));
    }
    let request_raw = pg::read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let operation = cmd::text(&request, "operation")?.to_owned();
    let context_path = super::absolute(super::text(invocation, "owner_context")?)?;
    let grant_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let grant_raw = read_absolute(&grant_path, uid, true, 1_048_576, deadline, cancelled)?;
    let owner_hint = cmd::parse(&grant_raw)?;
    if cmd::text(&owner_hint, "schema_version")? != OWNER_SCHEMA {
        return Err(SourceCommandError::Denied(
            "PostgreSQL private Alignment owner family",
        ));
    }

    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Alignment source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected Alignment PostgreSQL"))?,
    );
    let mut source_db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected Alignment PostgreSQL"))?,
    );
    let ((first_epoch, schema_members), _) = source_db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected_epoch = epoch(session);
                if selected_epoch.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Alignment source contract differs",
                    ));
                }
                let schemas = selected_contracts(session, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                Ok((selected_epoch, schemas))
            },
        )
        .map_err(pg::source_error)?;
    budget_members(
        &schema_members,
        MAX_MEMBERS,
        MAX_TOTAL_BYTES,
        MAX_MEMBER_BYTES,
        "PostgreSQL Alignment schema budget",
    )?;
    let worker_sources = source_files(&schema_members)?;
    let mut worker = super::revisions::selected_schema_from_context(
        invocation,
        &worker_sources,
        SourceRevision(first_epoch.source_revision),
        deadline,
        cancelled,
    )?;
    let context_schema = schema_members
        .get(CONTEXT_SCHEMA)
        .ok_or(SourceCommandError::Conflict(
            "PostgreSQL Alignment context schema absent",
        ))?;
    let (context, _) = OwnerTextContext::select_pg(
        &context_path,
        context_schema,
        &mut worker,
        deadline,
        cancelled,
    )?;
    context.install_pg_selected_members(schema_members.clone())?;
    let owner_context_binding = context.private_owner_scope_binding(deadline, cancelled)?;
    let scope = PrivateOwnerScopeV1::for_context(&selection.domain, owner_context_binding)
        .map_err(pg::source_error)?;
    let grant = OwnerTextAlignmentSelection::select(&context, &grant_path, deadline, cancelled)?;
    if cmd::text(&grant.config, "schema_version")? != OWNER_SCHEMA {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment owner family changed",
        ));
    }
    let owner_configuration =
        alignment::selected_configuration(&context, &grant, &worker, deadline, cancelled)?;
    if request_schema(&request)? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid(
            "native Alignment request schema",
        ));
    }
    if !matches!(
        operation.as_str(),
        "describe"
            | "prepare-create"
            | "prepare-revise"
            | "alignment.create"
            | "alignment.revise"
            | "inspect"
            | "inspect-version"
            | "inspect-recovery"
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private Alignment operation",
        ));
    }
    if matches!(operation.as_str(), "alignment.create" | "alignment.revise")
        && operation != grant.operation
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Alignment independently selected operation",
        ));
    }
    if matches!(operation.as_str(), "prepare-create" | "prepare-revise")
        && operation != operation_list(grant.operation)
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Alignment prepare operation",
        ));
    }

    let source_path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target_ref = context.private_package_target_ref(&source_path)?;
    let target_header = db
        .with_private_owner_read_session(&scope, deadline, cancelled, |session, _, _| {
            let current = session.current_header(&target_ref, deadline, cancelled)?;
            Ok(PrivateOwnerPackageHeaderV1 {
                scope: scope.clone(),
                target_ref: target_ref.clone(),
                current,
                private_audit_generation: session.private_audit_generation(),
            })
        })
        .map_err(pg::source_error)?;
    validate_alignment_target_header(&target_header, &scope, &target_ref)?;
    let private_generation = target_header.private_audit_generation;

    // These two owner views need target metadata only. In particular, they do
    // not hydrate any private package member or representation bytes.
    if operation == "describe" {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        let ((describe_epoch, describe_schemas), _) = source_db
            .with_source_revision_session(
                &store,
                &selection.domain,
                &selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
                |session| {
                    let selected_epoch = epoch(session);
                    let schemas = selected_contracts(session, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    Ok((selected_epoch, schemas))
                },
            )
            .map_err(pg::source_error)?;
        if !same_epoch(&first_epoch, &describe_epoch) || describe_schemas != schema_members {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment description selection changed",
            ));
        }
        worker.finish(deadline, cancelled).map_err(|_| {
            SourceCommandError::Denied("PostgreSQL Alignment describe worker FINAL")
        })?;
        final_grant_current(
            &context,
            &grant_path,
            &grant,
            &owner_configuration,
            &worker,
            owner_context_binding,
            deadline,
            cancelled,
        )?;
        invocation_changed(
            invocation_path,
            invocation_raw,
            uid,
            &runtime,
            executable_digest,
            deadline,
            cancelled,
        )?;
        return Ok(json!({
            "schema_version":"tos_native_pg_private_alignment_description_v1",
            "authentication":"local-unix-account",
            "owner_configuration":owner_configuration,
            "target_exists":target_header.current.is_some(),
            "supported_operations":[grant.operation],
            "command_operations":["describe",operation_list(grant.operation),grant.operation,"inspect","inspect-version","inspect-recovery"],
            "grants_admission":false,
            "assessment_applied":false,
            "aligner_executed":false,
            "content_disclosure":"withheld"
        }));
    }

    if operation == "inspect-recovery" {
        cmd::exact_keys(&request, &["schema_version", "operation", "command_id"])?;
        let command_id = cmd::text(&request, "command_id")?;
        if command_id.is_empty() || command_id.len() > 256 {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Alignment recovery command ID",
            ));
        }
        let metadata = db
            .with_private_owner_read_session(&scope, deadline, cancelled, |session, _, _| {
                session.command_metadata(&target_ref, command_id, deadline, cancelled)
            })
            .map_err(pg::source_error)?;
        if metadata.header != target_header {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment recovery target changed",
            ));
        }
        let recovery = match metadata.receipt {
            Some(receipt) => {
                let current = metadata.header.current.as_ref();
                let owner_configuration_digest = Digest256::from_prefixed(&owner_configuration)
                    .map_err(|_| {
                        SourceCommandError::Invalid(
                            "PostgreSQL Alignment owner configuration digest",
                        )
                    })?;
                if receipt.command_id != command_id
                    || receipt.target_digest != Digest256::of_bytes(target_ref.as_bytes())
                    || receipt.family != PrivateOwnerPackageFamilyV1::Alignment
                    || receipt.owner_context_binding != scope.context_binding
                    || receipt.owner_configuration_digest != owner_configuration_digest
                    || receipt.private_audit_generation_after > private_generation
                    || receipt.source_private_generation >= receipt.private_audit_generation_after
                    || current.is_none_or(|current| {
                        current.family != PrivateOwnerPackageFamilyV1::Alignment
                            || current.revision != 1
                            || current.package_digest != receipt.package_digest
                            || current.command_id != command_id
                            || current.commit_seq != receipt.commit_seq
                    })
                {
                    return Err(SourceCommandError::Conflict(
                        "PostgreSQL Alignment recovery receipt differs",
                    ));
                }
                "committed"
            }
            None if metadata.header.current.is_none() => "absent",
            None => {
                return Err(SourceCommandError::Conflict(
                    "PostgreSQL Alignment target exists without selected command",
                ));
            }
        };
        worker.finish(deadline, cancelled).map_err(|_| {
            SourceCommandError::Denied("PostgreSQL Alignment recovery worker FINAL")
        })?;
        final_grant_current(
            &context,
            &grant_path,
            &grant,
            &owner_configuration,
            &worker,
            owner_context_binding,
            deadline,
            cancelled,
        )?;
        invocation_changed(
            invocation_path,
            invocation_raw,
            uid,
            &runtime,
            executable_digest,
            deadline,
            cancelled,
        )?;
        return Ok(json!({
            "schema_version":"tos_local_native_alignment_result_v1",
            "authentication":"local-unix-account",
            "owner_configuration":owner_configuration,
            "target_exists":metadata.header.current.is_some(),
            "expected_source":null,
            "expected_revision":null,
            "supported_operations":[grant.operation],
            "command_operations":["describe",operation_list(grant.operation),grant.operation,"inspect","inspect-version","inspect-recovery"],
            "receipt_sha256":null,
            "replayed":false,
            "recovery_state":recovery,
            "recovery_action":if recovery == "absent" { Value::Null } else { json!("retry_exact_original_command") },
            "content_disclosure":"withheld",
            "grants_admission":false,
            "assessment_applied":false,
            "aligner_executed":false
        }));
    }

    let mut dependency_members = BTreeMap::new();
    let is_prepare = matches!(operation.as_str(), "prepare-create" | "prepare-revise");
    if operation == "inspect" || operation == "inspect-version" {
        let delegated_operation = grant.operation;
        if operation == "inspect" {
            cmd::exact_keys(&request, &["schema_version", "operation"])?;
        } else {
            cmd::exact_keys(&request, &["schema_version", "operation", "source"])?;
        }
        let exact = if operation == "inspect-version" {
            Some(cmd::field(&request, "source")?)
        } else {
            None
        };
        let ((selected_epoch, selected), current, history, selected_readset) = db
            .with_private_owner_read_session(
                &scope,
                deadline,
                cancelled,
                |private_session, _, _| {
                    if private_session.private_audit_generation() != private_generation {
                        return Err(DurableError::Conflict(
                            "PostgreSQL Alignment private generation changed",
                        ));
                    }
                    let (((selected_epoch, selected), current), selected_readset) = source_db
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
                                        "PostgreSQL Alignment source epoch changed",
                                    ));
                                }
                                let schemas = selected_contracts(session, deadline, cancelled)
                                    .map_err(DurableError::Source)?;
                                if schemas != schema_members {
                                    return Err(DurableError::Conflict(
                                        "PostgreSQL Alignment contracts changed",
                                    ));
                                }
                                let selected = resolve_selected_alignment(
                                    &grant,
                                    &context,
                                    session,
                                    private_session,
                                    &first_epoch,
                                    &schema_members,
                                    private_generation,
                                    &mut worker,
                                    false,
                                    deadline,
                                    cancelled,
                                )
                                .map_err(DurableError::Source)?;
                                let current = read_alignment_target_after_selection(
                                    private_session,
                                    &target_ref,
                                    &target_header,
                                    deadline,
                                    cancelled,
                                )
                                .map_err(DurableError::Source)?;
                                Ok(((epoch(session), selected), current))
                            },
                        )
                        .map_err(pg::source_error)
                        .map_err(DurableError::Source)?;
                    if !same_epoch(&first_epoch, &selected_epoch) {
                        return Err(DurableError::Conflict(
                            "PostgreSQL Alignment selected source epoch changed",
                        ));
                    }
                    let history = load_history_closure(
                        private_session,
                        &context,
                        &grant,
                        private_generation,
                        &mut dependency_members,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    Ok((
                        (selected_epoch, selected),
                        current,
                        history,
                        selected_readset,
                    ))
                },
            )
            .map_err(pg::source_error)?;
        context.extend_pg_selected_members(dependency_members.clone())?;
        let mut selected_source_members = selected.selected_members;
        selected_source_members.retain(|path, _| !schema_members.contains_key(path));
        context.extend_pg_selected_members(selected_source_members.clone())?;

        let ((inspection, selected_sources), inspection_readset) = db
            .with_private_owner_read_session(
                &scope,
                deadline,
                cancelled,
                |private_session, _, _| {
                    if private_session.private_audit_generation() != private_generation {
                        return Err(DurableError::Conflict(
                            "PostgreSQL Alignment private generation changed",
                        ));
                    }
                    source_db.with_source_revision_session(
                        &store,
                        &selection.domain,
                        &selection.job_id,
                        selection.job_fence,
                        deadline,
                        cancelled,
                        |session| {
                            if !same_epoch(&first_epoch, &epoch(session)) {
                                return Err(DurableError::Conflict(
                                    "PostgreSQL Alignment source epoch changed",
                                ));
                            }
                            let schemas = selected_contracts(session, deadline, cancelled)
                                .map_err(DurableError::Source)?;
                            if schemas != schema_members {
                                return Err(DurableError::Conflict(
                                    "PostgreSQL Alignment contracts changed",
                                ));
                            }
                            let selected = resolve_selected_alignment(
                                &grant,
                                &context,
                                session,
                                private_session,
                                &first_epoch,
                                &schema_members,
                                private_generation,
                                &mut worker,
                                false,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let mut selected_sources = selected.selected_members;
                            selected_sources.retain(|path, _| !schema_members.contains_key(path));
                            if selected_sources != selected_source_members {
                                return Err(DurableError::Conflict(
                                    "PostgreSQL Alignment inspection source closure changed",
                                ));
                            }
                            let inspected = alignment::inspect_pg_owner_alignment_selected(
                                context,
                                grant,
                                current.as_ref().map(|snapshot| &snapshot.members),
                                exact,
                                &mut worker,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            Ok((inspected, selected_sources))
                        },
                    )
                },
            )
            .map_err(pg::source_error)?;
        if selected_readset.selection().identity() != inspection_readset.selection().identity() {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment inspection source readset changed",
            ));
        }
        let result = inspection_result(
            &owner_configuration,
            delegated_operation,
            &operation,
            target_header.current.is_some(),
            inspection,
        )?;
        invocation_changed(
            invocation_path,
            invocation_raw,
            uid,
            &runtime,
            executable_digest,
            deadline,
            cancelled,
        )?;
        if selected_sources != selected_source_members {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment inspection source closure changed",
            ));
        }
        return Ok(result);
    }

    if is_prepare {
        cmd::exact_keys(
            &request,
            &["schema_version", "operation", "mapping", "qualifications"],
        )?;
    } else {
        cmd::exact_keys(
            &request,
            &[
                "schema_version",
                "operation",
                "command_id",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "mapping",
                "qualifications",
            ],
        )?;
    }

    // Resolve source and private owner members under nested private -> public
    // sessions. Each private representation read is exact-member only and the
    // owner grant is rechecked by the Sign resolver before every read.
    let ((selected_epoch, selected), current, history) = db
        .with_private_owner_read_session(&scope, deadline, cancelled, |private_session, _, _| {
            if private_session.private_audit_generation() != private_generation {
                return Err(DurableError::Conflict(
                    "PostgreSQL Alignment private generation changed",
                ));
            }
            let (((selected_epoch, selected), current), _) = source_db
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
                                "PostgreSQL Alignment source epoch changed",
                            ));
                        }
                        let schemas = selected_contracts(session, deadline, cancelled)
                            .map_err(DurableError::Source)?;
                        if schemas != schema_members {
                            return Err(DurableError::Conflict(
                                "PostgreSQL Alignment contracts changed",
                            ));
                        }
                        let selected = resolve_selected_alignment(
                            &grant,
                            &context,
                            session,
                            private_session,
                            &first_epoch,
                            &schema_members,
                            private_generation,
                            &mut worker,
                            true,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        let current = if is_prepare {
                            None
                        } else {
                            read_alignment_target_after_selection(
                                private_session,
                                &target_ref,
                                &target_header,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?
                        };
                        Ok(((epoch(session), selected), current))
                    },
                )
                .map_err(pg::source_error)
                .map_err(DurableError::Source)?;
            if !same_epoch(&first_epoch, &selected_epoch) {
                return Err(DurableError::Conflict(
                    "PostgreSQL Alignment selected source epoch changed",
                ));
            }
            let history = load_history_closure(
                private_session,
                &context,
                &grant,
                private_generation,
                &mut dependency_members,
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;
            Ok(((selected_epoch, selected), current, history))
        })
        .map_err(pg::source_error)?;
    context.extend_pg_selected_members(dependency_members.clone())?;
    let mut selected_source_members = selected.selected_members;
    selected_source_members.retain(|path, _| !schema_members.contains_key(path));
    context.extend_pg_selected_members(selected_source_members.clone())?;

    let tokens = alignment_identity_tokens(&grant)?;
    let identity_private_generation = if !is_prepare {
        db.with_private_owner_read_session(&scope, deadline, cancelled, |session, _, _| {
            if session.private_audit_generation() != private_generation {
                return Err(DurableError::Conflict(
                    "PostgreSQL Alignment private generation changed",
                ));
            }
            match target_header.current.as_ref() {
                Some(_) => replay_identity_generation(
                    session,
                    &target_ref,
                    &target_header,
                    private_generation,
                    &request,
                    &request_raw,
                    deadline,
                    cancelled,
                )
                .map(|value| value.unwrap_or(private_generation))
                .map_err(DurableError::Source),
                None => Ok(private_generation),
            }
        })
        .map_err(pg::source_error)?
    } else {
        private_generation
    };

    let (
        (final_preparation, final_selected, dependencies, final_configuration, publication),
        readset,
    ) = db
        .with_private_owner_read_session(&scope, deadline, cancelled, |private_session, _, _| {
            if private_session.private_audit_generation() != private_generation {
                return Err(DurableError::Conflict(
                    "PostgreSQL Alignment private generation changed",
                ));
            }
            source_db
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
                                "PostgreSQL Alignment source epoch changed",
                            ));
                        }
                        let schemas = selected_contracts(session, deadline, cancelled)
                            .map_err(DurableError::Source)?;
                        if schemas != schema_members {
                            return Err(DurableError::Conflict(
                                "PostgreSQL Alignment contracts changed",
                            ));
                        }
                        let selected = resolve_selected_alignment(
                            &grant,
                            &context,
                            session,
                            private_session,
                            &first_epoch,
                            &schema_members,
                            private_generation,
                            &mut worker,
                            true,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        if selected.selected_members
                            != schema_members
                                .iter()
                                .chain(selected_source_members.iter())
                                .map(|(path, raw)| (path.clone(), raw.clone()))
                                .collect()
                        {
                            return Err(DurableError::Conflict(
                                "PostgreSQL Alignment selected source closure changed",
                            ));
                        }
                        let delegated = private_session.read_text_identity_matches(
                            &tokens,
                            Some(&target_ref),
                            &history.ancestry,
                            deadline,
                            cancelled,
                        )?;
                        if delegated.private_audit_generation != private_generation
                            || !delegated.matches.is_empty()
                        {
                            return Err(DurableError::Source(SourceCommandError::Conflict(
                                "PostgreSQL Alignment delegated identity already owned privately",
                            )));
                        }
                        let public =
                            session.lookup_text_identity_owners(&tokens, deadline, cancelled)?;
                        if public.len() != tokens.len() || public.keys().ne(tokens.iter()) {
                            return Err(DurableError::Corrupt(
                                "PostgreSQL Alignment public identity point shape",
                            ));
                        }
                        if public.values().any(|row| !row.owners.is_empty()) {
                            return Err(DurableError::Source(SourceCommandError::Conflict(
                                "native Alignment delegated identity already owned publicly",
                            )));
                        }
                        let owners_by_token = public
                            .iter()
                            .map(|(token, observation)| (token.clone(), observation.owners.clone()))
                            .collect::<BTreeMap<_, _>>();
                        let generations_by_token = public
                            .iter()
                            .map(|(token, observation)| (token.clone(), observation.generation))
                            .collect::<BTreeMap<_, _>>();
                        let public_digest =
                            crate::source_text_identity::point_identity_snapshot_digest(
                                session.selection(),
                                &tokens,
                                &owners_by_token,
                                &generations_by_token,
                                &BTreeMap::new(),
                            )
                            .map_err(DurableError::Source)?;
                        let identity_digest = private_alignment_identity_digest(
                            public_digest,
                            identity_private_generation,
                            &tokens,
                            &target_ref,
                            &history.ancestry,
                        )
                        .map_err(DurableError::Source)?;
                        context
                            .install_pg_identity_snapshot(identity_digest)
                            .map_err(DurableError::Source)?;
                        context
                            .extend_pg_selected_members(selected_source_members.clone())
                            .map_err(DurableError::Source)?;
                        let expected_configuration = alignment::selected_configuration(
                            &context, &grant, &worker, deadline, cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        let mut dependencies = dependency_members.clone();
                        for (path, raw) in
                            schema_members.iter().chain(selected_source_members.iter())
                        {
                            if dependencies
                                .insert(path.clone(), raw.clone())
                                .is_some_and(|previous| previous.as_slice() != raw.as_slice())
                            {
                                return Err(DurableError::Conflict(
                                    "PostgreSQL Alignment dependency bytes differ",
                                ));
                            }
                        }
                        budget_members(
                            &dependencies,
                            MAX_MEMBERS,
                            MAX_TOTAL_BYTES,
                            MAX_MEMBER_BYTES,
                            "PostgreSQL Alignment dependency budget",
                        )
                        .map_err(DurableError::Source)?;
                        let publication = {
                            let input =
                                super::pg_growth::PgGrowthCurrentInput::new(session, 1, 8192)
                                    .map_err(|_| {
                                        DurableError::Invalid(
                                            "PostgreSQL Alignment publication input",
                                        )
                                    })?;
                            context
                                .install_pg_publication(&input, deadline, cancelled)
                                .map_err(DurableError::Source)?
                        };
                        let final_preparation = if is_prepare {
                            FinalAlignmentPreparation::Preview(
                                alignment::prepare_pg_alignment_preview(
                                    context,
                                    grant,
                                    &request,
                                    target_header.current.is_some(),
                                    &software,
                                    &components,
                                    &mut worker,
                                    deadline,
                                    cancelled,
                                )
                                .map_err(DurableError::Source)?,
                            )
                        } else {
                            FinalAlignmentPreparation::Package(
                                alignment::prepare_pg_alignment_package(
                                    context,
                                    grant,
                                    &request,
                                    current.as_ref().map(|snapshot| &snapshot.members),
                                    &software,
                                    &components,
                                    &mut worker,
                                    deadline,
                                    cancelled,
                                )
                                .map_err(DurableError::Source)?,
                            )
                        };
                        Ok((
                            final_preparation,
                            selected.selected_members,
                            dependencies,
                            expected_configuration,
                            publication,
                        ))
                    },
                )
                .map_err(pg::source_error)
                .map_err(DurableError::Source)
        })
        .map_err(pg::source_error)?;

    if final_selected
        != schema_members
            .iter()
            .chain(selected_source_members.iter())
            .map(|(path, raw)| (path.clone(), raw.clone()))
            .collect()
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment selected member set changed",
        ));
    }
    if is_prepare {
        let FinalAlignmentPreparation::Preview(preview) = final_preparation else {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment preview result absent",
            ));
        };
        if preview.owner_configuration != final_configuration {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL Alignment preview configuration changed",
            ));
        }
        source_db
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
                            "PostgreSQL Alignment preview source epoch changed",
                        ));
                    }
                    let input = super::pg_growth::PgGrowthCurrentInput::new(session, 1, 8192)
                        .map_err(|_| {
                            DurableError::Invalid("PostgreSQL Alignment publication input")
                        })?;
                    publication
                        .verify_from_input(&input, deadline, cancelled)
                        .map_err(DurableError::Source)
                },
            )
            .map_err(pg::source_error)?;
        invocation_changed(
            invocation_path,
            invocation_raw,
            uid,
            &runtime,
            executable_digest,
            deadline,
            cancelled,
        )?;
        return Ok(json!({
            "schema_version":"tos_local_native_alignment_result_v1",
            "authentication":"local-unix-account",
            "owner_configuration":preview.owner_configuration,
            "target_exists":preview.target_exists,
            "expected_source":null,
            "expected_revision":null,
            "supported_operations":[if operation == "prepare-create" {"alignment.create"} else {"alignment.revise"}],
            "command_operations":["describe",operation,if operation == "prepare-create" {"alignment.create"} else {"alignment.revise"},"inspect","inspect-version","inspect-recovery"],
            "receipt_sha256":null,
            "replayed":false,
            "content_disclosure":"withheld",
            "grants_admission":false,
            "assessment_applied":false,
            "aligner_executed":false,
            "expected_dependencies":preview.expected_dependencies
        }));
    }
    let FinalAlignmentPreparation::Package(prepared) = final_preparation else {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment prepared package absent",
        ));
    };
    if prepared.owner_configuration() != final_configuration {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Alignment package configuration changed",
        ));
    }
    let source_binding = alignment_source_binding(&readset, &dependencies, private_generation)
        .map_err(pg::source_error)?;
    let prepared_owner_configuration = prepared.owner_configuration().to_owned();
    let prepared_replayed = prepared.replayed();
    let mut capability = PrivateAlignmentPackageCapabilityV1::from_prepared(
        prepared,
        scope,
        current,
        source_binding,
        &request,
        &request_raw,
        &software,
        &components,
        invocation_path,
        invocation_raw,
        uid,
        &runtime,
        executable_digest,
        deadline,
        cancelled,
    )
    .map_err(pg::source_error)?;
    let committed = db
        .commit_private_owner_package(&mut capability)
        .map_err(pg::source_error)?;
    Ok(json!({
        "schema_version":"tos_local_native_alignment_result_v1",
        "authentication":"local-unix-account",
        "owner_configuration":prepared_owner_configuration,
        "target_exists":true,
        "expected_source":null,
        "expected_revision":null,
        "supported_operations":[operation],
        "command_operations":["describe",if operation == "alignment.create" {"prepare-create"} else {"prepare-revise"},operation,"inspect","inspect-version","inspect-recovery"],
        "command_id":cmd::text(&request, "command_id")?,
        "commit_seq":committed.commit_seq,
        "package_sha256":committed.package_digest.to_prefixed(),
        "receipt_sha256":Digest256::of_bytes(&committed.response_bytes).to_prefixed(),
        "replayed":prepared_replayed || committed.replayed,
        "grants_admission":false,
        "assessment_applied":false,
        "aligner_executed":false,
        "content_disclosure":"withheld"
    }))
}

fn request_schema(request: &JsonValue) -> SourceCommandResult<&str> {
    cmd::text(request, "schema_version")
}

fn inspection_result(
    owner_configuration: &str,
    delegated_operation: &str,
    operation: &str,
    target_exists: bool,
    inspection: alignment::NativeAlignmentInspection,
) -> SourceCommandResult<Value> {
    let summary: Value = serde_json::from_slice(&cmd::canonical(&inspection.mapping_summary)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Alignment summary"))?;
    Ok(json!({
        "schema_version":"tos_local_native_alignment_result_v1",
        "authentication":"local-unix-account",
        "owner_configuration":owner_configuration,
        "target_exists":target_exists,
        "expected_source":null,
        "expected_revision":null,
        "supported_operations":[delegated_operation],
        "command_operations":["describe",operation_list(delegated_operation),delegated_operation,"inspect","inspect-version","inspect-recovery"],
        "receipt_sha256":null,
        "replayed":false,
        "content_disclosure":"withheld",
        "grants_admission":false,
        "assessment_applied":false,
        "aligner_executed":false,
        "inspected_source_sha256":inspection.inspected_source_sha256,
        "record_version":inspection.record_version,
        "claim_version":inspection.claim_version,
        "change_kind":inspection.change_kind,
        "history_depth":inspection.history_depth,
        "metadata_verified":inspection.metadata_verified,
        "content_verified":inspection.content_verified,
        "mapping_summary":summary,
        "competition_forward_count":inspection.competing_forward_count,
        "reverse_competition_posture":"derived_from_explicit_external_record_refs_not_claimed_complete",
        "request_operation":operation
    }))
}
