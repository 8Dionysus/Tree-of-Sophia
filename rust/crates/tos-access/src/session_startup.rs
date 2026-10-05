//! Child module of core_snapshot: caller limits never include an invented stage descriptor.
//! Decode with the existing bounded strict startup JSON route before this owner constructor.
use super::{Admission, JsonAdmission, QueryStoreLimits, QueryStoreSelection, Request, Sources};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdmissionLimits {
    max_build_seconds: u64,
    tmpfs_quota_bytes: u64,
    inode_limit: u64,
    working_ram_bytes: u64,
    whole_max_rows: u64,
    whole_max_row_bytes: usize,
    whole_max_graph_bytes: usize,
    whole_max_catalog_bytes: usize,
    whole_max_catalog_inputs_bytes: usize,
    whole_max_state_bytes: usize,
    json: JsonAdmission,
    operation_seconds: f64,
    work_deadline_ns: u64,
    cold: tos_compiler::ColdOpenLimits,
    process: tos_compiler::NativeProcessLimits,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Startup {
    schema_version: String,
    admission: AdmissionLimits,
    source_paths: Sources,
    query_store: QueryStoreSelection,
    query_store_limits: Option<QueryStoreLimits>,
    http: crate::core_http_admission::HttpAdmission,
    session: super::session_transport::Limits,
    original_whole_deadline_ns: u64,
}

/// Small ordinary-session DTO. Limits and HTTP/query-store budgets are minted
/// by this native owner from the live stage ticket and process envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OrdinaryStartup {
    schema_version: String,
    source_paths: Sources,
    query_store: QueryStoreSelection,
    session: super::session_transport::Limits,
    original_whole_deadline_ns: u64,
}

/// Small non-session native snapshot DTO. Resource and query profiles are
/// constructed here; the SDK supplies only the selected request and deadline.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeOrdinarySnapshotStartup {
    schema_version: String,
    arguments: Value,
    source_paths: Sources,
    query_store: QueryStoreSelection,
    original_whole_deadline_ns: u64,
}

impl NativeOrdinarySnapshotStartup {
    pub fn into_owner_request(
        self,
        operation: &str,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<Request> {
        if self.schema_version != "tos_native_core_ordinary_snapshot_startup_v1"
            || original_work_ns == 0
            || original_work_ns >= self.original_whole_deadline_ns
        {
            return Err("Core ordinary snapshot original deadline association");
        }
        if !self.arguments.is_object() {
            return Err("Core ordinary snapshot arguments object required");
        }

        let stage = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::
            select_issued_from_environment()
            .map_err(|_| "Core ordinary snapshot actual stage ticket refused")?;
        let (tmpfs_quota_bytes, inode_limit, working_ram_bytes) = stage.resource_limits();
        drop(stage);
        let process = actual_process_limits()?;
        let limits = AdmissionLimits::native_ordinary(
            original_work_ns,
            tmpfs_quota_bytes,
            inode_limit,
            working_ram_bytes,
            process,
        )?;
        let stage_ticket_fd = std::env::var("ABYSS_STAGE_TICKET_FD")
            .map_err(|_| "Core ordinary snapshot actual stage ticket absent")?
            .parse::<i32>()
            .map_err(|_| "Core ordinary snapshot stage ticket descriptor")?;
        if stage_ticket_fd < 3 {
            return Err("Core ordinary snapshot stage ticket descriptor bound");
        }
        let admission = Admission {
            max_build_seconds: limits.max_build_seconds,
            tmpfs_quota_bytes: limits.tmpfs_quota_bytes,
            inode_limit: limits.inode_limit,
            working_ram_bytes: limits.working_ram_bytes,
            whole_max_rows: limits.whole_max_rows,
            whole_max_row_bytes: limits.whole_max_row_bytes,
            whole_max_graph_bytes: limits.whole_max_graph_bytes,
            whole_max_catalog_bytes: limits.whole_max_catalog_bytes,
            whole_max_catalog_inputs_bytes: limits.whole_max_catalog_inputs_bytes,
            whole_max_state_bytes: limits.whole_max_state_bytes,
            json: limits.json,
            operation_seconds: limits.operation_seconds,
            work_deadline_ns: limits.work_deadline_ns,
            stage_ticket_fd,
            cold: limits.cold,
            process: limits.process,
        };
        admission.deadline()?;
        super::bind_ticket(&admission)?;
        self.source_paths.validate()?;
        if !self.query_store.path.is_absolute()
            || self.query_store.path.as_os_str().len() > 8193
        {
            return Err("Core ordinary snapshot QueryStore selector");
        }
        let http = if matches!(operation, "tos_native_call" | "tos_native_serve") {
            Some(crate::core_http_admission::HttpAdmission::native_ordinary()?)
        } else {
            None
        };
        let caller_retained_state_bytes = caller_retained_state_bytes
            .checked_add(startup_bytes)
            .ok_or("Core ordinary snapshot startup retained state overflow")?;
        Ok(Request {
            caller_retained_state_bytes,
            admission,
            source_paths: self.source_paths,
            arguments: self.arguments,
            query_store: self.query_store,
            query_store_limits: Some(QueryStoreLimits::native_ordinary()),
            http,
        })
    }
}

impl OrdinaryStartup {
    pub fn into_owner_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        if self.schema_version != "tos_native_core_ordinary_session_startup_v1"
            || original_work_ns >= self.original_whole_deadline_ns
        {
            return Err("Core ordinary original startup association");
        }
        let stage = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::
            select_issued_from_environment()
            .map_err(|_| "Core ordinary actual stage ticket refused")?;
        let (tmpfs_quota_bytes, inode_limit, working_ram_bytes) = stage.resource_limits();
        drop(stage);
        let process = actual_process_limits()?;
        let admission = AdmissionLimits::native_ordinary(
            original_work_ns,
            tmpfs_quota_bytes,
            inode_limit,
            working_ram_bytes,
            process,
        )?;
        let startup = Startup {
            schema_version: self.schema_version,
            admission,
            source_paths: self.source_paths,
            query_store: self.query_store,
            query_store_limits: Some(QueryStoreLimits::native_ordinary()),
            http: crate::core_http_admission::HttpAdmission::native_ordinary()?,
            session: self.session,
            original_whole_deadline_ns: self.original_whole_deadline_ns,
        };
        startup.into_profile_request(
            original_work_ns,
            caller_retained_state_bytes,
            startup_bytes,
            "tos_native_core_ordinary_session_startup_v1",
            false,
        )
    }
}

