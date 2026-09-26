//! Consumer parsing and selected query dispatch. Query rules and authority stay in QRY.
use crate::{AccessError, AccessErrorCode, PreparedPacket};
use std::sync::Arc;
use tos_foundation::JsonValue;
use tos_query::{AbortProbe, AbortReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeOperation {
    Catalog,
    Node,
    Relation,
    Temporal,
    Lens,
    Explore,
}
impl KnowledgeOperation {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "tos.knowledge.catalog" => Self::Catalog,
            "tos.knowledge.node.inspect" => Self::Node,
            "tos.knowledge.relation.inspect" => Self::Relation,
            "tos.knowledge.temporal.compare" => Self::Temporal,
            "tos.lens.compile" => Self::Lens,
            "tos.knowledge.explore" => Self::Explore,
            _ => return None,
        })
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Catalog => "tos.knowledge.catalog",
            Self::Node => "tos.knowledge.node.inspect",
            Self::Relation => "tos.knowledge.relation.inspect",
            Self::Temporal => "tos.knowledge.temporal.compare",
            Self::Lens => "tos.lens.compile",
            Self::Explore => "tos.knowledge.explore",
        }
    }
}
#[derive(Clone, Debug)]
pub enum KnowledgeRequest {
    Catalog,
    Node {
        node_id: String,
        relation_limit: usize,
    },
    Relation {
        relation_id: String,
    },
    Temporal(JsonValue),
    Lens(JsonValue),
    Explore(JsonValue),
}
impl KnowledgeRequest {
    pub fn operation(&self) -> KnowledgeOperation {
        match self {
            Self::Catalog => KnowledgeOperation::Catalog,
            Self::Node { .. } => KnowledgeOperation::Node,
            Self::Relation { .. } => KnowledgeOperation::Relation,
            Self::Temporal(_) => KnowledgeOperation::Temporal,
            Self::Lens(_) => KnowledgeOperation::Lens,
            Self::Explore(_) => KnowledgeOperation::Explore,
        }
    }
    pub fn from_arguments(
        operation: KnowledgeOperation,
        args: &JsonValue,
    ) -> Result<Self, AccessError> {
        let fields = args
            .as_object()
            .ok_or_else(|| invalid("tool arguments must be an object"))?;
        let allowed: &[&str] = match operation {
            KnowledgeOperation::Catalog => &[],
            KnowledgeOperation::Node => &["node_id", "relation_limit"],
            KnowledgeOperation::Relation => &["relation_id"],
            KnowledgeOperation::Lens => &["spec"],
            KnowledgeOperation::Temporal | KnowledgeOperation::Explore => &["request"],
        };
        if fields
            .iter()
            .any(|(name, _)| !name.as_str().is_some_and(|name| allowed.contains(&name)))
        {
            return Err(invalid("unknown tool argument"));
        }
        let id = |key: &str| {
            args.object_get(key)
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty() && s.chars().count() <= 4096)
                .map(str::to_owned)
                .ok_or_else(|| invalid("inspect identifier must be a nonempty bounded string"))
        };
        Ok(match operation {
            KnowledgeOperation::Catalog => Self::Catalog,
            KnowledgeOperation::Node => {
                let limit =
                    match args.object_get("relation_limit") {
                        None => 200,
                        Some(v) => v.as_u64().filter(|n| *n <= 1000).ok_or_else(|| {
                            invalid("relation_limit must be an integer in 0..1000")
                        })? as usize,
                    };
                Self::Node {
                    node_id: id("node_id")?,
                    relation_limit: limit,
                }
            }
            KnowledgeOperation::Relation => Self::Relation {
                relation_id: id("relation_id")?,
            },
            operation => {
                let key = if operation == KnowledgeOperation::Lens {
                    "spec"
                } else {
                    "request"
                };
                let raw = args
                    .object_get(key)
                    .filter(|v| v.as_object().is_some())
                    .ok_or_else(|| invalid("structured query must be an object"))?
                    .clone();
                match operation {
                    KnowledgeOperation::Temporal => Self::Temporal(raw),
                    KnowledgeOperation::Lens => Self::Lens(raw),
                    _ => Self::Explore(raw),
                }
            }
        })
    }
    pub fn from_body(operation: KnowledgeOperation, body: JsonValue) -> Result<Self, AccessError> {
        if body.as_object().is_none() {
            return Err(invalid("structured query must be an object"));
        }
        match operation {
            KnowledgeOperation::Temporal => Ok(Self::Temporal(body)),
            KnowledgeOperation::Lens => Ok(Self::Lens(body)),
            KnowledgeOperation::Explore => Ok(Self::Explore(body)),
            _ => Err(invalid("operation does not accept a structured body")),
        }
    }
}
fn invalid(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::InvalidRequest, message)
}
pub(crate) fn check_abort(probe: &Arc<dyn AbortProbe>) -> Result<(), AccessError> {
    match probe.reason() {
        None => Ok(()),
        Some(reason) => Err(AccessError::new(
            match reason {
                AbortReason::Cancelled => AccessErrorCode::Cancelled,
                AbortReason::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
            },
            "query interrupted",
        )),
    }
}

