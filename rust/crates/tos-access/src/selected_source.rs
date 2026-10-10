//! Installed exact-source owner paired with the prepared publication fence.
use crate::source_read::{Operation, Request};
use crate::{AccessError, AccessErrorCode, AccessProfile, DisclosureFence, PreparedPacket};
use std::{
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_command::source_read_owner::{
    ExactWorkerIdentity, SelectedSourceReadOwner, SourceReadOperation, SourceReadPacket,
};
use tos_query::AbortProbe;

pub(crate) struct Selection {
    owner: SelectedSourceReadOwner,
    worker: Arc<crate::site::InstalledSchemaWorker>,
    timeout: Duration,
}
fn unavailable() -> AccessError {
    AccessError::new(
        AccessErrorCode::Unavailable,
        "explicit installed source owner unavailable",
    )
}
fn owner_error(error: tos_command::source_command::SourceCommandError) -> AccessError {
    use tos_command::source_command::SourceCommandError as E;
    let (code, message) = match error {
        E::Invalid(message) => (AccessErrorCode::InvalidRequest, message),
        E::Conflict(message) => (AccessErrorCode::StaleSelection, message),
        E::Denied(message) => (AccessErrorCode::PolicyDenied, message),
        E::DeniedWithReason(_) => (AccessErrorCode::PolicyDenied, "source owner access refused"),
        E::Unsupported(message) => (AccessErrorCode::Unavailable, message),
        E::SchemaExecution { .. } => (
            AccessErrorCode::InvalidRequest,
            "exact-source schema execution refused",
        ),
        E::MissingProductionAdmission => (
            AccessErrorCode::PolicyDenied,
            "source owner admission absent",
        ),
    };
    AccessError::new(code, message)
}
impl Selection {
    pub fn open(
        root: &Path,
        inputs: &Path,
        local: Option<&Path>,
        revision: &str,
        profile: AccessProfile,
    ) -> Result<Self, AccessError> {
        // Software admission hashes the selected executable and schema worker.
        // Its absolute startup budget is separate from each request's retained
        // processing probe. Neither site nor worker stores this
        // admission probe; subsequent requests and final flush use fresh request
        // probes without inheriting an expired server-construction deadline.
        let probe = profile
            .with_query_timeout(Duration::from_secs(30))
            .deadline_probe();
        // Same metadata ceiling as PreparedSourceInputs::parse. Never load an unbounded vector.
        let mut file =
            tos_fd_open::open_absolute_regular(inputs, 1_048_576).map_err(|_| unavailable())?;
        let mut raw = Vec::new();
        file.by_ref()
            .take(1_048_577)
            .read_to_end(&mut raw)
            .map_err(|_| unavailable())?;
        if raw.len() > 1_048_576 {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "source vector byte budget exceeded",
            ));
        }
        Self::open_raw_with_probe(root, &raw, local, revision, profile, probe)
    }
    /// Retained immutable vector transport; it does not require a Knowledge
    /// prepared-publication selector or manufacture such a disclosure fence.
    pub(crate) fn open_raw_with_probe(
        root: &Path,
        raw: &[u8],
        local: Option<&Path>,
        revision: &str,
        profile: AccessProfile,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<Self, AccessError> {
        let timeout = profile.query_timeout.ok_or_else(unavailable)?;
        if raw.len() > 1_048_576 {
            return Err(unavailable());
        }
        crate::knowledge::check_abort(&probe)?;
        let site = crate::site::SoftwareSite::installed(Arc::clone(&probe))?;
        let worker = Arc::new(site.source_schema_worker(512 * 1024 * 1024, Arc::clone(&probe))?);
        let identity = ExactWorkerIdentity {
            absolute_path: worker.path().to_owned(),
            sha256: worker.sha256(),
        };
        let owner = SelectedSourceReadOwner::open(root, raw, revision, identity, local)
            .map_err(owner_error)?;
        worker.verify()?;
        crate::knowledge::check_abort(&probe)?;
        Ok(Self {
            owner,
            worker,
            timeout,
        })
    }
    pub(crate) fn software(&self) -> Arc<crate::site::SoftwareSite> {
        self.worker.software()
    }
    pub fn source_inputs_sha256(&self) -> &str {
        self.owner.source_inputs_sha256()
    }
    pub fn deadline(&self) -> Result<Instant, AccessError> {
        Instant::now()
            .checked_add(self.timeout)
            .ok_or_else(unavailable)
    }
    pub fn prepare(
        &self,
        request: Request,
        prepared: Box<dyn DisclosureFence>,
        deadline: Instant,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        self.prepare_inner(request, Some(prepared), deadline, probe)
    }
    pub(crate) fn prepare_owner(
        &self,
        request: Request,
        deadline: Instant,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        self.prepare_inner(request, None, deadline, probe)
    }
    fn prepare_inner(
        &self,
        request: Request,
        mut prepared: Option<Box<dyn DisclosureFence>>,
        deadline: Instant,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        if let Some(fence) = prepared.as_mut() {
            fence.recheck()?;
        }
        self.worker.verify()?;
        let op = match request.operation {
            Operation::Capabilities => SourceReadOperation::Capabilities,
            Operation::Contract => SourceReadOperation::Contract,
            Operation::Discover => SourceReadOperation::Discover,
            Operation::Read => SourceReadOperation::Read,
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let packet = self
            .owner
            .prepare(op, &request.body, deadline, Arc::clone(&cancelled))
            .map_err(owner_error)?;
        crate::knowledge::check_abort(&probe)?;
        let body = crate::source_read::portable_packet(request.operation, packet.body().to_vec())?;
        let mut fence = Fence {
            packet,
            prepared,
            worker: Arc::clone(&self.worker),
            deadline,
            cancelled,
            probe,
        };
        fence.recheck()?;
        Ok(PreparedPacket {
            body,
            fence: Box::new(fence),
        })
    }
}
struct Fence {
    packet: SourceReadPacket,
    prepared: Option<Box<dyn DisclosureFence>>,
    worker: Arc<crate::site::InstalledSchemaWorker>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    probe: Arc<dyn AbortProbe>,
}
impl DisclosureFence for Fence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        if let Some(fence) = self.prepared.as_mut() {
            fence.recheck()?;
        }
        self.worker.verify()?;
        self.packet
            .verify_current(self.deadline, &self.cancelled)
            .map_err(owner_error)?;
        crate::knowledge::check_abort(&self.probe)
    }
}
