//! Exact selected-path observations, before any full Root model/capture selection.
//! A distinct weak profile: no content revision, model, checkpoint or producer-state token.
use super::*;
use std::cell::Cell;
use tos_compiler::private_tmpfs_stage::{PRIVATE_TMPFS_SELECT_COST, PRIVATE_TMPFS_VERIFY_COST};

struct State {
    retained: usize,
    limit: usize,
    reserved: Cell<usize>,
    work: Cell<u64>,
    work_limit: u64,
    visits: Cell<usize>,
}
impl State {
    fn available(&self) -> Result<usize> {
        self.limit
            .checked_sub(self.retained)
            .and_then(|n| n.checked_sub(self.reserved.get()))
            .ok_or("Core probe original simultaneous state")
    }
    fn reserve(&self, bytes: usize) -> Result<()> {
        let n = self
            .reserved
            .get()
            .checked_add(bytes)
            .ok_or("Core probe reservation overflow")?;
        if bytes > self.available()? {
            return Err("Core probe original remaining state");
        }
        self.reserved.set(n);
        Ok(())
    }
    fn release(&self, bytes: usize) {
        self.reserved.set(
            self.reserved
                .get()
                .checked_sub(bytes)
                .expect("owned probe reservation"),
        );
    }
    fn work(&self, bytes: u64) -> Result<()> {
        let n = self
            .work
            .get()
            .checked_add(bytes)
            .filter(|n| *n <= self.work_limit)
            .ok_or("Core probe original aggregate byte work")?;
        self.work.set(n);
        Ok(())
    }
}
struct Workspace<'a>(&'a State);
impl session_transport::Workspace for Workspace<'_> {
    fn reserve(&mut self, bytes: usize) -> Result<()> {
        self.0.reserve(bytes)
    }
    fn release(&mut self, bytes: usize) {
        self.0.release(bytes)
    }
    fn charge_work(&mut self, bytes: u64) -> Result<()> {
        self.0.work(bytes)
    }
}
struct Reserved<'a>(&'a State, usize);
impl Drop for Reserved<'_> {
    fn drop(&mut self) {
        self.0.release(self.1);
    }
}
fn reserve(state: &State, bytes: usize) -> Result<Reserved<'_>> {
    state.reserve(bytes)?;
    Ok(Reserved(state, bytes))
}
fn field<'a>(
    v: &'a tos_foundation::JsonValue,
    name: &str,
) -> Option<&'a tos_foundation::JsonValue> {
    v.as_object()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(name))
        .map(|(_, v)| v)
}
fn call(
    raw: &[u8],
    sequence: u64,
    request: &Request,
    state: &State,
    deadline: Instant,
) -> Result<(ExistsKind, Instant)> {
    active(deadline)?;
    state.work(raw.len() as u64)?; // Before traversal, including invalid/refused calls.
    let mut limits = request.admission.json.limits()?;
    limits.max_visits = limits.max_visits.min(state.visits.get());
    let mut check = || {
        active(deadline).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "Core probe original parse cutoff",
            )
        })
    };
    let fixed = std::mem::size_of::<JsonLimits>()
        + std::mem::size_of::<tos_foundation::JsonDocument>()
        + std::mem::size_of::<Value>()
        + std::mem::size_of::<ExistsKind>()
        + std::mem::size_of::<Operation>()
        + std::mem::size_of::<(
            u64,
            u64,
            &[u8],
            &Request,
            &State,
            &tos_foundation::JsonValue,
            &str,
            Instant,
        )>()
        + std::mem::size_of::<Instant>()
        + std::mem::size_of_val(&check)
        + 4 * std::mem::size_of::<usize>();
    let _local = reserve(state, fixed)?;
    let doc = tos_foundation::parse_json_with_state_budget_and_check(
        raw,
        JsonMode::PublishedStrict,
        limits,
        state.available()?,
        &mut check,
    )
    .map_err(|_| "Core probe original JSON/state admission")?;
    state.visits.set(
        state
            .visits
            .get()
            .checked_sub(doc.visits())
            .ok_or("Core probe aggregate JSON visits")?,
    );
    let root = doc.root();
    let fields = root.as_object().ok_or("Core probe call object")?;
    if fields.len() != 5
        || fields.iter().any(|(k, _)| {
            !matches!(
                k.as_str(),
                Some(
                    "schema_version" | "sequence" | "operation" | "arguments" | "work_deadline_ns"
                )
            )
        })
        || field(root, "schema_version").and_then(tos_foundation::JsonValue::as_str)
            != Some("tos_native_core_session_call_v1")
        || field(root, "sequence").and_then(tos_foundation::JsonValue::as_u64) != Some(sequence)
        || !field(root, "arguments")
            .and_then(tos_foundation::JsonValue::as_object)
            .is_some_and(|f| f.is_empty())
    {
        return Err("Core probe strict call DTO");
    }
    let work = field(root, "work_deadline_ns")
        .and_then(tos_foundation::JsonValue::as_u64)
        .filter(|n| *n > 0 && *n <= request.admission.work_deadline_ns)
        .ok_or("Core probe call cutoff extension")?;
    let id = field(root, "operation")
        .and_then(tos_foundation::JsonValue::as_str)
        .ok_or("Core probe operation")?;
    if !matches!(
        id,
        "tos_corpus_index_exists"
            | "tos_philosophy_projection_exists"
            | "tos_evidence_projection_exists"
            | "tos_philosophy_audit_exists"
    ) {
        return Err("Core probe operation outside admitted profile");
    }
    // Existing Core operation owner is the sole operation-to-selected-path grammar.
    let kind = match operation(id, Value::Object(Default::default()))? {
        Operation::Exists(kind) => kind,
        _ => return Err("Core selected probe owner mismatch"),
    };
    Ok((kind, original_cli_deadline(work)?)) // Document drops before filesystem/send.
}