impl AdmissionLimits {
    pub(super) fn native_ordinary(
        work_deadline_ns: u64,
        tmpfs_quota_bytes: u64,
        inode_limit: u64,
        working_ram_bytes: u64,
        process: tos_compiler::NativeProcessLimits,
    ) -> super::Result<Self> {
        let now = monotonic_ns()?;
        let remaining = work_deadline_ns
            .checked_sub(now)
            .filter(|remaining| *remaining > 0)
            .ok_or("Core ordinary original work deadline expired")?;
        let max_build_seconds = remaining.div_ceil(1_000_000_000);
        // Retained state must leave half of the tighter live RAM/AS envelope
        // for framework, SQLite, and transient decoding allocations.
        let whole_max_state_bytes = usize::try_from(
            working_ram_bytes.min(process.address_space_bytes) / 2,
        )
        .ok()
        .filter(|bytes| *bytes > 0)
        .ok_or("Core ordinary retained-state envelope too small")?;
        // Cold source copy, VACUUM output, and SQLite TEMP can coexist in the
        // issued stage; cap one model file to one third of quota and RLIMIT_FSIZE.
        let max_file_bytes = process.file_size_bytes.min(tmpfs_quota_bytes / 3);
        if max_file_bytes == 0 {
            return Err("Core ordinary model-file envelope too small");
        }
        Ok(Self {
            max_build_seconds,
            tmpfs_quota_bytes,
            inode_limit,
            working_ram_bytes,
            whole_max_rows: 10_000_000,
            whole_max_row_bytes: 16 * 1024 * 1024,
            whole_max_graph_bytes: 64 * 1024 * 1024,
            whole_max_catalog_bytes: 64 * 1024 * 1024,
            whole_max_catalog_inputs_bytes: 16 * 1024 * 1024,
            whole_max_state_bytes,
            json: JsonAdmission {
                max_bytes: 16 * 1024 * 1024,
                max_depth: 64,
                max_visits: 1_000_000,
                max_integer_digits: 64,
            },
            operation_seconds: max_build_seconds as f64,
            work_deadline_ns,
            cold: tos_compiler::ColdOpenLimits {
                max_file_bytes,
                max_vm_steps: 1_000_000_000_000,
                sqlite_cache_kib: 64 * 1024,
                max_rows: 10_000_000,
                max_work_bytes: 16 * 1024 * 1024 * 1024,
                max_row_bytes: 16 * 1024 * 1024,
                max_metadata_bytes: 4 * 1024 * 1024,
                max_sources: 4096,
            },
            process,
        })
    }
}

fn monotonic_ns() -> super::Result<u64> {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0
        || now.tv_sec < 0
        || now.tv_nsec < 0
    {
        return Err("Core ordinary monotonic clock unavailable");
    }
    u64::try_from(now.tv_sec)
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .and_then(|ns| ns.checked_add(now.tv_nsec as u64))
        .ok_or("Core ordinary monotonic clock overflow")
}

fn actual_process_limits() -> super::Result<tos_compiler::NativeProcessLimits> {
    let mut address_space = unsafe { std::mem::zeroed::<libc::rlimit>() };
    let mut file_size = unsafe { std::mem::zeroed::<libc::rlimit>() };
    if unsafe { libc::getrlimit(libc::RLIMIT_AS as _, &mut address_space) } != 0
        || unsafe { libc::getrlimit(libc::RLIMIT_FSIZE as _, &mut file_size) } != 0
    {
        return Err("Core ordinary live process envelope unavailable");
    }
    let limits = tos_compiler::NativeProcessLimits {
        address_space_bytes: u64::try_from(address_space.rlim_cur)
            .map_err(|_| "Core ordinary address-space envelope range")?,
        file_size_bytes: u64::try_from(file_size.rlim_cur)
            .map_err(|_| "Core ordinary file-size envelope range")?,
    };
    limits
        .verify_current()
        .map_err(|_| "Core ordinary process envelope is not finite/current")?;
    Ok(limits)
}

