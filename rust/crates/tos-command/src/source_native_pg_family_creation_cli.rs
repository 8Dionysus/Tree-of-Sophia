//! Native Historical and generic public Profile creation over a bounded PG
//! source read epoch. The semantic owner prepares and seals the package; this
//! adapter only selects exact current inputs and invokes the shared writer.

use super::*;
use crate::durable_private_packages::{PrivateOwnerReadSession, PrivateOwnerScopeV1};
use crate::source_cohort::creation::{COMMITMENT_VERSION, PgCreationAttempt};
use crate::source_cohort::revision::{SourceRevisionIdentityClosure, SourceRevisionReadSession};
use crate::source_creation::pg_family::PgFamilyCreationObservations;
use crate::source_native_text_read::{LocalTextReadSelection, PG_PRIVATE_SCHEMA};
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_sign_native::{NativeReadKind, SignNativeRead};
use crate::source_text_owner::{OwnerTextContext, read_absolute};
use crate::{DurableError, DurablePgCoordinator};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_segment_store::SegmentStore;

const INVOCATION_SCHEMA: &str = "tos_local_pg_historical_profile_creation_invocation_v1";
const OWNER_CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";

#[derive(Clone)]
struct NativeTextPackagePin {
    revision: u64,
    package_digest: Digest256,
    command_id: String,
}

struct CreationPgPrivateTextSelection {
    profile: LocalTextReadSelection,
    context: OwnerTextContext,
    pins: BTreeMap<String, NativeTextPackagePin>,
    scope: PrivateOwnerScopeV1,
}

/// The source.create writer can read selected public PG members and exact
/// private Text package history only through this independent, preselected
/// grant. Owner-private bytes never fall back to a local filesystem path.
struct CreationPgPrivateTextReader<'a, 'private_db, 'source_db> {
    profile: &'a LocalTextReadSelection,
    context: &'a OwnerTextContext,
    private_session: &'a mut PrivateOwnerReadSession<'private_db>,
    source_session: &'a mut SourceRevisionReadSession<'source_db>,
    pins: &'a BTreeMap<String, NativeTextPackagePin>,
    identity: crate::source_cohort::revision::PgRecordRevisionIdentity,
    selected_members: BTreeMap<String, Vec<u8>>,
}