pub(super) fn run(
    session: &session_owner::Session,
    request: &Request,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    startup_bytes: usize,
) -> Result<()> {
    if request.admission.cold.max_work_bytes == 0 {
        return Err("Core probe original work allowance absent");
    }
    let mut state = State {
        retained: request
            .retained_resource_state_upper_bound()?
            .checked_add(std::mem::size_of::<State>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Workspace>()))
            .ok_or("Core probe fixed state overflow")?,
        limit: request.admission.whole_max_state_bytes,
        reserved: Cell::new(0),
        work: Cell::new(0),
        work_limit: request.admission.cold.max_work_bytes,
        visits: Cell::new(
            request
                .admission
                .json
                .max_visits
                .checked_sub(session.startup_visits)
                .ok_or("Core probe startup visits")?,
        ),
    };
    state.work(startup_bytes as u64)?;
    // Existing Stage owner declares its parsing/verification forecasts. Debit BEFORE
    // execution, even when selection refuses; this never opens or invents a model.
    let setup = reserve(&state, PRIVATE_TMPFS_SELECT_COST.workspace_bytes)?;
    state.work(PRIVATE_TMPFS_SELECT_COST.read_bytes)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core probe actual private Stage refused")?;
    drop(setup);
    // Owner's existing post-selection upper bound covers Ticket heap and held FDs.
    state.retained = state
        .retained
        .checked_add(
            isolation
                .retained_state_upper_bound()
                .map_err(|_| "Core probe Stage retained state")?,
        )
        .ok_or("Core probe Stage retained state overflow")?;
    state.available()?;
    request
        .admission
        .process
        .verify_current()
        .map_err(|_| "Core probe actual process envelope")?;
    // Same actual Linux RAM/swap custody used by the existing Root owner. Its
    // bounded helper uses two <=8194-byte path arrays/membership buffers and <=64-byte
    // scalar observations. This local conservative forecast remains ORIGINAL state/work.
    const KERNEL_WORKSPACE: usize = 64 * 1024;
    const KERNEL_READ_WORK: u64 = 2 * 8194 + 16 * 64;
    let kernel_setup = reserve(&state, KERNEL_WORKSPACE)?;
    state.work(KERNEL_READ_WORK)?;
    let resources = crate::native_cold_resources::LinuxCgroupColdOpenResourceHold::acquire(
        request.admission.working_ram_bytes,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core probe genuine kernel RAM custody")?;
    drop(kernel_setup);
    state.retained = state
        .retained
        .checked_add(
            resources
                .retained_state_upper_bound()
                .map_err(|_| "Core probe kernel custody retained state")?,
        )
        .ok_or("Core probe kernel state overflow")?;
    state.available()?;
    let mut workspace = Workspace(&state);
    let fence = || {
        active(deadline)?;
        session
            .control
            .verify_current()
            .map_err(|_| "Core probe issued control fence")?;
        request
            .admission
            .process
            .verify_current()
            .map_err(|_| "Core probe current process fence")?;
        let held = reserve(&state, PRIVATE_TMPFS_VERIFY_COST.workspace_bytes)?;
        state.work(PRIVATE_TMPFS_VERIFY_COST.read_bytes)?;
        isolation
            .quota_usage()
            .map_err(|_| "Core probe live private Stage fence")?;
        drop(held);
        let kernel = reserve(&state, KERNEL_WORKSPACE)?;
        state.work(KERNEL_READ_WORK)?;
        resources
            .check_current(deadline, cancelled.as_ref())
            .map_err(|_| "Core probe current hard RAM/swap fence")?;
        drop(kernel);
        active(deadline)
    };
    session_transport::run(
        session.control.as_fd(),
        session.limits,
        deadline,
        cancelled.as_ref(),
        &mut workspace,
        |reply| {
            const READY:&[u8]=br#"{"schema_version":"tos_native_core_probe_session_ready_v1","ok":true,"profile":"tos_core_selected_probes_v1","reference_semantics":"cpython_pathlib_is_file_3_14","source_revision":null,"state_reused":false,"capabilities":[{"operation":"tos_corpus_index_exists"},{"operation":"tos_philosophy_projection_exists"},{"operation":"tos_evidence_projection_exists"},{"operation":"tos_philosophy_audit_exists"}]}"#;
            if READY.len()
                > request
                    .http
                    .as_ref()
                    .ok_or("Core probe startup profile absent")?
                    .max_startup_receipt_bytes
            {
                return Err("Core probe startup receipt cap");
            }
            reply.send(session_transport::STARTUP, 0, &[READY])
        },
        |sequence, raw, reply| {
            let (kind, cutoff) = call(raw, sequence, request, &state, deadline)?;
            let cutoff = cutoff.min(deadline);
            reply.narrow_deadline(cutoff)?;
            active(cutoff)?;
            let path = match kind {
                ExistsKind::Index => &request.source_paths.index_path,
                ExistsKind::Philosophy => &request.source_paths.philosophy_graph_projection_path,
                ExistsKind::Evidence => &request.source_paths.evidence_projection_path,
                ExistsKind::PhilosophyAudit => {
                    &request.source_paths.philosophy_post_planting_audit_path
                }
            };
            state.work(
                (path.as_os_str().len() as u64)
                    .checked_mul(4)
                    .ok_or("Core probe path work overflow")?,
            )?;
            let disclose = |exists| {
                const YES:&[u8]=br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":true}"#;
                const NO:&[u8]=br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":false}"#;
                fence()?;
                active(cutoff)?;
                reply.send(
                    session_transport::REPLY,
                    sequence,
                    &[if exists { YES } else { NO }],
                )?;
                active(cutoff)?;
                fence()
            };
            let workspace = SELECTED_PROBE_METADATA_WORKSPACE
                .checked_add(std::mem::size_of_val(&disclose))
                .and_then(|n| n.checked_add(std::mem::size_of::<Result<()>>()))
                .ok_or("Core probe metadata workspace overflow")?;
            let held = reserve(&state, workspace)?;
            with_selected_is_file(path, cutoff, disclose)?;
            drop(held);
            Ok(())
        },
        fence,
    )
}
