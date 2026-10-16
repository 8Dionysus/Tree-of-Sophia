//! Typed PG transaction boundary for maintained Edition -> Item adoption.
//!
//! The selected-source owner preparation remains a separate step; this type
//! seals its exact proposal, semantic identities, Item Claim custody and PG
//! readset into the common source-revision writer.
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionReadSession, SourceRevisionReadSet,
};
use crate::source_cohort::{self, IndexRows, PreparedPgSourceRevision, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceChange, SourceCommandError,
    SourceCommandResult,
};
use crate::source_creation_store::{self, CreationFilesystem};
use crate::source_revisions::ReadonlyRecordFiles;
use crate::{DurableError, DurablePgCoordinator, DurableResult};
use postgres::Transaction;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_segment_store::SegmentStore;
use tos_validation::item_rules::ItemLimits;

const ITEM_ADOPTION_FAMILY_TAG: &str = source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG;
const ITEM_ADOPTION_INVOCATION: &str = "tos_local_pg_item_adoption_invocation_v1";
const ITEM_ADOPTION_RESULT: &str = "tos_local_pg_item_adoption_result_v1";
const MAX_ITEM_REQUEST_BYTES: usize = 1_048_576;
const ITEM_ADOPTION_CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-relation-claim.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/source-resource-inventory.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/provenance-event.schema.json",
    "ToS/contracts/source-claim-record.schema.json",
    "ToS/contracts/claim-display-fields.schema.json",
    "ToS/contracts/artifact-source-witness.schema.json",
    "ToS/contracts/artifact-source-witness-v2.schema.json",
    "ToS/contracts/source-link.schema.json",
    "ToS/contracts/object-link-claim-v2.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/source-metadata-record.schema.json",
    "ToS/contracts/semantic-description-record.schema.json",
];

fn item_prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-item-adoption-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn item_commit_result(receipt: &crate::DurableCommitReceipt) -> Value {
    json!({
        "domain": receipt.domain,
        "command_id": receipt.command_id,
        "prepare_id_sha256": Digest256::of_bytes(&receipt.prepare_id).to_prefixed(),
        "commit_seq": receipt.commit_seq,
        "request_sha256": receipt.raw_request_digest.to_prefixed(),
        "delta_sha256": receipt.delta_digest.to_prefixed(),
        "member_root_sha256": receipt.member_root.to_prefixed(),
        "replayed": receipt.replayed,
    })
}

fn read_item_request(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_ITEM_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PG Item request read"))?;
    if raw.len() > MAX_ITEM_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid("PG Item request budget"));
    }
    Ok(raw)
}

fn read_item_current(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let relative = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PG Item contract path"))?;
    let member = session
        .read_current_member(&relative, 4_194_304, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item selected contract absent"))?;
    Ok(member.raw)
}

fn item_contracts(
    session: &mut SourceRevisionReadSession<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut pending = ITEM_ADOPTION_CONTRACTS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        if visited.len() > 128 || !path.starts_with("ToS/") {
            return Err(SourceCommandError::Invalid("PG Item schema closure bound"));
        }
        let raw = read_item_current(session, &path, deadline, cancelled)?;
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            continue;
        }
        crate::source_revisions::collect_schema_refs(&cmd::parse(&raw)?, &path, &mut pending, 0)?;
    }
    Ok(())
}

fn item_source_changes(
    context: &CommandContext,
    outputs: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<Vec<SourceChange>> {
    if outputs.is_empty() || outputs.len() > 512 {
        return Err(SourceCommandError::Invalid("PG Item output package count"));
    }
    outputs
        .iter()
        .map(|(path, after)| {
            let relative = RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("PG Item output path"))?;
            if relative.as_str() != path || !path.starts_with("ToS/") {
                return Err(SourceCommandError::Invalid(
                    "PG Item noncanonical output path",
                ));
            }
            let before = context.file(&relative)?.map(Digest256::of_bytes);
            Ok(SourceChange {
                path: relative,
                before,
                after: Some(after.clone()),
            })
        })
        .collect()
}

