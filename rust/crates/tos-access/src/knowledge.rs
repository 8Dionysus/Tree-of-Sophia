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
    ExplorationContracts,
    Focus,
    StoredLens,
    Contracts,
    SearchCapabilities,
    Dossier,
    PhilosophyNode,
    PhilosophyEdge,
    PhilosophyNeighborhood,
    PhilosophyPath,
    PhilosophyView,
    PhilosophyViews,
    PhilosophyLayers,
    PhilosophyClusters,
    PhilosophyReview,
    PhilosophySnapshot,
    PhilosophyUnresolved,
    CorpusStatus,
    CorpusSummary,
    CorpusSearch,
    CorpusResources,
    CorpusNode,
    CorpusRelationPack,
    CorpusGraphView,
    CorpusPacket,
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
            crate::exploration_contracts::OPERATION => Self::ExplorationContracts,
            "tos.knowledge.focus" => Self::Focus,
            "tos.lens.open" => Self::StoredLens,
            "tos.knowledge.contracts" => Self::Contracts,
            "tos.knowledge.search.capabilities" => Self::SearchCapabilities,
            "tos.dossier.inspect" => Self::Dossier,
            "tos.node.inspect" => Self::PhilosophyNode,
            "tos_philosophy_graph_edge" => Self::PhilosophyEdge,
            "tos.neighborhood" => Self::PhilosophyNeighborhood,
            "tos.path.find" => Self::PhilosophyPath,
            "tos.view.open" => Self::PhilosophyView,
            "tos_philosophy_graph_views" => Self::PhilosophyViews,
            "tos_philosophy_graph_layers" => Self::PhilosophyLayers,
            "tos_philosophy_graph_clusters" => Self::PhilosophyClusters,
            "tos_philosophy_graph_review_packet" => Self::PhilosophyReview,
            "tos.snapshot" => Self::PhilosophySnapshot,
            "tos_philosophy_graph_unresolved" => Self::PhilosophyUnresolved,
            "tos_corpus_status" => Self::CorpusStatus,
            "tos_corpus_summary" => Self::CorpusSummary,
            "tos_corpus_search" => Self::CorpusSearch,
            "tos_corpus_resources" => Self::CorpusResources,
            "tos_corpus_node" => Self::CorpusNode,
            "tos_corpus_relation_pack" => Self::CorpusRelationPack,
            "tos_corpus_graph_view" => Self::CorpusGraphView,
            "tos_corpus_packet" => Self::CorpusPacket,
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
            Self::ExplorationContracts => crate::exploration_contracts::OPERATION,
            Self::Focus => "tos.knowledge.focus",
            Self::StoredLens => "tos.lens.open",
            Self::Contracts => "tos.knowledge.contracts",
            Self::SearchCapabilities => "tos.knowledge.search.capabilities",
            Self::Dossier => "tos.dossier.inspect",
            Self::PhilosophyNode => "tos.node.inspect",
            Self::PhilosophyEdge => "tos_philosophy_graph_edge",
            Self::PhilosophyNeighborhood => "tos.neighborhood",
            Self::PhilosophyPath => "tos.path.find",
            Self::PhilosophyView => "tos.view.open",
            Self::PhilosophyViews => "tos_philosophy_graph_views",
            Self::PhilosophyLayers => "tos_philosophy_graph_layers",
            Self::PhilosophyClusters => "tos_philosophy_graph_clusters",
            Self::PhilosophyReview => "tos_philosophy_graph_review_packet",
            Self::PhilosophySnapshot => "tos.snapshot",
            Self::PhilosophyUnresolved => "tos_philosophy_graph_unresolved",
            Self::CorpusStatus => "tos_corpus_status",
            Self::CorpusSummary => "tos_corpus_summary",
            Self::CorpusSearch => "tos_corpus_search",
            Self::CorpusResources => "tos_corpus_resources",
            Self::CorpusNode => "tos_corpus_node",
            Self::CorpusRelationPack => "tos_corpus_relation_pack",
            Self::CorpusGraphView => "tos_corpus_graph_view",
            Self::CorpusPacket => "tos_corpus_packet",
        }
    }
    pub fn is_corpus(self) -> bool {
        matches!(
            self,
            Self::CorpusStatus
                | Self::CorpusSummary
                | Self::CorpusSearch
                | Self::CorpusResources
                | Self::CorpusNode
                | Self::CorpusRelationPack
                | Self::CorpusGraphView
                | Self::CorpusPacket
        )
    }
    pub fn is_philosophy(self) -> bool {
        matches!(
            self,
            Self::PhilosophyNode
                | Self::PhilosophyEdge
                | Self::PhilosophyNeighborhood
                | Self::PhilosophyPath
                | Self::PhilosophyView
                | Self::PhilosophyViews
                | Self::PhilosophyLayers
                | Self::PhilosophyClusters
                | Self::PhilosophyReview
                | Self::PhilosophySnapshot
                | Self::PhilosophyUnresolved
        )
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
    ExplorationContracts,
    Focus(tos_query::knowledge_focus::KnowledgeFocusRequest),
    StoredLens {
        lens_id: String,
    },
    Contracts,
    SearchCapabilities,
    Dossier {
        object_id: String,
        limit: usize,
    },
    Philosophy(tos_query::philosophy_read::PhilosophyReadRequest),
    Corpus(tos_query::corpus_read::CorpusReadRequest),
    /// Internal boot projections, not separately advertised operations.
    PhilosophyViewIds,
    CorpusViewIds,
}
impl KnowledgeRequest {
    pub fn operation(&self) -> KnowledgeOperation {
        match self {
            Self::PhilosophyViewIds => KnowledgeOperation::PhilosophyViews,
            Self::CorpusViewIds => KnowledgeOperation::CorpusSummary,
            Self::Catalog => KnowledgeOperation::Catalog,
            Self::Node { .. } => KnowledgeOperation::Node,
            Self::Relation { .. } => KnowledgeOperation::Relation,
            Self::Temporal(_) => KnowledgeOperation::Temporal,
            Self::Lens(_) => KnowledgeOperation::Lens,
            Self::Explore(_) => KnowledgeOperation::Explore,
            Self::ExplorationContracts => KnowledgeOperation::ExplorationContracts,
            Self::Focus(_) => KnowledgeOperation::Focus,
            Self::StoredLens { .. } => KnowledgeOperation::StoredLens,
            Self::Contracts => KnowledgeOperation::Contracts,
            Self::SearchCapabilities => KnowledgeOperation::SearchCapabilities,
            Self::Dossier { .. } => KnowledgeOperation::Dossier,
            Self::Corpus(request) => {
                KnowledgeOperation::from_id(request.operation_id()).expect("typed corpus scope")
            }
            Self::Philosophy(request) => {
                use tos_query::philosophy_read::PhilosophyReadRequest as P;
                match request {
                    P::Node { .. } => KnowledgeOperation::PhilosophyNode,
                    P::Edge { .. } => KnowledgeOperation::PhilosophyEdge,
                    P::Neighborhood { .. } => KnowledgeOperation::PhilosophyNeighborhood,
                    P::Path { .. } => KnowledgeOperation::PhilosophyPath,
                    P::View { .. } => KnowledgeOperation::PhilosophyView,
                    P::Views => KnowledgeOperation::PhilosophyViews,
                    P::Layers => KnowledgeOperation::PhilosophyLayers,
                    P::Clusters { .. } => KnowledgeOperation::PhilosophyClusters,
                    P::Review { .. } => KnowledgeOperation::PhilosophyReview,
                    P::Snapshot => KnowledgeOperation::PhilosophySnapshot,
                    P::Unresolved { .. } => KnowledgeOperation::PhilosophyUnresolved,
                }
            }
        }
    }
    pub fn from_arguments(
        operation: KnowledgeOperation,
        args: &JsonValue,
    ) -> Result<Self, AccessError> {
        if operation.is_corpus() {
            return corpus_from_arguments(operation, args).map(Self::Corpus);
        }
        if operation.is_philosophy() {
            return philosophy_from_arguments(operation, args).map(Self::Philosophy);
        }
        let fields = args
            .as_object()
            .ok_or_else(|| invalid("tool arguments must be an object"))?;
        let allowed: &[&str] = match operation {
            KnowledgeOperation::Catalog
            | KnowledgeOperation::Contracts
            | KnowledgeOperation::SearchCapabilities
            | KnowledgeOperation::ExplorationContracts => &[],
            KnowledgeOperation::Focus => &[
                "node_id",
                "sources",
                "depth",
                "direction",
                "predicate_ids",
                "node_limit",
                "relation_limit",
                "profile",
            ],
            KnowledgeOperation::StoredLens => &["lens_id"],
            KnowledgeOperation::Dossier => &["object_id", "limit"],
            KnowledgeOperation::Node => &["node_id", "relation_limit"],
            KnowledgeOperation::Relation => &["relation_id"],
            KnowledgeOperation::Lens => &["spec"],
            KnowledgeOperation::Temporal | KnowledgeOperation::Explore => &["request"],
            _ => unreachable!("philosophy handled above"),
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
            KnowledgeOperation::ExplorationContracts => Self::ExplorationContracts,
            KnowledgeOperation::Contracts => Self::Contracts,
            KnowledgeOperation::SearchCapabilities => Self::SearchCapabilities,
            KnowledgeOperation::Dossier => Self::Dossier {
                object_id: id("object_id")?,
                limit: match args.object_get("limit") {
                    None => 300,
                    Some(value) => value
                        .as_u64()
                        .filter(|n| (1..=300).contains(n))
                        .ok_or_else(|| invalid("dossier limit must be an integer in 1..300"))?
                        as usize,
                },
            },
            KnowledgeOperation::Focus => Self::Focus(focus_from_arguments(args)?),
            KnowledgeOperation::StoredLens => Self::StoredLens {
                lens_id: id("lens_id")?,
            },
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
fn corpus_from_arguments(
    operation: KnowledgeOperation,
    args: &JsonValue,
) -> Result<tos_query::corpus_read::CorpusReadRequest, AccessError> {
    use KnowledgeOperation as O;
    use tos_query::corpus_read::CorpusReadRequest as R;
    let allowed: &[&str] = match operation {
        O::CorpusStatus | O::CorpusSummary => &[],
        O::CorpusSearch => &["query", "limit", "resource_kind"],
        O::CorpusResources => &["resource_kind", "owner_branch", "limit"],
        O::CorpusNode => &["node_id"],
        O::CorpusRelationPack => &["pack_id"],
        O::CorpusGraphView => &["view_id", "limit"],
        O::CorpusPacket => &["query", "view_id", "limit"],
        _ => return Err(invalid("not a corpus operation")),
    };
    let fields = args
        .as_object()
        .ok_or_else(|| invalid("tool arguments must be an object"))?;
    if fields
        .iter()
        .any(|(key, _)| !key.as_str().is_some_and(|key| allowed.contains(&key)))
    {
        return Err(invalid("unknown tool argument"));
    }
    let optional = |key: &str| -> Result<Option<String>, AccessError> {
        match args.object_get(key) {
            None | Some(JsonValue::Null) => Ok(None),
            Some(value) => value
                .as_str()
                .map(|v| Some(v.to_owned()))
                .ok_or_else(|| invalid("corpus argument must be a bounded string")),
        }
    };
    let required = |key: &str| optional(key)?.ok_or_else(|| invalid("corpus argument required"));
    let count = |default: usize, max: u64| -> Result<usize, AccessError> {
        match args.object_get("limit") {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .filter(|v| *v >= 1 && *v <= max)
                .map(|v| v as usize)
                .ok_or_else(|| invalid("corpus limit out of range")),
        }
    };
    Ok(match operation {
        O::CorpusStatus => R::Status,
        O::CorpusSummary => R::Summary,
        O::CorpusSearch => R::Search {
            query: required("query")?,
            limit: count(20, 100)?,
            resource_kind: optional("resource_kind")?,
        },
        O::CorpusResources => R::Resources {
            resource_kind: optional("resource_kind")?,
            owner_branch: optional("owner_branch")?,
            limit: count(100, 1000)?,
        },
        O::CorpusNode => R::Node {
            node_id: required("node_id")?,
        },
        O::CorpusRelationPack => R::RelationPack {
            pack_id: required("pack_id")?,
        },
        O::CorpusGraphView => R::GraphView {
            view_id: required("view_id")?,
            limit: count(100, 1000)?,
        },
        O::CorpusPacket => R::Packet {
            query: optional("query")?.unwrap_or_default(),
            view_id: optional("view_id")?,
            limit: count(20, 100)?,
        },
        _ => unreachable!(),
    })
}
fn philosophy_from_arguments(
    operation: KnowledgeOperation,
    args: &JsonValue,
) -> Result<tos_query::philosophy_read::PhilosophyReadRequest, AccessError> {
    use KnowledgeOperation as O;
    use tos_query::philosophy_read::{PhilosophyDirection as D, PhilosophyReadRequest as R};
    let fields = args
        .as_object()
        .ok_or_else(|| invalid("tool arguments must be an object"))?;
    let allowed: &[&str] = match operation {
        O::PhilosophyNode => &["node_id"],
        O::PhilosophyEdge => &["edge_id"],
        O::PhilosophyNeighborhood => &["node_id", "depth", "limit", "layers", "predicates"],
        O::PhilosophyPath => &[
            "from_id",
            "to_id",
            "layers",
            "predicates",
            "max_depth",
            "direction",
            "view_id",
            "excluded_edge_ids",
            "alternative_limit",
        ],
        O::PhilosophyView => &["view_id", "limit"],
        O::PhilosophyClusters => &["view_id", "cluster_kind", "limit"],
        O::PhilosophyReview | O::PhilosophyUnresolved => &["view_id"],
        O::PhilosophyViews | O::PhilosophyLayers | O::PhilosophySnapshot => &[],
        _ => return Err(invalid("not a philosophy operation")),
    };
    if fields
        .iter()
        .any(|(key, _)| !key.as_str().is_some_and(|key| allowed.contains(&key)))
    {
        return Err(invalid("unknown tool argument"));
    }
    let optional = |key: &str| -> Result<Option<String>, AccessError> {
        match args.object_get(key) {
            None | Some(JsonValue::Null) => Ok(None),
            Some(value) => value
                .as_str()
                .filter(|s| s.chars().count() <= 4096)
                .map(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_owned())
                    }
                })
                .ok_or_else(|| invalid("philosophy identifier must be a bounded string")),
        }
    };
    let id = |key: &str| optional(key)?.ok_or_else(|| invalid("philosophy identifier is required"));
    let strings = |key: &str| -> Result<Vec<String>, AccessError> {
        match args.object_get(key) {
            None | Some(JsonValue::Null) => Ok(vec![]),
            Some(value) => value
                .as_array()
                .ok_or_else(|| invalid("philosophy filters must be arrays"))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|s| s.chars().count() <= 4096)
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("philosophy filter must be a bounded string"))
                })
                .collect(),
        }
    };
    // The maintained methods clamp integer options; this is caller coercion,
    // while traversal/selection remains entirely in QRY.
    let count =
        |key: &str, default: usize, low: usize, high: usize| -> Result<usize, AccessError> {
            match args.object_get(key) {
                None => Ok(default),
                Some(JsonValue::Number(number))
                    if number.kind == tos_foundation::JsonNumberKind::Int =>
                {
                    if number.lexeme.starts_with('-') {
                        Ok(low)
                    } else {
                        Ok(number
                            .lexeme
                            .parse::<usize>()
                            .unwrap_or(usize::MAX)
                            .clamp(low, high))
                    }
                }
                _ => Err(invalid("philosophy limit must be an integer")),
            }
        };
    Ok(match operation {
        O::PhilosophyNode => R::Node {
            node_id: id("node_id")?,
        },
        O::PhilosophyEdge => R::Edge {
            edge_id: id("edge_id")?,
        },
        O::PhilosophyNeighborhood => R::Neighborhood {
            node_id: id("node_id")?,
            depth: count("depth", 1, 1, 3)?,
            limit: count("limit", 80, 1, 300)?,
            layers: strings("layers")?,
            predicates: strings("predicates")?,
        },
        O::PhilosophyPath => R::Path {
            from_id: id("from_id")?,
            to_id: id("to_id")?,
            layers: strings("layers")?,
            predicates: strings("predicates")?,
            max_depth: count("max_depth", 6, 1, 8)?,
            direction: match (match args.object_get("direction") {
                None => "outgoing",
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| invalid("invalid philosophy path direction"))?,
            }) {
                "outgoing" => D::Outgoing,
                "incoming" => D::Incoming,
                "either" => D::Either,
                _ => return Err(invalid("invalid philosophy path direction")),
            },
            view_id: optional("view_id")?,
            excluded_edge_ids: strings("excluded_edge_ids")?,
            alternative_limit: count("alternative_limit", 1, 1, 5)?,
        },
        O::PhilosophyView => R::View {
            view_id: id("view_id")?,
            limit: count("limit", 1000, 1, 1000)?,
        },
        O::PhilosophyViews => R::Views,
        O::PhilosophyLayers => R::Layers,
        O::PhilosophyClusters => R::Clusters {
            view_id: optional("view_id")?,
            cluster_kind: optional("cluster_kind")?,
            limit: count("limit", 80, 1, 1000)?,
        },
        O::PhilosophyReview => R::Review {
            view_id: optional("view_id")?.unwrap_or_else(|| "chronology".into()),
        },
        O::PhilosophySnapshot => R::Snapshot,
        O::PhilosophyUnresolved => R::Unresolved {
            view_id: optional("view_id")?,
        },
        _ => unreachable!("validated philosophy operation"),
    })
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
pub fn from_inspect<'hold>(packet: tos_query::DisclosableInspect<'hold>) -> PreparedPacket<'hold> {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(InspectFence(lease)),
    }
}
pub fn from_catalog<'hold>(packet: tos_query::DisclosableCatalog<'hold>) -> PreparedPacket<'hold> {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(CatalogFence(lease)),
    }
}
pub fn from_indexed_search(packet: tos_query::DisclosableIndexedSearch) -> PreparedPacket<'static> {
    let (body, lease) = packet.into_parts();
    PreparedPacket {
        body,
        fence: Box::new(SearchFence(lease)),
    }
}
struct InspectFence<'hold>(Box<dyn tos_query::InspectDisclosureLease + 'hold>);
impl crate::DisclosureFence for InspectFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}
struct CatalogFence<'hold>(Box<dyn tos_query::CatalogDisclosureLease + 'hold>);
impl crate::DisclosureFence for CatalogFence<'_> {
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
pub fn execute_selected_knowledge<'hold>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    catalog: &mut dyn tos_query::CatalogCurrentAuthority<'hold>,
    inspect: &mut dyn tos_query::InspectCurrentAuthority<'hold>,
    checkpoints: &mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
    request: KnowledgeRequest,
    budgets: SelectedKnowledgeBudgets,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
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
        KnowledgeRequest::ExplorationContracts => {
            return Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "software contracts use the program executor",
            ));
        }
        KnowledgeRequest::CorpusViewIds | KnowledgeRequest::Corpus(_) => {
            return Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "selected corpus source context unavailable",
            ));
        }
        KnowledgeRequest::PhilosophyViewIds => from_inspect(
            tos_query::philosophy_read::execute_selected_philosophy_view_ids(
                model,
                bound,
                &mut inspect,
                tos_query::philosophy_read::PhilosophyReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
            )?,
        ),
        KnowledgeRequest::Philosophy(request) => {
            let packet = tos_query::philosophy_read::execute_selected_philosophy(
                model,
                bound,
                &mut inspect,
                &request,
                tos_query::philosophy_read::PhilosophyReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
            )?;
            from_inspect(packet)
        }
        KnowledgeRequest::SearchCapabilities => from_inspect(
            tos_query::knowledge_legacy_search::execute_selected_search_capabilities(
                model,
                bound,
                &mut inspect,
                budgets.inspect,
            )?,
        ),
        KnowledgeRequest::Dossier { object_id, limit } => {
            from_inspect(tos_query::source_dossier::execute_selected_dossier(
                model,
                bound,
                &mut inspect,
                &object_id,
                limit,
                tos_query::source_dossier::DossierBudget {
                    inspect: budgets.inspect,
                    max_candidates: usize::try_from(budgets.inspect.max_rows).unwrap_or(usize::MAX),
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                    block_size: 128,
                },
            )?)
        }
        KnowledgeRequest::Contracts => {
            return Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "selected registry carriers and contracts authority unavailable",
            ));
        }
        KnowledgeRequest::Focus(request) => {
            from_inspect(tos_query::knowledge_lens::execute_selected_focus(
                model,
                bound,
                &mut inspect,
                &request,
                budgets.lens,
            )?)
        }
        KnowledgeRequest::StoredLens { lens_id } => {
            from_inspect(tos_query::knowledge_lens::execute_selected_stored_lens(
                model,
                bound,
                &mut inspect,
                &lens_id,
                budgets.lens,
            )?)
        }
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
/// Native transport adapter for the maintained v1 engine, with exact owner scope.
/// The session supplies its selected model, authority and explicit scan budgets.
pub fn execute_selected_legacy_search<'hold>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    authority: &mut dyn tos_query::InspectCurrentAuthority<'hold>,
    request: &tos_query::knowledge_legacy_search::LegacySearchRequest,
    budget: tos_query::knowledge_legacy_search::LegacySearchBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
    check_abort(&probe)?;
    let owner_probe = combined_probe(Arc::clone(&probe), authority.abort_probe());
    let mut authority = InspectProbe {
        inner: authority,
        probe: owner_probe,
    };
    let packet = from_inspect(
        tos_query::knowledge_legacy_search::execute_selected_legacy_search(
            model,
            bound,
            &mut authority,
            request,
            budget,
        )?,
    );
    check_abort(&probe)?;
    Ok(packet)
}