/// Moves packet bytes; the query lease remains held by transport through flush.
pub fn from_inspect(packet: tos_query::DisclosableInspect) -> PreparedPacket {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(InspectFence(lease)),
    }
}
pub fn from_catalog(packet: tos_query::DisclosableCatalog) -> PreparedPacket {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(CatalogFence(lease)),
    }
}
pub fn from_indexed_search(packet: tos_query::DisclosableIndexedSearch) -> PreparedPacket {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(SearchFence(lease)),
    }
}
struct InspectFence(Box<dyn tos_query::InspectDisclosureLease>);
impl crate::DisclosureFence for InspectFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}
struct CatalogFence(Box<dyn tos_query::CatalogDisclosureLease>);
impl crate::DisclosureFence for CatalogFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}
struct SearchFence(Box<dyn tos_query::IndexedDisclosureLease>);
impl crate::DisclosureFence for SearchFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}

impl From<tos_query::search_v2::SearchV2Error> for AccessError {
    fn from(error: tos_query::search_v2::SearchV2Error) -> Self {
        use tos_query::search_v2::SearchV2ErrorCode as Q;
        let code = match error.code {
            Q::InvalidRequest | Q::QueryTooLong | Q::QueryTooShort => {
                AccessErrorCode::InvalidRequest
            }
            Q::UnknownIdentifier => AccessErrorCode::UnknownExactId,
            Q::StaleSelection | Q::StaleContinuation | Q::StalePolicy => {
                AccessErrorCode::StaleSelection
            }
            Q::CursorExpired => AccessErrorCode::CursorExpired,
            Q::BudgetExceeded => AccessErrorCode::BudgetExceeded,
            Q::Cancelled => AccessErrorCode::Cancelled,
            Q::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
            Q::IndexIncomplete | Q::SelectionIncomplete => AccessErrorCode::IndexIncomplete,
            Q::CorruptSelectedCarrier | Q::NonMonotoneProgress | Q::MissingProgress => {
                AccessErrorCode::CorruptSelectedCarrier
            }
            Q::PolicyBindingUnavailable
            | Q::UnsupportedProfile
            | Q::UnsupportedModel
            | Q::Unavailable => AccessErrorCode::Unavailable,
        };
        Self::new(code, error.message)
    }
}
impl From<tos_query::CatalogError> for AccessError {
    fn from(error: tos_query::CatalogError) -> Self {
        use tos_query::CatalogErrorCode as Q;
        Self::new(
            match error.code {
                Q::BudgetExceeded => AccessErrorCode::BudgetExceeded,
                Q::StaleSelection => AccessErrorCode::StaleSelection,
                Q::CorruptSelectedCarrier => AccessErrorCode::CorruptSelectedCarrier,
                Q::PolicyBindingUnavailable => AccessErrorCode::Unavailable,
                Q::Unauthorized => AccessErrorCode::PolicyDenied,
                Q::Cancelled => AccessErrorCode::Cancelled,
                Q::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
            },
            error.message,
        )
    }
}