fn item_source_writes(
    context: &CommandContext,
    session: &mut SourceRevisionReadSession<'_>,
    changes: &[SourceChange],
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceRevisionWrite>> {
    let mut writes = Vec::with_capacity(changes.len());
    for change in changes {
        let raw = change
            .after
            .as_ref()
            .ok_or(SourceCommandError::Conflict("PG Item output bytes absent"))?;
        let context_before = context.file(&change.path)?.map(Digest256::of_bytes);
        if context_before != change.before {
            return Err(SourceCommandError::Conflict(
                "PG Item expected predecessor differs from selected context",
            ));
        }
        let (expected_predecessor, proposed_revision) =
            match change.before {
                Some(before) => {
                    let member = session
                        .read_current_member(&change.path, max_member_bytes, deadline, cancelled)
                        .map_err(|_| SourceCommandError::Conflict("PG Item predecessor changed"))?;
                    if Digest256::of_bytes(&member.raw) != before {
                        return Err(SourceCommandError::Conflict(
                            "PG Item predecessor bytes differ from held source",
                        ));
                    }
                    let proposed = member.custody_revision.checked_add(1).ok_or(
                        SourceCommandError::Invalid("PG Item custody revision overflow"),
                    )?;
                    (Some((member.custody_revision, before)), proposed)
                }
                None => {
                    if session
                        .has_file(change.path.as_str(), deadline, cancelled)
                        .map_err(|_| {
                            SourceCommandError::Conflict("PG Item insertion absence changed")
                        })?
                    {
                        return Err(SourceCommandError::Conflict(
                            "PG Item insertion target became occupied",
                        ));
                    }
                    (None, 1)
                }
            };
        writes.push(SourceRevisionWrite {
            subject: change.path.as_str().to_owned(),
            expected_predecessor,
            proposed_revision,
            exact_bytes: raw.clone(),
        });
    }
    Ok(writes)
}

fn item_dependencies_from_attempt(
    attempt: &crate::source_cohort::SourceRevisionAttempt,
    request: &JsonValue,
) -> DurableResult<JsonValue> {
    // The compound receipt retains the digest of these bindings. The exact
    // binding object belongs to the sealed publication authorization.
    let package = attempt
        .publication_package
        .as_ref()
        .ok_or(DurableError::Corrupt(
            "PG Item adoption attempt publication package absent",
        ))?;
    let plan = cmd::field(&package.manifest, "plan").map_err(DurableError::Source)?;
    let authorization = cmd::field(plan, "authorization").map_err(DurableError::Source)?;
    let dependencies =
        cmd::field(authorization, "dependency_bindings").map_err(DurableError::Source)?;
    cmd::exact_keys(
        dependencies,
        &[
            "catalog_and_sources",
            "contracts",
            "implementation",
            "retained_transactions",
        ],
    )
    .map_err(DurableError::Source)?;
    let dependency_digest = cmd::record_digest(dependencies)
        .map_err(DurableError::Source)?
        .to_prefixed();
    if cmd::text(authorization, "schema_version").map_err(DurableError::Source)?
        != "tos_item_adoption_authorization_v1"
        || cmd::text(authorization, "command_id").map_err(DurableError::Source)?
            != attempt.command_id
        || cmd::text(authorization, "owner_configuration").map_err(DurableError::Source)?
            != attempt.configuration_canonical_digest.to_prefixed()
        || cmd::text(authorization, "request_digest").map_err(DurableError::Source)?
            != cmd::record_digest(request)
                .map_err(DurableError::Source)?
                .to_prefixed()
        || cmd::text(request, "expected_dependencies").map_err(DurableError::Source)?
            != dependency_digest
    {
        return Err(DurableError::Corrupt(
            "PG Item recovery authorization differs",
        ));
    }
    let mut found = None;
    for write in &attempt.writes {
        if write.subject.ends_with("/edition-item-receipt.json") {
            let receipt = cmd::parse(&write.exact_bytes).map_err(DurableError::Source)?;
            if cmd::text(&receipt, "dependencies").map_err(DurableError::Source)?
                != dependency_digest
                || cmd::text(&receipt, "command_id").map_err(DurableError::Source)?
                    != attempt.command_id
                || cmd::text(&receipt, "transaction_id").map_err(DurableError::Source)?
                    != package.transaction_id
            {
                return Err(DurableError::Corrupt(
                    "PG Item recovery dependency binding differs",
                ));
            }
            if found.replace(dependencies.clone()).is_some() {
                return Err(DurableError::Corrupt(
                    "PG Item adoption attempt has duplicate compound receipts",
                ));
            }
        }
    }
    found.ok_or(DurableError::Corrupt(
        "PG Item adoption attempt compound receipt absent",
    ))
}

fn verify_item_deposit_receipt(bytes: &[u8], stage: &Value) -> SourceCommandResult<()> {
    let observed = cmd::parse(bytes)?;
    let expected = crate::source_item_deposit::foundation(
        &crate::source_item_deposit::public_receipt(stage)?,
    )?;
    if !cmd::same(&observed, &expected)? {
        return Err(SourceCommandError::Conflict(
            "PG Item attempt deposit receipt differs from retained custody",
        ));
    }
    Ok(())
}

fn recovered_deposit_receipt_bytes(
    attempt: &crate::source_cohort::SourceRevisionAttempt,
) -> DurableResult<Vec<u8>> {
    let mut found = None;
    for write in &attempt.writes {
        if write.subject.ends_with("/item-deposit-receipt.json")
            && found.replace(write.exact_bytes.clone()).is_some()
        {
            return Err(DurableError::Corrupt(
                "PG Item adoption attempt has duplicate deposit receipts",
            ));
        }
    }
    found.ok_or(DurableError::Corrupt(
        "PG Item adoption attempt deposit receipt absent",
    ))
}

fn pg_item_result(result: Value, receipt: &crate::DurableCommitReceipt) -> Value {
    json!({
        "schema_version": ITEM_ADOPTION_RESULT,
        "result": result,
        "replayed": receipt.replayed,
        "durable_commit": item_commit_result(receipt),
        "grants_admission": false
    })
}

fn pg_item_no_commit_result(result: Value) -> Value {
    json!({
        "schema_version": ITEM_ADOPTION_RESULT,
        "result": result,
        "replayed": false,
        "durable_commit": null,
        "grants_admission": false
    })
}

fn item_source_read_work(work: &crate::source_cohort::revision::SourceRevisionReadWork) -> Value {
    json!({
        "scope":"bounded PG/STO source selection; software capture and protected payload I/O excluded",
        "exact_source_members_read":work.exact_source_members_read,
        "exact_source_bytes_read":work.exact_source_bytes_read,
        "historical_source_members_read":work.historical_source_members_read,
        "historical_source_bytes_read":work.historical_source_bytes_read,
        "custody_metadata_bytes_read":work.custody_metadata_bytes_read,
        "segment_bytes_read":work.segment_bytes_read,
        "committed_member_rows_examined":work.committed_member_rows_examined,
        "metadata_member_lookups":work.metadata_member_lookups,
        "directory_index_rows_examined":work.directory_index_rows_examined,
        "unique_index_lookups":work.unique_index_lookups,
        "text_identity_lookups":work.text_identity_lookups,
        "claim_dependency_lookups":work.claim_dependency_lookups,
        "claim_navigation_lookups":work.claim_navigation_lookups,
        "item_file_lookups":work.item_file_lookups,
        "item_rights_lookups":work.item_rights_lookups,
        "item_edition_lookups":work.item_edition_lookups,
        "sign_candidate_lookups":work.sign_candidate_lookups,
    })
}

enum PgItemSessionOutcome {
    Preview(JsonValue),
    NoMetadata {
        context: CommandContext,
        response: JsonValue,
        dependencies: JsonValue,
        transaction_id: String,
        stage: Value,
        payload_identity: Value,
    },
    Prepared(PreparedPgItemAdoptionRevision),
}

/// Run one maintained Item adoption with an exact PG source epoch. The local
/// filesystem remains an independently checked owner for protected custody
/// only; every authored source read, directory fence and output is in PG.
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
    if super::text(invocation, "schema_version")? != ITEM_ADOPTION_INVOCATION {
        return Err(SourceCommandError::Unsupported(
            "PG Item invocation profile",
        ));
    }
    let selection = super::pg_claims::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) =
        super::pg_claims::invocation_budgets(invocation)?;
    let expected_executable = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    let _ = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Item executable identity",
        ));
    }

    let owner_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let owner_config = cmd::parse(&owner_raw)?;
    if cmd::text(&owner_config, "schema_version")? != "tos_local_item_adoption_owner_v1" {
        return Err(SourceCommandError::Denied("PG Item protected owner family"));
    }
    let request_raw = read_item_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let preparing = operation == "prepare-create";
    if cmd::text(&request, "schema_version")? != "tos_local_item_adoption_command_v1"
        || !matches!(operation, "prepare-create" | "item.adopt")
    {
        return Err(SourceCommandError::Unsupported("PG Item operation"));
    }
    let (command_id, transaction_id) = if preparing {
        (None, None)
    } else {
        let command_id = cmd::text(&request, "command_id")?;
        if command_id.is_empty() || command_id.len() > 256 || command_id.contains('\0') {
            return Err(SourceCommandError::Invalid("PG Item command identity"));
        }
        (
            Some(command_id.to_owned()),
            Some(source_creation_store::pg_item_adoption_transaction_id_from_request(&request)?),
        )
    };
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Item STO"))?;
    let (software, components) =
        super::pg_claims::software_selection(invocation, deadline, cancelled)?;
    let (software_files, _, _) = super::pg_claims::software_files(
        &software,
        &components,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let owner_configuration_digest = cmd::record_digest(&owner_config)?;

    // Resolve the already sealed command before reading current source for a
    // fresh attempt. The attempt itself retains its original PG source epoch.
    if let Some(command_id) = command_id.as_deref() {
        let transaction_id = transaction_id
            .as_deref()
            .ok_or(SourceCommandError::Invalid("PG Item transaction identity"))?;
        let prepare_id = item_prepare_id(&selection.domain, command_id);
        let request_digest = Digest256::of_bytes(&request_raw);
        match db.recover_source_revision_attempt(
            &store,
            &selection.domain,
            &prepare_id,
            command_id,
            request_digest,
            Digest256::of_bytes(&owner_raw),
            owner_configuration_digest,
            ITEM_ADOPTION_FAMILY_TAG,
            false,
            &selection.job_id,
            selection.job_fence,
            None,
            deadline,
            cancelled,
        ) {
            Ok(attempt) => {
                if attempt.contract != selection.contract {
                    return Err(SourceCommandError::Conflict(
                        "recovered PG Item contract changed",
                    ));
                }
                let dependencies = item_dependencies_from_attempt(&attempt, &request)
                    .map_err(super::pg_claims::source_error)?;
                let deposit_config = crate::source_item_deposit::value(&owner_config)?;
                let stage = crate::source_item_deposit::read_stage(
                    &deposit_config,
                    &transaction_id,
                    uid,
                    deadline,
                    cancelled,
                )?
                .ok_or(SourceCommandError::Conflict(
                    "recovered PG Item deposit stage absent",
                ))?;
                verify_item_deposit_receipt(
                    &recovered_deposit_receipt_bytes(&attempt)
                        .map_err(super::pg_claims::source_error)?,
                    &stage,
                )?;
                let payload_identity = crate::source_item_deposit::deposited_identity(
                    &deposit_config,
                    &stage,
                    uid,
                    deadline,
                    cancelled,
                )?;
                let context = CommandContext {
                    base_revision: attempt.predecessor_source_revision,
                    configuration_raw: owner_raw.clone(),
                    request_raw: request_raw.clone(),
                    recorded_at: crate::source_serialization::instant()?,
                    effective_uid: u64::from(uid),
                    files: software_files.clone(),
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
                        return Err(DurableError::Conflict(
                            "PG Item invocation changed before recovery",
                        ));
                    }
                    let (_, current_raw) =
                        CreationFilesystem::select_protected_native_owner_at_root(
                            &owner_path,
                            &selection.store_root,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                    if current_raw != owner_raw
                        || runtime
                            .current_digest(deadline, cancelled)
                            .map_err(DurableError::Source)?
                            != expected_executable
                    {
                        return Err(DurableError::Conflict(
                            "PG Item owner or executable changed before recovery",
                        ));
                    }
                    crate::source_creation_store::verify_pg_item_adoption_owner(
                        &owner_fs,
                        &context,
                        &software,
                        &components,
                        &transaction_id,
                        &dependencies,
                        &stage,
                        &payload_identity,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)
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
                    .map_err(super::pg_claims::source_error)?;
                let recovered = source_creation_store::pg_item_adoption_response_from_attempt(
                    &owner_fs, &context, &attempt, &stage, deadline, cancelled,
                )?;
                if recovered.get("transaction_id").and_then(Value::as_str) != Some(transaction_id) {
                    return Err(SourceCommandError::Conflict(
                        "PG Item recovery transaction response differs",
                    ));
                }
                return Ok(pg_item_result(recovered, &receipt));
            }
            Err(DurableError::Refused("source revision attempt absent")) => {}
            Err(error) => return Err(super::pg_claims::source_error(error)),
        }
    }

    let recorded_at = crate::source_serialization::instant()?;
    let limits = ItemLimits {
        max_member_bytes: usize::try_from(max_member_bytes)
            .map_err(|_| SourceCommandError::Invalid("PG Item member budget"))?,
        max_total_bytes,
        max_state_bytes: usize::try_from(max_total_bytes)
            .map_err(|_| SourceCommandError::Invalid("PG Item state budget"))?,
        max_issues: 256,
        deadline,
    };
    let (prepared, readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected PG Item source contract differs",
                    ));
                }
                let identity = session.selection().identity();
                let base = CommandContext {
                    base_revision: identity.bootstrap_revision,
                    configuration_raw: owner_raw.clone(),
                    request_raw: request_raw.clone(),
                    recorded_at: recorded_at.clone(),
                    effective_uid: u64::from(uid),
                    files: software_files.clone(),
                };
                let owner_selection = if preparing {
                    source_creation_store::select_pg_item_adoption_proposal(
                        &owner_fs, &base, session, deadline, cancelled,
                    )
                } else {
                    source_creation_store::select_pg_item_adoption_owner(
                        &owner_fs, &base, session, deadline, cancelled,
                    )
                }
                .map_err(DurableError::Source)?;
                item_contracts(session, deadline, cancelled).map_err(DurableError::Source)?;
                let (context, _) =
                    super::context_with_pg_exact_reads(&base, session, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                super::pg_claims::check_context_budget(
                    &context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                let mut worker = super::revisions::selected_candidate_schema_from_context(
                    invocation,
                    &context.files,
                    identity,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let input = super::PgGrowthCurrentInput::new(
                    session,
                    4096,
                    usize::try_from(max_member_bytes)
                        .map_err(|_| DurableError::Invalid("PG Item input member budget"))?,
                )
                .map_err(|_| DurableError::Invalid("PG Item selected input budget"))?;
                let owner_plan = source_creation_store::prepare_pg_item_adoption(
                    &owner_fs,
                    &context,
                    owner_selection,
                    &input,
                    &software,
                    &components,
                    &mut worker,
                    limits,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                drop(input);
                let (mut final_context, _) =
                    super::context_with_pg_exact_reads(&context, session, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                super::pg_claims::check_context_budget(
                    &final_context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                if owner_plan.prepared_request.is_some() {
                    return Ok(PgItemSessionOutcome::Preview(owner_plan.response));
                }
                if owner_plan.outputs.is_empty() {
                    if owner_plan.item_claim_owner != JsonValue::Null
                        || cmd::field(&owner_plan.response, "metadata_committed")
                            .map_err(DurableError::Source)?
                            != &JsonValue::Bool(false)
                    {
                        return Err(DurableError::Corrupt(
                            "PG Item no-metadata result carries source writes",
                        ));
                    }
                    return Ok(PgItemSessionOutcome::NoMetadata {
                        context: final_context,
                        response: owner_plan.response,
                        dependencies: owner_plan.dependency_bindings,
                        transaction_id: owner_plan.transaction_id,
                        stage: owner_plan.stage,
                        payload_identity: owner_plan.payload_identity,
                    });
                }
                let (control_before, control_after) =
                    owner_plan
                        .publication_control
                        .ok_or(DurableError::Conflict(
                            "PG Item publication control output absent",
                        ))?;
                let control_path =
                    RelativePath::parse("ToS/source-witnesses/.metadata-publication.json")
                        .map_err(|_| DurableError::Invalid("PG Item publication control path"))?;
                // The publication control is a PG virtual member, fenced by
                // source-publication/current, not an authored STO exact read.
                let control_raw = if session
                    .has_file(control_path.as_str(), deadline, cancelled)
                    .map_err(DurableError::Source)?
                {
                    Some(
                        session
                            .read(control_path.as_str(), 8192, deadline, cancelled)
                            .map_err(DurableError::Source)?,
                    )
                } else {
                    None
                };
                if control_raw.as_deref().map(Digest256::of_bytes) != control_before {
                    return Err(DurableError::Conflict(
                        "PG Item publication control predecessor differs",
                    ));
                }
                if let Some(raw) = control_raw {
                    if let Some(existing) = final_context
                        .file(&control_path)
                        .map_err(DurableError::Source)?
                    {
                        if existing != raw {
                            return Err(DurableError::Conflict(
                                "PG Item captured publication control differs",
                            ));
                        }
                    } else {
                        final_context.files.push(cmd::SourceFile {
                            path: control_path.clone(),
                            raw,
                        });
                        final_context.files.sort_by(|a, b| a.path.cmp(&b.path));
                    }
                }
                super::pg_claims::check_context_budget(
                    &final_context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                let mut output_members = owner_plan.outputs;
                if output_members
                    .insert(control_path.as_str().to_owned(), control_after)
                    .is_some()
                {
                    return Err(DurableError::Corrupt(
                        "PG Item publication control collides with owner output",
                    ));
                }
                let changes = item_source_changes(&final_context, &output_members)
                    .map_err(DurableError::Source)?;
                // Validate the full publication package, then select the same
                // retained source delta as the common transaction writer.
                let (_, retained_changes) =
                    crate::source_work_transaction::extract_selected_publication_package(&changes)
                        .map_err(DurableError::Source)?;
                let writes = item_source_writes(
                    &final_context,
                    session,
                    &retained_changes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let semantic_indexes =
                    source_creation_store::pg_expected_revision_indexes(&writes)?;
                PreparedPgItemAdoptionRevision::observe_indexes(
                    session,
                    &semantic_indexes,
                    &writes,
                    deadline,
                    cancelled,
                )?;
                let final_binding = super::pg_growth::PgGrowthReadBinding::capture(session);
                let proposal = final_context
                    .plan(
                        "native-item-adoption",
                        owner_plan.response.clone(),
                        changes,
                        false,
                    )
                    .map_err(DurableError::Source)?;
                let prepared = PreparedPgItemAdoptionRevision::new(
                    final_context,
                    proposal,
                    identity,
                    final_binding,
                    semantic_indexes,
                    owner_plan.item_claim_owner,
                    owner_plan.dependency_bindings,
                    owner_plan.transaction_id,
                    owner_plan.stage,
                    owner_plan.payload_identity,
                )?;
                Ok(PgItemSessionOutcome::Prepared(prepared))
            },
        )
        .map_err(super::pg_claims::source_error)?;

    let prepared = match prepared {
        PgItemSessionOutcome::Preview(response) => {
            if super::read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )? != invocation_raw
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item invocation changed before prepare-create reply",
                ));
            }
            let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
                &owner_path,
                &selection.store_root,
                deadline,
                cancelled,
            )?;
            if current_raw != owner_raw
                || runtime.current_digest(deadline, cancelled)? != expected_executable
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item owner or executable changed before prepare-create reply",
                ));
            }
            let mut result: serde_json::Value = serde_json::from_slice(&cmd::canonical(&response)?)
                .map_err(|_| SourceCommandError::Invalid("PG Item preview result JSON"))?;
            result["source_read_work"] = item_source_read_work(&readset.work);
            return Ok(pg_item_no_commit_result(result));
        }
        PgItemSessionOutcome::NoMetadata {
            context,
            response,
            dependencies,
            transaction_id,
            stage,
            payload_identity,
        } => {
            if super::read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )? != invocation_raw
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item invocation changed before no-metadata reply",
                ));
            }
            let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
                &owner_path,
                &selection.store_root,
                deadline,
                cancelled,
            )?;
            if current_raw != owner_raw
                || runtime.current_digest(deadline, cancelled)? != expected_executable
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item owner or executable changed before no-metadata reply",
                ));
            }
            source_creation_store::verify_pg_item_adoption_owner(
                &owner_fs,
                &context,
                &software,
                &components,
                &transaction_id,
                &dependencies,
                &stage,
                &payload_identity,
                deadline,
                cancelled,
            )?;
            let mut result: serde_json::Value = serde_json::from_slice(&cmd::canonical(&response)?)
                .map_err(|_| SourceCommandError::Invalid("PG Item no-metadata result JSON"))?;
            result["source_read_work"] = item_source_read_work(&readset.work);
            return Ok(pg_item_no_commit_result(result));
        }
        PgItemSessionOutcome::Prepared(prepared) => prepared,
    };

    let transaction_id =
        transaction_id.ok_or(SourceCommandError::Invalid("PG Item transaction identity"))?;
    let command_id = command_id.ok_or(SourceCommandError::Invalid("PG Item command identity"))?;
    let prepare_id = item_prepare_id(&selection.domain, &command_id);

    let mut result: serde_json::Value =
        serde_json::from_slice(&cmd::canonical(&prepared.proposal().response)?)
            .map_err(|_| SourceCommandError::Invalid("PG Item prepared result JSON"))?;
    result["source_read_work"] = item_source_read_work(&readset.work);
    let attempt = db
        .register_source_revision(
            &store,
            &prepared,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(super::pg_claims::source_error)?;
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
            return Err(DurableError::Conflict(
                "PG Item invocation changed before commit",
            ));
        }
        let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
            &owner_path,
            &selection.store_root,
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?;
        if current_raw != owner_raw
            || runtime
                .current_digest(deadline, cancelled)
                .map_err(DurableError::Source)?
                != expected_executable
        {
            return Err(DurableError::Conflict(
                "PG Item owner or executable changed before commit",
            ));
        }
        let deposit_receipt = prepared
            .proposal()
            .changes
            .iter()
            .find(|change| change.path.as_str().ends_with("/item-deposit-receipt.json"))
            .and_then(|change| change.after.as_deref())
            .ok_or(DurableError::Corrupt(
                "PG Item prepared deposit receipt absent",
            ))?;
        verify_item_deposit_receipt(deposit_receipt, &prepared.stage)
            .map_err(DurableError::Source)?;
        source_creation_store::verify_pg_item_adoption_owner(
            &owner_fs,
            prepared.context(),
            &software,
            &components,
            &prepared.transaction_id,
            &prepared.dependencies,
            &prepared.stage,
            &prepared.payload_identity,
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)
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
        .map_err(super::pg_claims::source_error)?;
    {
        let result_object = result
            .as_object_mut()
            .ok_or(SourceCommandError::Invalid("PG Item owner response object"))?;
        result_object.insert("transaction_id".to_owned(), json!(transaction_id));
        result_object.insert("metadata_committed".to_owned(), json!(true));
        let deposit = result_object
            .get_mut("deposit")
            .and_then(Value::as_object_mut)
            .ok_or(SourceCommandError::Invalid(
                "PG Item deposit response object",
            ))?;
        deposit.insert("metadata_committed".to_owned(), json!(true));
    }
    Ok(pg_item_result(result, &receipt))
}

