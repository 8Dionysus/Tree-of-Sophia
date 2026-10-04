//! Borrowed HTTP delivery over Core's exact held legacy SQL selection.
use super::*;
use crate::{AccessError, AccessErrorCode, DisclosureFence, PreparedPacket, ScopedAccessExecutor};
use std::sync::Mutex;
fn denied(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::Unavailable, message)
}
struct StoreFence<'a> {
    store: &'a Mutex<tos_query::source_diagnostic::LegacyStore>,
    resources: &'a crate::native_cold_resources::LinuxCgroupColdOpenResourceHold,
    request: &'a Request,
    deadline: Instant,
    cancelled: &'a Arc<AtomicBool>,
    probe: Arc<dyn tos_query::AbortProbe>,
}
impl DisclosureFence for StoreFence<'_> {
    fn recheck(&mut self) -> std::result::Result<(), AccessError> {
        if self.probe.reason().is_some() {
            return Err(denied("legacy query operation expired"));
        }
        self.store
            .lock()
            .map_err(|_| denied("legacy query hold poisoned"))?
            .verify_currentness()
            .map_err(|_| denied("legacy query store changed"))?;
        self.resources
            .check_current(self.deadline, self.cancelled.as_ref())
            .map_err(|_| denied("legacy query resources changed"))?;
        self.request
            .admission
            .process
            .verify_current()
            .map_err(|_| denied("legacy query process changed"))
    }
}
struct StoreExecutor<'a> {
    store: &'a Mutex<tos_query::source_diagnostic::LegacyStore>,
    resources: &'a crate::native_cold_resources::LinuxCgroupColdOpenResourceHold,
    request: &'a Request,
    deadline: Instant,
    cancelled: &'a Arc<AtomicBool>,
}
impl<'a> StoreExecutor<'a> {
    fn packet(
        &self,
        tool: &str,
        args: Value,
        probe: Arc<dyn tos_query::AbortProbe>,
    ) -> std::result::Result<PreparedPacket<'a>, AccessError> {
        let mut fence = StoreFence {
            store: self.store,
            resources: self.resources,
            request: self.request,
            deadline: self.deadline,
            cancelled: self.cancelled,
            probe: probe.clone(),
        };
        fence.recheck()?;
        let result = self
            .store
            .lock()
            .map_err(|_| denied("legacy query hold poisoned"))?
            .query_call_with_probe(
                tool,
                &args,
                self.request.admission.whole_max_state_bytes,
                probe,
            )
            .map_err(|_| denied("legacy query operation refused"))?;
        let response_cap = self
            .request
            .http
            .as_ref()
            .map(|http| http.profile())
            .transpose()
            .map_err(denied)?
            .map(|profile| profile.max_response_bytes)
            .unwrap_or(OUTPUT_CAP);
        let mut output = BoundedOutput::new(OUTPUT_CAP.min(response_cap), self.deadline);
        output.value(&result).map_err(denied)?;
        fence.recheck()?;
        Ok(PreparedPacket {
            body: output.bytes,
            fence: Box::new(fence),
        })
    }
}
impl<'a> ScopedAccessExecutor<'a> for StoreExecutor<'a> {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: crate::Params,
        _: Arc<dyn tos_query::AbortProbe>,
    ) -> std::result::Result<PreparedPacket<'a>, AccessError> {
        Err(denied(
            "source descent unavailable on selected legacy QueryStore",
        ))
    }
    fn knowledge_available(&self, operation: crate::KnowledgeOperation) -> bool {
        matches!(
            operation,
            crate::KnowledgeOperation::Catalog
                | crate::KnowledgeOperation::Node
                | crate::KnowledgeOperation::Relation
        )
    }
    fn knowledge(
        &self,
        request: crate::KnowledgeRequest,
        probe: Arc<dyn tos_query::AbortProbe>,
    ) -> std::result::Result<PreparedPacket<'a>, AccessError> {
        match request {
            crate::KnowledgeRequest::Catalog => {
                self.packet("tos_knowledge_catalog", serde_json::json!({}), probe)
            }
            crate::KnowledgeRequest::Node {
                node_id,
                relation_limit,
            } => self.packet(
                "tos_knowledge_node",
                serde_json::json!({"node_id":node_id,"relation_limit":relation_limit}),
                probe,
            ),
            crate::KnowledgeRequest::Relation { relation_id } => self.packet(
                "tos_knowledge_relation",
                serde_json::json!({"relation_id":relation_id}),
                probe,
            ),
            _ => Err(denied(
                "selected legacy QueryStore knowledge route unavailable",
            )),
        }
    }
    fn knowledge_search_legacy_available(&self) -> bool {
        true
    }
    fn knowledge_search_legacy(
        &self,
        r: tos_query::knowledge_legacy_search::LegacySearchRequest,
        probe: Arc<dyn tos_query::AbortProbe>,
    ) -> std::result::Result<PreparedPacket<'a>, AccessError> {
        self.packet("tos_knowledge_search",serde_json::json!({"query":r.query,"sources":r.sources,"kind_ids":r.kind_ids,"predicate_ids":r.predicate_ids,"offset":r.offset,"limit":r.limit}),probe)
    }
}
pub(super) fn serve_legacy_store(
    store: tos_query::source_diagnostic::LegacyStore,
    resources: &crate::native_cold_resources::LinuxCgroupColdOpenResourceHold,
    request: &Request,
    listen: &str,
    max_connections: u64,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<()> {
    let http = request
        .http
        .as_ref()
        .ok_or("Core legacy HTTP explicit allowances absent")?;
    let profile = http.profile()?;
    let store = Mutex::new(store);
    let executor = StoreExecutor {
        store: &store,
        resources,
        request,
        deadline,
        cancelled,
    };
    let probe: Arc<dyn tos_query::AbortProbe> = Arc::new(CoreQueryProbe {
        deadline,
        cancelled: cancelled.clone(),
    });
    let mut fence = StoreFence {
        store: &store,
        resources,
        request,
        deadline,
        cancelled,
        probe,
    };
    fence
        .recheck()
        .map_err(|_| "Core legacy HTTP initial fence refused")?;
    // Readiness names the authentic weak SQL carrier, never a source epoch or
    // capture declaration. The same hold survives every socket disclosure.
    let revision = store
        .lock()
        .map_err(|_| "Core legacy HTTP hold poisoned")?
        .revision
        .clone();
    let receipt = serde_json::json!({"schema_version":"tos_native_core_legacy_http_ready_v1","listen":listen,"source_revision":revision,"state_profile":"tos_query_store_v1","selected_store_authenticated":true,"source_capture_available":false});
    let mut startup = BoundedOutput::new(http.max_startup_receipt_bytes, deadline);
    startup.value(&receipt)?;
    startup.literal(b"\n")?;
    disclose_bytes(&startup.bytes, deadline)?;
    fence
        .recheck()
        .map_err(|_| "Core legacy HTTP readiness fence refused")?;
    let finished = AtomicBool::new(false);
    let mut accepted = 0u64;
    crate::http::serve_selected_connections(
        listen,
        profile,
        deadline,
        cancelled,
        &finished,
        |stream, site, profile, control| {
            fence.recheck().map_err(std::io::Error::other)?;
            let result = crate::http::serve_connection_scoped_controlled(
                stream, &executor, site, profile, control,
            );
            accepted = accepted
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("Core legacy HTTP count overflow"))?;
            if accepted >= max_connections {
                finished.store(true, Ordering::Release);
            }
            fence.recheck().map_err(std::io::Error::other)?;
            result
        },
    )
    .map_err(|_| "Core legacy HTTP socket delivery refused")?;
    fence
        .recheck()
        .map_err(|_| "Core legacy HTTP terminal fence refused")
}