#[derive(Clone, Copy)]
pub struct SelectedKnowledgeBudgets {
    pub catalog: tos_query::CatalogBudget,
    pub inspect: tos_query::InspectBudget,
    pub lens: tos_query::knowledge_lens::LensBudget,
    pub exploration: tos_query::knowledge_exploration::ExplorationBudget,
}
/// Execute against a freshly opened, owner-bound session. The caller owns cold
/// admission and serializes access; no path or policy is discovered by transport.
/// Each authority is wrapped only to carry the caller's cancellation probe.
pub fn execute_selected_knowledge(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    catalog: &mut dyn tos_query::CatalogCurrentAuthority,
    inspect: &mut dyn tos_query::InspectCurrentAuthority,
    checkpoints: &mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
    request: KnowledgeRequest,
    budgets: SelectedKnowledgeBudgets,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket, AccessError> {
    use tos_query::search_v2::SearchKind;
    check_abort(&probe)?;
    let catalog_probe = combined_probe(Arc::clone(&probe), catalog.abort_probe());
    let mut catalog = CatalogProbe {
        inner: catalog,
        probe: catalog_probe,
    };
    let inspect_probe = combined_probe(Arc::clone(&probe), inspect.abort_probe());
    let mut inspect = InspectProbe {
        inner: inspect,
        probe: inspect_probe,
    };
    let packet = match request {
        KnowledgeRequest::Catalog => from_catalog(tos_query::execute_selected_catalog(
            model,
            bound,
            &mut catalog,
            budgets.catalog,
        )?),
        KnowledgeRequest::Node {
            node_id,
            relation_limit,
        } => from_inspect(tos_query::execute_selected_inspect(
            model,
            bound,
            &mut inspect,
            SearchKind::Nodes,
            &node_id,
            relation_limit,
            budgets.inspect,
        )?),
        KnowledgeRequest::Relation { relation_id } => {
            from_inspect(tos_query::execute_selected_inspect(
                model,
                bound,
                &mut inspect,
                SearchKind::Relations,
                &relation_id,
                0,
                budgets.inspect,
            )?)
        }
        KnowledgeRequest::Temporal(request) => from_inspect(tos_query::execute_selected_temporal(
            model,
            bound,
            &mut inspect,
            &request,
            budgets.inspect,
        )?),
        KnowledgeRequest::Lens(spec) => {
            from_inspect(tos_query::knowledge_lens::execute_selected_lens(
                model,
                bound,
                &mut inspect,
                &spec,
                budgets.lens,
            )?)
        }
        KnowledgeRequest::Explore(request) => from_inspect(
            tos_query::knowledge_exploration::execute_selected_exploration(
                model,
                bound,
                &mut inspect,
                checkpoints,
                &request,
                budgets.exploration,
            )?,
        ),
    };
    check_abort(&probe)?;
    Ok(packet)
}
struct CatalogProbe<'a> {
    inner: &'a mut dyn tos_query::CatalogCurrentAuthority,
    probe: Arc<dyn AbortProbe>,
}
impl tos_query::CatalogCurrentAuthority for CatalogProbe<'_> {
    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        Some(Arc::clone(&self.probe))
    }
    fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
        self.inner.policy_binding()
    }
    fn disclosure_scope(&self) -> tos_query::CatalogDisclosureScope {
        self.inner.disclosure_scope()
    }
    fn check_selected(&mut self) -> Result<(), tos_query::CatalogError> {
        self.inner.check_selected()
    }
    fn authorize_current(
        &mut self,
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::CatalogError> {
        self.inner.authorize_current(hash)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &tos_query::CatalogDisclosureScope,
        hash: tos_foundation::Digest256,
    ) -> Result<Box<dyn tos_query::CatalogDisclosureLease>, tos_query::CatalogError> {
        self.inner.acquire_disclosure(scope, hash)
    }
}
struct InspectProbe<'a> {
    inner: &'a mut dyn tos_query::InspectCurrentAuthority,
    probe: Arc<dyn AbortProbe>,
}
impl tos_query::InspectCurrentAuthority for InspectProbe<'_> {
    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        Some(Arc::clone(&self.probe))
    }
    fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
        self.inner.policy_binding()
    }
    fn disclosure_scope(&self) -> tos_query::IndexedDisclosureScope {
        self.inner.disclosure_scope()
    }
    fn check_selected(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner.check_selected()
    }
    fn authorize_current(
        &mut self,
        carrier: &tos_query::InspectedCarrier,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner.authorize_current(carrier)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &tos_query::IndexedDisclosureScope,
        consulted: &[tos_query::ObservedInspectCarrier],
    ) -> Result<Box<dyn tos_query::InspectDisclosureLease>, tos_query::search_v2::SearchV2Error>
    {
        self.inner.acquire_disclosure(scope, consulted)
    }
}

struct CombinedProbe {
    transport: Arc<dyn AbortProbe>,
    owner: Arc<dyn AbortProbe>,
}
impl AbortProbe for CombinedProbe {
    fn reason(&self) -> Option<AbortReason> {
        self.transport.reason().or_else(|| self.owner.reason())
    }
}
fn combined_probe(
    transport: Arc<dyn AbortProbe>,
    owner: Option<Arc<dyn AbortProbe>>,
) -> Arc<dyn AbortProbe> {
    match owner {
        Some(owner) => Arc::new(CombinedProbe { transport, owner }),
        None => transport,
    }
}