impl QueryStoreLimits {
    fn native_ordinary() -> Self {
        Self {
            max_database_bytes: 16 * 1024 * 1024 * 1024,
            max_input_bytes: 64 * 1024 * 1024,
            max_json_bytes: 16 * 1024 * 1024,
            max_rows: 10_000_000,
            max_work_steps: 1_000_000_000_000,
            max_sql_vm_steps: 1_000_000_000_000,
            sqlite_cache_kib: 64 * 1024,
        }
    }
}

impl Startup {
    /// Only callable inside the native issuer consumer; stage bytes are authenticated by
    /// the existing bind_ticket owner. No fd0 sentinel or caller-selected ticket exists.
    pub fn into_owner_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        self.into_profile_request(
            original_work_ns,
            caller_retained_state_bytes,
            startup_bytes,
            "tos_native_core_session_startup_v1",
            true,
            false,
        )
    }
    pub fn into_probe_owner_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        self.into_profile_request(
            original_work_ns,
            caller_retained_state_bytes,
            startup_bytes,
            "tos_native_core_probe_session_startup_v1",
            false,
            false,
        )
    }
    pub fn into_lazy_owner_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        self.into_profile_request(
            original_work_ns,
            caller_retained_state_bytes,
            startup_bytes,
            "tos_native_core_lazy_session_startup_v1",
            false,
            false,
        )
    }
    fn into_profile_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
        schema: &str,
        require_root: bool,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        if self.schema_version != schema
            || self.admission.work_deadline_ns != original_work_ns
            || original_work_ns >= self.original_whole_deadline_ns
        {
            return Err("Core session original startup association");
        }
        let actual_stage_fd = std::env::var("ABYSS_STAGE_TICKET_FD")
            .map_err(|_| "Core session actual issuer stage ticket absent")?
            .parse::<i32>()
            .map_err(|_| "Core session issuer stage descriptor")?;
        if actual_stage_fd < 3 {
            return Err("Core session actual stage descriptor bound");
        }
        let a = self.admission;
        let admission = Admission {
            max_build_seconds: a.max_build_seconds,
            tmpfs_quota_bytes: a.tmpfs_quota_bytes,
            inode_limit: a.inode_limit,
            working_ram_bytes: a.working_ram_bytes,
            whole_max_rows: a.whole_max_rows,
            whole_max_row_bytes: a.whole_max_row_bytes,
            whole_max_graph_bytes: a.whole_max_graph_bytes,
            whole_max_catalog_bytes: a.whole_max_catalog_bytes,
            whole_max_catalog_inputs_bytes: a.whole_max_catalog_inputs_bytes,
            whole_max_state_bytes: a.whole_max_state_bytes,
            json: a.json,
            operation_seconds: a.operation_seconds,
            work_deadline_ns: a.work_deadline_ns,
            stage_ticket_fd: actual_stage_fd,
            cold: a.cold,
            process: a.process,
        };
        admission.deadline()?;
        super::bind_ticket(&admission)?;
        self.source_paths.validate()?;
        if !self.query_store.path.is_absolute() || self.query_store.path.as_os_str().len() > 8193 {
            return Err("Core session query store selector");
        }
        // First implementation requires captured Root. A selected existing QueryStore
        // retains its independent five-input owner; never silently rebuild it as Root.
        if require_root && (self.query_store.configured || self.query_store.path.exists()) {
            return Err("Core session selected QueryStore owner unsupported");
        }
        let session = self.session.validate()?;
        if session.max_total_request_bytes
            > admission
                .json
                .max_bytes
                .checked_sub(startup_bytes)
                .ok_or("Core session startup original JSON bytes")? as u64
            || session.max_total_reply_bytes
                > self
                    .http
                    .profile()?
                    .max_response_bytes
                    .min(admission.whole_max_graph_bytes)
                    .min(super::OUTPUT_CAP) as u64
            || session.max_call_bytes > admission.json.max_bytes.min(super::INPUT_CAP)
            || session.max_reply_bytes
                > self
                    .http
                    .profile()?
                    .max_mcp_frame_bytes
                    .min(super::OUTPUT_CAP)
        {
            return Err("Core session transport exceeds original query allowance");
        }
        Ok((
            Request {
                caller_retained_state_bytes,
                admission,
                source_paths: self.source_paths,
                arguments: serde_json::Value::Null,
                query_store: self.query_store,
                query_store_limits: self.query_store_limits,
                http: Some(self.http),
            },
            session,
            self.original_whole_deadline_ns,
        ))
    }
}