impl SignNativeRead for CreationPgPrivateTextReader<'_, '_, '_> {
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
                    "PG source.create selected native member byte budget",
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
                    "PG source.create private Text package is not explicitly selected",
                ))?;
            let member_name = reference
                .rsplit('/')
                .next()
                .ok_or(SourceCommandError::Invalid(
                    "PG source.create private Text member path",
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
                .map_err(super::source_error)?
                .ok_or(SourceCommandError::Conflict(
                    "PG source.create private Text package member absent",
                ))?;
            if selected.private_audit_generation != self.private_session.private_audit_generation()
            {
                return Err(SourceCommandError::Conflict(
                    "PG source.create private Text package generation changed",
                ));
            }
            selected.bytes
        } else {
            let path = RelativePath::parse(reference)
                .map_err(|_| SourceCommandError::Denied("PG source.create source path"))?;
            self.source_session
                .read_current_member(&path, max_bytes as u64, deadline, cancelled)
                .map_err(super::source_error)?
                .raw
        };
        if raw.len() > max_bytes {
            return Err(SourceCommandError::Unsupported(
                "PG source.create selected native member byte budget",
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
        self.profile.verify(deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        if self.source_session.selection().identity() != self.identity {
            return Err(SourceCommandError::Conflict(
                "PG source.create private read source epoch changed",
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

fn select_creation_private_text(
    invocation: &Value,
    request: &JsonValue,
    context: &cmd::CommandContext,
    selection: &super::SourceSelection,
    worker: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        crate::source_cohort::revision::PgRecordRevisionIdentity,
    >,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<CreationPgPrivateTextSelection>> {
    let profile_path = invocation
        .get("native_text_read_profile")
        .and_then(Value::as_str);
    let record = cmd::field(request, "record")?;
    let Some(binding) = record.object_get("native_text_binding") else {
        if profile_path.is_some() {
            return Err(SourceCommandError::Denied(
                "PG private Text read profile has no selected native binding",
            ));
        }
        return Ok(None);
    };
    let packet_ref = cmd::text(binding, "packet_ref")?;
    let layer_ref = cmd::text(cmd::field(binding, "text_layer")?, "record_ref")?;
    let private_metadata = [packet_ref, layer_ref]
        .iter()
        .any(|reference| reference.starts_with("ToS/source-witnesses/owner-local/"));
    let Some(profile_path) = profile_path else {
        if private_metadata {
            return Err(SourceCommandError::Denied(
                "private source.create TextUnit requires an independent PG read selection",
            ));
        }
        return Ok(None);
    };
    let profile_path = super::absolute(profile_path)?;
    let profile_raw = read_absolute(&profile_path, uid, true, 262_144, deadline, cancelled)?;
    let profile_hint = cmd::parse(&profile_raw)?;
    let profile_owner_context = cmd::field(&profile_hint, "owner_context")?;
    let owner_context_path =
        crate::source_text_owner::normalized_absolute(cmd::text(profile_owner_context, "path")?)?;
    let owner_schema_path = RelativePath::parse(OWNER_CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("PG owner context schema path"))?;
    let owner_schema_raw = context
        .file(&owner_schema_path)?
        .ok_or(SourceCommandError::Conflict(
            "selected PG owner context schema absent",
        ))?
        .to_vec();
    let (owner_context, _) = OwnerTextContext::select_pg(
        &owner_context_path,
        &owner_schema_raw,
        worker,
        deadline,
        cancelled,
    )?;
    let selected_members = context
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/"))
        .map(|file| (file.path.as_str().to_owned(), file.raw.clone()))
        .collect::<BTreeMap<_, _>>();
    owner_context.install_pg_selected_members(selected_members)?;
    let profile_schema_path = RelativePath::parse(PG_PRIVATE_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("PG private read profile schema path"))?;
    let profile_schema_raw = context
        .file(&profile_schema_path)?
        .ok_or(SourceCommandError::Conflict(
            "selected PG private read profile schema absent",
        ))?
        .to_vec();
    let profile = LocalTextReadSelection::load_pg_private_profile(
        &profile_path,
        owner_context.public_root(),
        &profile_schema_raw,
        worker,
        deadline,
        cancelled,
    )?;
    let configured_context = cmd::field(profile.config(), "owner_context")?;
    if cmd::text(profile.config(), "source_domain")? != selection.domain.as_str()
        || cmd::text(profile.config(), "source_contract_sha256")?
            != selection.contract.to_hex().as_str()
        || crate::source_text_owner::normalized_absolute(cmd::text(configured_context, "path")?)?
            != owner_context.configuration_path()
        || cmd::text(configured_context, "sha256")?
            != owner_context
                .configuration_sha256(deadline, cancelled)?
                .as_str()
    {
        return Err(SourceCommandError::Denied(
            "PG source.create private read selection source or context differs",
        ));
    }
    let mut pins = BTreeMap::new();
    for package in profile.selected_pg_packages(binding, None, deadline, cancelled)? {
        let target_ref = cmd::text(&package, "target_ref")?.to_owned();
        if !target_ref.starts_with(owner_context.private_prefix())
            || RelativePath::parse(&target_ref).is_err()
            || pins.contains_key(&target_ref)
        {
            return Err(SourceCommandError::Denied(
                "PG source.create private package pin is duplicated or outside owner scope",
            ));
        }
        let revision = cmd::integer(&package, "revision")?;
        if revision == 0 {
            return Err(SourceCommandError::Invalid(
                "PG source.create private package revision",
            ));
        }
        let package_digest = Digest256::from_hex(cmd::text(&package, "package_sha256")?)
            .map_err(|_| SourceCommandError::Invalid("PG source.create package pin digest"))?;
        let command_id = cmd::text(&package, "command_id")?.to_owned();
        if command_id.is_empty() || command_id.len() > 256 || command_id.contains('\0') {
            return Err(SourceCommandError::Invalid(
                "PG source.create private package command ID",
            ));
        }
        pins.insert(
            target_ref,
            NativeTextPackagePin {
                revision,
                package_digest,
                command_id,
            },
        );
    }
    if pins.is_empty() || pins.len() > 32 {
        return Err(SourceCommandError::Denied(
            "PG source.create private package pins are empty or over budget",
        ));
    }
    let required_targets = [packet_ref, layer_ref]
        .iter()
        .filter(|reference| owner_context.is_owner_private_reference(reference))
        .map(|reference| owner_context.private_package_target_ref(reference))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if !required_targets.is_subset(&pins.keys().cloned().collect()) {
        return Err(SourceCommandError::Denied(
            "PG source.create private metadata package is not pinned",
        ));
    }
    let context_binding = owner_context.private_owner_scope_binding(deadline, cancelled)?;
    let scope = PrivateOwnerScopeV1::for_context(&selection.domain, context_binding)
        .map_err(super::source_error)?;
    Ok(Some(CreationPgPrivateTextSelection {
        profile,
        context: owner_context,
        pins,
        scope,
    }))
}

/// Recheck the protected private Text grant for a recovered creation attempt.
/// Recovery only selects the two schemas needed to validate the grant; it does
/// not run the source.create identity or target-absence admission path again.
fn select_recovered_creation_private_text(
    db: &mut DurablePgCoordinator,
    store: &SegmentStore,
    selection: &super::SourceSelection,
    invocation: &Value,
    request: &JsonValue,
    replay_context: &cmd::CommandContext,
    family: CreationFamily,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<CreationPgPrivateTextSelection>> {
    if invocation.get("native_text_read_profile").is_none() {
        return Ok(None);
    }
    if family != CreationFamily::PublicProfile {
        return Err(SourceCommandError::Denied(
            "PG private Text read grant applies only to PublicProfile creation",
        ));
    }

    let ((context, identity), readset) = db
        .with_source_revision_session(
            store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract
                    || replay_context.base_revision != session.selection().cohort.initial_revision()
                {
                    return Err(DurableError::Conflict(
                        "recovered private Text source contract or bootstrap changed",
                    ));
                }
                let identity = session.selection().identity();
                let mut context = replay_context.clone();
                for schema_ref in [OWNER_CONTEXT_SCHEMA, PG_PRIVATE_SCHEMA] {
                    let path = RelativePath::parse(schema_ref).map_err(|_| {
                        DurableError::Source(SourceCommandError::Invalid(
                            "recovered private Text schema path",
                        ))
                    })?;
                    if context.file(&path).map_err(DurableError::Source)?.is_some() {
                        return Err(DurableError::Conflict(
                            "recovered private Text schema was not selected from PG",
                        ));
                    }
                    let member = session.read_current_member(
                        &path,
                        max_member_bytes,
                        deadline,
                        cancelled,
                    )?;
                    context.files.push(cmd::SourceFile {
                        path,
                        raw: member.raw,
                    });
                }
                context
                    .files
                    .sort_by(|left, right| left.path.cmp(&right.path));
                super::check_context_budget(
                    &context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                context.check().map_err(DurableError::Source)?;
                Ok((context, identity))
            },
        )
        .map_err(super::source_error)?;
    if readset.selection().identity() != identity {
        return Err(SourceCommandError::Conflict(
            "recovered private Text schema selection changed",
        ));
    }

    let mut worker = crate::source_native_cli::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let private_text = select_creation_private_text(
        invocation,
        request,
        &context,
        selection,
        &mut worker,
        uid,
        deadline,
        cancelled,
    )?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
    if let Some(private_text) = private_text.as_ref() {
        private_text.profile.verify(deadline, cancelled)?;
        private_text.context.snapshot(deadline, cancelled)?;
    }
    Ok(private_text)
}

fn prepare_id(domain: &str, command_id: &str, family_tag: &str, record_type: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-historical-profile-creation-v1\0".to_vec();
    for value in [domain, command_id, family_tag, record_type] {
        seed.extend_from_slice(&(value.len() as u64).to_be_bytes());
        seed.extend_from_slice(value.as_bytes());
    }
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn replay_binding(
    invocation_raw: &[u8],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
) -> Digest256 {
    fn append(seed: &mut Vec<u8>, bytes: &[u8]) {
        seed.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        seed.extend_from_slice(bytes);
    }

    let capture = software.selection();
    let mut seed = b"tos-pg-historical-profile-replay-binding-v1\0".to_vec();
    append(&mut seed, Digest256::of_bytes(invocation_raw).as_bytes());
    append(&mut seed, software.profile().as_bytes());
    append(&mut seed, capture.source_git_commit.as_bytes());
    append(&mut seed, capture.source_git_tree.as_bytes());
    append(&mut seed, capture.capture_manifest_sha256.as_bytes());
    append(
        &mut seed,
        &(components.members().count() as u64).to_be_bytes(),
    );
    for member in components.members() {
        append(&mut seed, member.path.as_str().as_bytes());
        append(&mut seed, &member.size_bytes.to_be_bytes());
        append(&mut seed, member.sha256.as_bytes());
        append(&mut seed, &member.mode.to_be_bytes());
    }
    Digest256::of_bytes(&seed)
}

fn verify_selected_software_components(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for member in components.members() {
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected software component changed"))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "selected software component fixity differs",
            ));
        }
    }
    Ok(())
}

fn creation_envelope(response: Value, historical: bool) -> Value {
    json!({
        "schema_version": if historical {
            "tos_local_pg_historical_creation_result_v1"
        } else {
            "tos_local_pg_public_profile_creation_result_v1"
        },
        "authentication":"local-unix-account",
        "result":response,
        "grants_admission":false
    })
}

fn response(
    request: &tos_foundation::JsonValue,
    owner_config: &tos_foundation::JsonValue,
    attempt: &PgCreationAttempt,
    receipt: &crate::DurableCommitReceipt,
    read_work: crate::source_cohort::revision::SourceRevisionReadWork,
    replay_verification: bool,
) -> SourceCommandResult<Value> {
    let operation = cmd::text(request, "operation")?;
    let record = cmd::field(request, "record")?;
    let record_id = cmd::text(record, "record_id")?;
    let record_type = cmd::text(record, "record_type")?;
    let source_path = cmd::text(owner_config, "source_path")?;
    // Historical/Profile grants select their family or registry profile, not
    // a corpus record_type field. Admission already bound that type in attempt.
    if !["historical.create", "source.create"].contains(&operation)
        || record_id != cmd::text(owner_config, "record_id")?
        || record_type != attempt.record_type
        || !attempt
            .writes
            .iter()
            .any(|write| write.subject == source_path)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Historical/Profile response identity differs",
        ));
    }
    let created_source_paths = attempt
        .writes
        .iter()
        .map(|write| write.subject.clone())
        .collect::<Vec<_>>();
    Ok(json!({
        "operation":operation,
        "record_id":record_id,
        "record_type":record_type,
        "source_path":source_path,
        "replayed":receipt.replayed,
        "created_source_paths":created_source_paths,
        "durable_commit":super::commit_result(receipt),
        "source_read_work": if replay_verification {
            super::source_read_work(
                crate::source_cohort::revision::SourceRevisionReadWork::default(),
                "durable replay verification; current source read predicates are checked by the writer",
            )
        } else {
            super::source_read_work(
                read_work,
                "bounded PG/STO Historical/Profile source selection; software capture and physical I/O excluded",
            )
        },
    }))
}

fn is_private_path(path: &RelativePath) -> bool {
    path.as_str()
        .split('/')
        .any(|part| ["owner-local", "payload", "local-content", "catalog"].contains(&part))
}

fn add_owner_paths(
    closure: &SourceRevisionIdentityClosure,
    pending: &mut BTreeSet<RelativePath>,
    required: &mut BTreeSet<RelativePath>,
) -> SourceCommandResult<()> {
    for owner in closure.owners().values().flatten() {
        let path = owner.1.clone();
        if is_private_path(&path) {
            return Err(SourceCommandError::Denied(
                "Historical Claim reference resolves to private source content",
            ));
        }
        required.insert(path.clone());
        pending.insert(path);
    }
    Ok(())
}

fn select_context_from_pg(
    session: &mut SourceRevisionReadSession<'_>,
    mut context: cmd::CommandContext,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(cmd::CommandContext, PgFamilyCreationObservations), DurableError> {
    let requirements =
        crate::source_creation::pg_family::requirements(&context).map_err(DurableError::Source)?;
    let home = session.observe_home_absence(&requirements.home, deadline, cancelled)?;
    let unique = requirements
        .unique_tokens
        .iter()
        .map(|item| session.observe_unique_index(&item.kind, &item.token, deadline, cancelled))
        .collect::<Result<Vec<_>, _>>()?;
    let identity_closure =
        session.lookup_source_identities(&requirements.reference_tokens, deadline, cancelled)?;
    let dependency_generation =
        session.observe_dependency_commitment(COMMITMENT_VERSION, deadline, cancelled)?;

    let mut selected = BTreeMap::<RelativePath, Vec<u8>>::new();
    let mut visited = BTreeSet::<RelativePath>::new();
    let mut required = BTreeSet::<RelativePath>::new();
    let mut dependencies = BTreeSet::<RelativePath>::new();
    let mut total = context
        .files
        .iter()
        .try_fold(0u64, |sum, file| {
            sum.checked_add(file.raw.len() as u64)
                .filter(|value| *value <= max_total_bytes)
        })
        .ok_or(DurableError::Refused(
            "PG Historical/Profile selected context byte budget",
        ))?;

    loop {
        let current = crate::source_creation::pg_family::requirements(&context)
            .map_err(DurableError::Source)?;
        let mut pending = BTreeSet::new();
        for path in &current.required_source_inputs {
            required.insert(path.clone());
            pending.insert(path.clone());
        }
        for path in &current.source_refs {
            pending.insert(path.clone());
        }
        add_owner_paths(&identity_closure, &mut pending, &mut required)
            .map_err(DurableError::Source)?;
        let missing_required = required.iter().any(|path| {
            !selected.contains_key(path)
                && !context.files.iter().any(|file| file.path == *path)
                && visited.contains(path)
        });
        if missing_required {
            return Err(DurableError::Conflict(
                "PG Historical/Profile required selected source member absent",
            ));
        }
        pending.retain(|path| {
            !visited.contains(path)
                && !selected.contains_key(path)
                && !context.files.iter().any(|file| file.path == *path)
        });
        if pending.is_empty() {
            break;
        }
        while let Some(path) = pending.pop_first() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if is_private_path(&path) {
                return Err(DurableError::Refused(
                    "PG Historical/Profile dependency addresses private source content",
                ));
            }
            if context.files.iter().any(|file| file.path == path) {
                continue;
            }
            if selected.len() + context.files.len() >= max_members {
                return Err(DurableError::Refused(
                    "PG Historical/Profile selected context member budget",
                ));
            }
            let present =
                ReadonlyRecordFiles::has_file(session, path.as_str(), deadline, cancelled)
                    .map_err(DurableError::Source)?;
            if !present {
                if required.contains(&path) || dependencies.contains(&path) {
                    return Err(DurableError::Conflict(
                        "PG Historical/Profile selected dependency absent",
                    ));
                }
                continue;
            }
            let member_cap = max_member_bytes.min(max_total_bytes.saturating_sub(total));
            if member_cap == 0 {
                return Err(DurableError::Refused(
                    "PG Historical/Profile selected context byte budget",
                ));
            }
            let member = session.read_current_member(&path, member_cap, deadline, cancelled)?;
            total = total
                .checked_add(member.raw.len() as u64)
                .filter(|value| *value <= max_total_bytes)
                .ok_or(DurableError::Refused(
                    "PG Historical/Profile selected context byte budget",
                ))?;
            for dependency in member.dependency_claims.as_deref().unwrap_or(&[]) {
                if dependency.as_str().starts_with("ToS/") {
                    dependencies.insert(dependency.clone());
                    pending.insert(dependency.clone());
                }
            }
            selected.insert(path, member.raw);
        }
        // The next requirements pass must see the registry and other inputs
        // selected in this round before it expands the exact profile route.
        for (path, raw) in std::mem::take(&mut selected) {
            context.files.push(cmd::SourceFile { path, raw });
        }
        context
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
    }
    context
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    super::check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)
        .map_err(DurableError::Source)?;
    context.check().map_err(DurableError::Source)?;
    Ok((
        context,
        PgFamilyCreationObservations {
            family: requirements.family,
            record_type: requirements.record_type,
            home,
            unique,
            identity_closure,
            dependency_generation,
        },
    ))
}

/// Execute one exact HistoricalV1/V2 or generic PublicProfile creation route.
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PG Historical/Profile invocation profile",
        ));
    }
    let mut invocation_keys = vec![
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
    ];
    if invocation.get("native_text_read_profile").is_some() {
        invocation_keys.push("native_text_read_profile");
        if invocation
            .get("native_text_read_profile")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(SourceCommandError::Invalid(
                "PG source.create private read profile path",
            ));
        }
    }
    exact(invocation, &invocation_keys)?;
    let selection = super::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = super::invocation_budgets(invocation)?;
    let _ = super::absolute(text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let expected_executable_digest = super::digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable_digest {
        return Err(SourceCommandError::Conflict(
            "native Historical/Profile command executable identity",
        ));
    }

    let owner_path = super::absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) =
        CreationFilesystem::select_creation_owner(&owner_path, deadline, cancelled)?;
    let owner_config = cmd::parse(&owner_raw)?;
    let family = CreationFamily::parse(cmd::text(&owner_config, "schema_version")?)?;
    if !matches!(
        family,
        CreationFamily::HistoricalV1 | CreationFamily::HistoricalV2 | CreationFamily::PublicProfile
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile owner family",
        ));
    }
    let request_raw = super::request_bytes(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile source request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile operation",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let record_type = cmd::text(record, "record_type")?.to_owned();
    if operation == family.operation() {
        let command_id = cmd::text(&request, "command_id")?;
        if command_id.is_empty() || command_id.chars().count() > MAX_JOB_ID_BYTES {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Historical/Profile command identity",
            ));
        }
    }

    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private source store"))?;
    let (software, components) = super::software_selection(invocation, deadline, cancelled)?;
    if components
        .members()
        .any(|member| member.path.as_str().starts_with("ToS/"))
    {
        return Err(SourceCommandError::Denied(
            "Historical/Profile PG software selection cannot replace ToS source members",
        ));
    }
    let current_replay_binding = replay_binding(invocation_raw, &software, &components);
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let family_tag = crate::source_creation::pg_family::family_tag(family)?;
    let prepare_id = if operation == family.operation() {
        Some(prepare_id(
            &selection.domain,
            cmd::text(&request, "command_id")?,
            family_tag,
            &record_type,
        ))
    } else {
        None
    };
    let recovered = if let Some(prepare_id) = prepare_id.as_deref() {
        let configuration_canonical_digest = Digest256::of_bytes(&cmd::canonical(&owner_config)?);
        match db.recover_pg_creation_attempt(
            &store,
            &selection.domain,
            prepare_id,
            cmd::text(&request, "command_id")?,
            Digest256::of_bytes(&request_raw),
            &record_type,
            Digest256::of_bytes(&owner_raw),
            configuration_canonical_digest,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
        ) {
            Ok(attempt) => {
                if attempt.family_tag.as_deref() != Some(family_tag) {
                    return Err(SourceCommandError::Conflict(
                        "PG Historical/Profile ready family differs",
                    ));
                }
                if attempt.replay_binding != Some(current_replay_binding) {
                    return Err(SourceCommandError::Conflict(
                        "PG Historical/Profile invocation or software selection differs from ready attempt",
                    ));
                }
                Some(attempt)
            }
            Err(DurableError::Refused("bounded PG creation attempt absent")) => None,
            Err(error) => return Err(super::source_error(error)),
        }
    } else {
        None
    };

    if let Some(attempt) = recovered {
        verify_selected_software_components(
            &software,
            &components,
            max_member_bytes,
            deadline,
            cancelled,
        )?;
        let selected = db
            .open_source_revision_selection(
                &store,
                &selection.domain,
                &selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
            )
            .map_err(super::source_error)?;
        if selected.contract != selection.contract {
            return Err(SourceCommandError::Conflict(
                "selected source contract differs from current domain",
            ));
        }
        let replay_context = super::selected_context(
            &owner_raw,
            &request_raw,
            selected.cohort.initial_revision(),
            &software,
            &components,
            uid,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
        )?;
        owner_fs.current_context(&replay_context, deadline, cancelled)?;
        let replay_private_text = select_recovered_creation_private_text(
            &mut db,
            &store,
            &selection,
            invocation,
            &request,
            &replay_context,
            family,
            max_members,
            max_total_bytes,
            max_member_bytes,
            uid,
            deadline,
            cancelled,
        )?;
        let owner_check = || {
            if crate::source_text_owner::read_absolute(
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
                    "Historical/Profile PG invocation changed before replay",
                ));
            }
            let (current_owner, current_raw) =
                CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)
                    .map_err(DurableError::Source)?;
            if current_raw != owner_raw {
                return Err(DurableError::Conflict(
                    "Historical/Profile PG owner changed before replay",
                ));
            }
            current_owner
                .current_context(&replay_context, deadline, cancelled)
                .map_err(DurableError::Source)?;
            if let Some(private_text) = replay_private_text.as_ref() {
                private_text
                    .profile
                    .verify(deadline, cancelled)
                    .map_err(DurableError::Source)?;
                private_text
                    .context
                    .snapshot(deadline, cancelled)
                    .map_err(DurableError::Source)?;
            }
            if runtime_observation
                .current_digest(deadline, cancelled)
                .map_err(DurableError::Source)?
                != expected_executable_digest
            {
                return Err(DurableError::Conflict(
                    "native command executable changed before replay",
                ));
            }
            Ok(())
        };
        let (receipt, _) = db
            .commit_pg_creation(
                &store,
                &attempt,
                selection.contract,
                &owner_check,
                deadline,
                cancelled,
            )
            .map_err(super::source_error)?;
        let result = response(
            &request,
            &owner_config,
            &attempt,
            &receipt,
            crate::source_cohort::revision::SourceRevisionReadWork::default(),
            true,
        )?;
        return Ok(creation_envelope(result, family.historical()));
    }

    let ((context, identity, observations), readset) = db
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
                        "selected source contract differs from current domain",
                    ));
                }
                let identity = selected.identity();
                let context = super::selected_context(
                    &owner_raw,
                    &request_raw,
                    selected.cohort.initial_revision(),
                    &software,
                    &components,
                    uid,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let (context, observations) = select_context_from_pg(
                    session,
                    context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )?;
                owner_fs
                    .current_context(&context, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                Ok((context, identity, observations))
            },
        )
        .map_err(super::source_error)?;
    observations
        .identity_closure
        .verify_readset(&readset)
        .map_err(super::source_error)?;
    let mut worker = crate::source_native_cli::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let private_text = if family == CreationFamily::PublicProfile {
        select_creation_private_text(
            invocation,
            &request,
            &context,
            &selection,
            &mut worker,
            uid,
            deadline,
            cancelled,
        )?
    } else {
        if invocation.get("native_text_read_profile").is_some() {
            return Err(SourceCommandError::Denied(
                "PG private Text read grant applies only to PublicProfile creation",
            ));
        }
        None
    };
    let (prepared, readset) = if let Some(private_text) = private_text.as_ref() {
        let ((prepared, observations), readset) = db
            .with_private_owner_read_session(
                &private_text.scope,
                deadline,
                cancelled,
                |private_session, _, _| {
                    let source_client =
                        crate::source_projection_cli::connect_selected_source_pg16().map_err(
                            |_| DurableError::Conflict("selected PostgreSQL private Text source"),
                        )?;
                    let mut source_db = DurablePgCoordinator::from_configured_client(source_client);
                    source_db.with_source_revision_session(
                        &store,
                        &selection.domain,
                        &selection.job_id,
                        selection.job_fence,
                        deadline,
                        cancelled,
                        |session| {
                            if session.selection().identity() != identity {
                                return Err(DurableError::Conflict(
                                    "PG source.create private read source epoch changed",
                                ));
                            }
                            let base = super::selected_context(
                                &owner_raw,
                                &request_raw,
                                session.selection().cohort.initial_revision(),
                                &software,
                                &components,
                                uid,
                                max_members,
                                max_total_bytes,
                                max_member_bytes,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?;
                            let (context, observations) = select_context_from_pg(
                                session,
                                base,
                                max_members,
                                max_total_bytes,
                                max_member_bytes,
                                deadline,
                                cancelled,
                            )?;
                            owner_fs
                                .current_context(&context, deadline, cancelled)
                                .map_err(DurableError::Source)?;
                            let mut reader = CreationPgPrivateTextReader {
                                profile: &private_text.profile,
                                context: &private_text.context,
                                private_session,
                                source_session: session,
                                pins: &private_text.pins,
                                identity,
                                selected_members: BTreeMap::new(),
                            };
                            let prepared =
                                crate::source_creation::pg_family::prepare_pg_family_creation(
                                    context,
                                    identity,
                                    observations.clone(),
                                    &software,
                                    &components,
                                    current_replay_binding,
                                    &mut worker,
                                    Some(&mut reader),
                                    Some(&private_text.profile),
                                    deadline,
                                    cancelled,
                                )
                                .map_err(DurableError::Source)?;
                            Ok((prepared, observations))
                        },
                    )
                },
            )
            .map_err(super::source_error)?;
        observations
            .identity_closure
            .verify_readset(&readset)
            .map_err(super::source_error)?;
        (prepared, readset)
    } else {
        let prepared = crate::source_creation::pg_family::prepare_pg_family_creation(
            context,
            identity,
            observations,
            &software,
            &components,
            current_replay_binding,
            &mut worker,
            None,
            None,
            deadline,
            cancelled,
        )?;
        (prepared, readset)
    };
    if operation == "prepare-create" {
        let response = super::response_value(&prepared.preview()?)?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
        return Ok(creation_envelope(response, family.historical()));
    }
    let serialized = prepared.serialize(&software, &mut worker, deadline, cancelled)?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
    owner_fs.current_context(serialized.context(), deadline, cancelled)?;
    if let Some(private_text) = private_text.as_ref() {
        private_text.profile.verify(deadline, cancelled)?;
        private_text.context.snapshot(deadline, cancelled)?;
    }
    let prepare_id = prepare_id.ok_or(SourceCommandError::Invalid(
        "PG Historical/Profile prepare identity",
    ))?;
    let attempt = db
        .register_pg_creation(
            &store,
            &serialized,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(super::source_error)?;
    let owner_check = || {
        if crate::source_text_owner::read_absolute(
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
                "Historical/Profile PG invocation changed before commit",
            ));
        }
        let (current_owner, current_raw) =
            CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)
                .map_err(DurableError::Source)?;
        if current_raw != owner_raw {
            return Err(DurableError::Conflict(
                "Historical/Profile PG owner changed before commit",
            ));
        }
        current_owner
            .current_context(serialized.context(), deadline, cancelled)
            .map_err(DurableError::Source)?;
        if let Some(private_text) = private_text.as_ref() {
            private_text
                .profile
                .verify(deadline, cancelled)
                .map_err(DurableError::Source)?;
            private_text
                .context
                .snapshot(deadline, cancelled)
                .map_err(DurableError::Source)?;
        }
        if runtime_observation
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable_digest
        {
            return Err(DurableError::Conflict(
                "native command executable changed before commit",
            ));
        }
        Ok(())
    };
    let (receipt, _) = db
        .commit_pg_creation(
            &store,
            &attempt,
            selection.contract,
            &owner_check,
            deadline,
            cancelled,
        )
        .map_err(super::source_error)?;
    let result = response(
        &request,
        &owner_config,
        &attempt,
        &receipt,
        readset.work,
        false,
    )?;
    Ok(creation_envelope(result, family.historical()))
}