/// Execute contracts only when the trusted selected session supplies both exact
/// borrowed registry carriers. Its authority hold must cover both grants.
/// The generic dispatcher remains unavailable without that owner composition.
pub fn execute_selected_knowledge_contracts<'hold>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    authority: &mut dyn tos_query::InspectCurrentAuthority<'hold>,
    registry_bytes: [&[u8]; 2],
    budget: tos_query::knowledge_contracts::KnowledgeContractBudget,
    inspect: tos_query::InspectBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
    check_abort(&probe)?;
    let owner_probe = combined_probe(Arc::clone(&probe), authority.abort_probe());
    let mut authority = InspectProbe {
        inner: authority,
        probe: owner_probe,
    };
    let packet = from_inspect(
        tos_query::knowledge_contracts::execute_selected_knowledge_contracts(
            model,
            bound,
            &mut authority,
            registry_bytes,
            budget,
            inspect,
        )?,
    );
    check_abort(&probe)?;
    Ok(packet)
}

struct CatalogProbe<'a, 'hold> {
    inner: &'a mut dyn tos_query::CatalogCurrentAuthority<'hold>,
    probe: Arc<dyn AbortProbe>,
}
impl<'hold> tos_query::CatalogCurrentAuthority<'hold> for CatalogProbe<'_, 'hold> {
    fn authorize_managed_source_current(
        &mut self,
        proof: &tos_compiler::ManagedSourceProofV1,
    ) -> Result<(), tos_query::CatalogError> {
        self.inner.authorize_managed_source_current(proof)
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        proof: &tos_compiler::ManagedSourceProofV2,
    ) -> Result<(), tos_query::CatalogError> {
        self.inner.authorize_managed_source_v2_current(proof)
    }
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
    ) -> Result<Box<dyn tos_query::CatalogDisclosureLease + 'hold>, tos_query::CatalogError> {
        self.inner.acquire_disclosure(scope, hash)
    }
}
struct InspectProbe<'a, 'hold> {
    inner: &'a mut dyn tos_query::InspectCurrentAuthority<'hold>,
    probe: Arc<dyn AbortProbe>,
}
impl<'hold> tos_query::InspectCurrentAuthority<'hold> for InspectProbe<'_, 'hold> {
    fn authorize_managed_source_current(
        &mut self,
        proof: &tos_compiler::ManagedSourceProofV1,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner.authorize_managed_source_current(proof)
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        proof: &tos_compiler::ManagedSourceProofV2,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner.authorize_managed_source_v2_current(proof)
    }
    fn authorize_corpus_view_identity_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        ordinal: u64,
        view_id: Option<&str>,
        sha: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner
            .authorize_corpus_view_identity_current(receipt, ordinal, view_id, sha)
    }
    fn authorize_corpus_original_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        collection: tos_compiler::CorpusOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner
            .authorize_corpus_original_current(receipt, collection, ordinal, raw, hash)
    }

    fn authorize_philosophy_original_current(
        &mut self,
        receipt: &tos_compiler::PhilosophyOriginalReceipt,
        collection: tos_compiler::PhilosophyOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner
            .authorize_philosophy_original_current(receipt, collection, ordinal, raw, hash)
    }
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
    fn authorize_navigation_original_current(
        &mut self,
        receipt: &tos_compiler::NavigationOriginalReceipt,
        ordinal: i64,
        raw: &[u8],
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner
            .authorize_navigation_original_current(receipt, ordinal, raw, hash)
    }
    fn authorize_registry_current(
        &mut self,
        registry_id: &str,
        raw: &[u8],
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner
            .authorize_registry_current(registry_id, raw, hash)
    }
    fn authorize_catalog_current(
        &mut self,
        hash: tos_foundation::Digest256,
    ) -> Result<(), tos_query::search_v2::SearchV2Error> {
        self.inner.authorize_catalog_current(hash)
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
    ) -> Result<
        Box<dyn tos_query::InspectDisclosureLease + 'hold>,
        tos_query::search_v2::SearchV2Error,
    > {
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

pub(crate) fn focus_from_arguments(
    args: &JsonValue,
) -> Result<tos_query::knowledge_focus::KnowledgeFocusRequest, AccessError> {
    use tos_query::knowledge_focus::{FocusDirection, FocusProfile, KnowledgeFocusRequest};
    let id = args
        .object_get("node_id")
        .and_then(JsonValue::as_str)
        .filter(|id| !id.is_empty() && id.chars().count() <= 4096)
        .ok_or_else(|| invalid("focus node_id must be a nonempty bounded string"))?;
    let mut request = KnowledgeFocusRequest::new(id);
    let strings = |key: &str| -> Result<Option<Vec<String>>, AccessError> {
        match args.object_get(key) {
            None | Some(JsonValue::Null) => Ok(None),
            Some(value) => {
                let values = value
                    .as_array()
                    .filter(|values| values.len() <= 256)
                    .ok_or_else(|| invalid("focus filter must be a bounded array"))?;
                Ok(Some(
                    values
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .filter(|value| value.chars().count() <= 1024)
                                .map(str::to_owned)
                                .ok_or_else(|| invalid("focus filter must contain bounded strings"))
                        })
                        .collect::<Result<_, _>>()?,
                ))
            }
        }
    };
    let count = |key: &str, default: usize, min: u64, max: u64| -> Result<usize, AccessError> {
        match args.object_get(key) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .filter(|n| *n >= min && *n <= max)
                .map(|n| n as usize)
                .ok_or_else(|| invalid("focus count out of range")),
        }
    };
    request.sources = strings("sources")?;
    request.predicate_ids = strings("predicate_ids")?.unwrap_or_default();
    request.depth = count("depth", 1, 0, 5)?;
    request.node_limit = count("node_limit", 200, 1, 1000)?;
    request.relation_limit = count("relation_limit", 400, 0, 2000)?;
    request.direction = match args.object_get("direction") {
        None => FocusDirection::Either,
        Some(value) => match value.as_str() {
            Some("outgoing") => FocusDirection::Outgoing,
            Some("incoming") => FocusDirection::Incoming,
            Some("either") => FocusDirection::Either,
            _ => return Err(invalid("invalid focus direction")),
        },
    };
    request.profile = match args.object_get("profile") {
        None => FocusProfile::Overview,
        Some(value) => match value.as_str() {
            Some("all") => FocusProfile::All,
            Some("overview") => FocusProfile::Overview,
            _ => return Err(invalid("invalid focus profile")),
        },
    };
    Ok(request)
}