/// Owner-prepared Item adoption revision. The contained Claim block is
/// derived from the exact 18 Edition/Item successor members and is carried
/// through registration, recovery and commit with the package bytes.
pub(crate) struct PreparedPgItemAdoptionRevision {
    context: CommandContext,
    proposal: PreparedCommand,
    identity: PgRecordRevisionIdentity,
    binding: super::pg_growth::PgGrowthReadBinding,
    semantic_indexes: IndexRows,
    item_claim_owner: JsonValue,
    dependencies: JsonValue,
    transaction_id: String,
    stage: Value,
    payload_identity: Value,
}

impl PreparedPgItemAdoptionRevision {
    pub(crate) fn new(
        context: CommandContext,
        proposal: PreparedCommand,
        identity: PgRecordRevisionIdentity,
        binding: super::pg_growth::PgGrowthReadBinding,
        semantic_indexes: IndexRows,
        item_claim_owner: JsonValue,
        dependencies: JsonValue,
        transaction_id: String,
        stage: Value,
        payload_identity: Value,
    ) -> DurableResult<Self> {
        if context.base_revision != identity.bootstrap_revision
            || proposal.base_revision != identity.bootstrap_revision
            || proposal.handler_id != "native-item-adoption"
            || proposal.operation != "item.adopt"
            || proposal.changes.is_empty()
            || proposal.changes.iter().any(|change| change.after.is_none())
            || semantic_indexes.iter().any(|(kind, token, _)| {
                !["metadata", "claim", "event", "anchor", "form"].contains(&kind.as_str())
                    || token.is_empty()
            })
            || semantic_indexes
                .iter()
                .map(|(_, token, _)| token)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != semantic_indexes.len()
        {
            return Err(DurableError::Conflict(
                "PG Item adoption prepared family or package differs",
            ));
        }
        let owner_fields = item_claim_owner
            .as_object()
            .ok_or(DurableError::Corrupt("PG Item adoption owner block shape"))?;
        let closureless = JsonValue::Object(
            owner_fields
                .iter()
                .filter(|(key, _)| key.as_str() != Some("closure_sha256"))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        let closure =
            crate::source_command::record_digest(&closureless).map_err(DurableError::Source)?;
        if item_claim_owner
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            != Some("tos_managed_item_adoption_claim_owner_inventory_v1")
            || item_claim_owner
                .object_get("closure_sha256")
                .and_then(JsonValue::as_str)
                != Some(closure.to_prefixed().as_str())
        {
            return Err(DurableError::Corrupt(
                "PG Item adoption owner contribution closure differs",
            ));
        }
        Ok(Self {
            context,
            proposal,
            identity,
            binding,
            semantic_indexes,
            item_claim_owner,
            dependencies,
            transaction_id,
            stage,
            payload_identity,
        })
    }

    pub(crate) fn observe_indexes(
        session: &mut SourceRevisionReadSession<'_>,
        semantic_indexes: &IndexRows,
        writes: &[SourceRevisionWrite],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        if semantic_indexes.len() > 512
            || semantic_indexes.iter().any(|(kind, token, path)| {
                !["metadata", "claim", "event", "anchor", "form"].contains(&kind.as_str())
                    || token.is_empty()
                    || path.is_empty()
            })
        {
            return Err(DurableError::Invalid(
                "PG Item adoption semantic index observation bound",
            ));
        }
        let tokens = semantic_indexes
            .iter()
            .map(|(_, token, _)| token.clone())
            .collect();
        // Compound adoption updates existing Edition identities and creates
        // Item identities. Fence both cases across all five namespaces.
        let observed = session.lookup_source_identities(&tokens, deadline, cancelled)?;
        for (token, owner) in observed.owners() {
            if let Some((kind, path)) = owner {
                if !semantic_indexes
                    .iter()
                    .any(|row| row == &(kind.clone(), token.clone(), path.as_str().to_owned()))
                    || !writes.iter().any(|write| {
                        write.subject == path.as_str() && write.expected_predecessor.is_some()
                    })
                {
                    return Err(DurableError::Conflict(
                        "PG Item identity is occupied outside selected predecessor",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl PreparedPgSourceRevision for PreparedPgItemAdoptionRevision {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }

    fn family_tag(&self) -> &str {
        ITEM_ADOPTION_FAMILY_TAG
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        super::pg_growth::validate_growth_readset(&self.binding, readset)
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
        let expected = source_creation_store::pg_expected_revision_indexes(writes)?;
        if expected != self.semantic_indexes {
            return Err(DurableError::Corrupt(
                "PG Item adoption semantic indexes differ from sealed output",
            ));
        }
        let mut indexes = source_cohort::check_revision_path_indexes_for_family(
            tx,
            domain,
            writes,
            ITEM_ADOPTION_FAMILY_TAG,
        )?;
        for (kind, token, path) in expected {
            let existing = tx.query_opt(
                "SELECT path FROM cmd2_source_index WHERE domain=$1 AND kind=$2 AND token=$3 LIMIT 1",
                &[&domain, &kind, &token],
            )?;
            if existing
                .as_ref()
                .is_some_and(|row| row.get::<_, String>(0) != path)
            {
                return Err(DurableError::Conflict(
                    "PG Item adoption semantic identity is occupied",
                ));
            }
            indexes.insert((kind, token, path));
        }
        Ok(indexes)
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, JsonValue>> {
        source_creation_store::pg_inventory_owner_contributions(
            writes,
            &self.context.files,
            &self.item_claim_owner,
        )
    }
}
