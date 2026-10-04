//! Child module of core_snapshot: caller limits never include an invented stage descriptor.
//! Decode with the existing bounded strict startup JSON route before this owner constructor.
use super::{Admission, JsonAdmission, QueryStoreLimits, QueryStoreSelection, Request, Sources};
use serde::Deserialize;

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
impl Startup {
    /// Only callable inside the native issuer consumer; stage bytes are authenticated by
    /// the existing bind_ticket owner. No fd0 sentinel or caller-selected ticket exists.
    pub fn into_owner_request(
        self,
        original_work_ns: u64,
        caller_retained_state_bytes: usize,
        startup_bytes: usize,
    ) -> super::Result<(Request, super::session_transport::Limits, u64)> {
        if self.schema_version != "tos_native_core_session_startup_v1"
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
        if self.query_store.configured || self.query_store.path.exists() {
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