/// Corpus context is supplied only by the declared original-member holder.
pub fn execute_selected_corpus<'hold>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    authority: &mut dyn tos_query::InspectCurrentAuthority<'hold>,
    context: &tos_query::corpus_read::CorpusReadContext,
    request: &tos_query::corpus_read::CorpusReadRequest,
    budget: tos_query::corpus_read::CorpusReadBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
    check_abort(&probe)?;
    let probe = combined_probe(probe, authority.abort_probe());
    let mut authority = InspectProbe {
        inner: authority,
        probe: Arc::clone(&probe),
    };
    let packet = tos_query::corpus_read::execute_selected_corpus(
        model,
        bound,
        &mut authority,
        context,
        request,
        budget,
    )?;
    check_abort(&probe)?;
    Ok(from_inspect(packet))
}

/// Internal site metadata uses the existing Summary scope and current hold.
pub fn execute_selected_corpus_view_ids<'hold>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    authority: &mut dyn tos_query::InspectCurrentAuthority<'hold>,
    budget: tos_query::corpus_read::CorpusReadBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
    check_abort(&probe)?;
    let probe = combined_probe(probe, authority.abort_probe());
    let mut authority = InspectProbe {
        inner: authority,
        probe: Arc::clone(&probe),
    };
    let packet = tos_query::corpus_read::execute_selected_corpus_view_ids(
        model,
        bound,
        &mut authority,
        budget,
    )?;
    check_abort(&probe)?;
    Ok(from_inspect(packet))
}
