//! Maintained corpus packets from selected ordered originals. Addressed reads
//! use the producer's verified scalar indexes; no normalized-row reconstruction.
use crate::knowledge_inspect::{
    InspectVisitMeter, Reader, execute_selected_carrier_packet,
    execute_selected_carrier_packet_with_optional_meter,
    execute_selected_carrier_packet_with_state,
};
use crate::knowledge_lens_spec::{py_string, truthy};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::source_read_projection::{object, text};
use crate::{BoundCmpKnowledge, DisclosableInspect, InspectBudget, InspectCurrentAuthority};
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{
    CorpusOriginalCollection as Collection, CorpusOriginalReceipt,
    CorpusOriginalSelector as Selector, VerifiedKnowledgeModel,
};
use tos_foundation::{
    JsonNumber, JsonNumberKind, JsonString, JsonValue, python_lower_unicode16_v1,
    python_strip_unicode16_v1,
};

pub const CORPUS_INTENDED_USE: &str = "read_only_public_corpus_projection_v1";
pub const CORPUS_CARRIER_LAYER: &str = crate::knowledge_packet::INDEXED_SEARCH_CARRIER_LAYER;

/// Paths describe the actually selected release member, supplied by its held
/// owner. A retained SQLite component alone does not assert a filesystem index.
#[derive(Clone, Debug)]
pub struct CorpusReadContext {
    pub tos_root: String,
    pub index_path: String,
}
#[derive(Clone, Copy, Debug)]
pub struct CorpusReadBudget {
    pub inspect: InspectBudget,
    pub max_work_steps: u64,
}
#[derive(Clone, Debug)]
pub enum CorpusReadRequest {
    Status,
    Summary,
    GraphViews,
    Search {
        query: String,
        limit: usize,
        resource_kind: Option<String>,
    },
    Resources {
        resource_kind: Option<String>,
        owner_branch: Option<String>,
        limit: usize,
    },
    Node {
        node_id: String,
    },
    RelationPack {
        pack_id: String,
    },
    GraphView {
        view_id: String,
        limit: usize,
    },
    Packet {
        query: String,
        view_id: Option<String>,
        limit: usize,
    },
}
impl CorpusReadRequest {
    pub fn operation_id(&self) -> &'static str {
        match self {
            Self::Status => "tos_corpus_status",
            Self::Summary => "tos_corpus_summary",
            Self::GraphViews => "tos_corpus_graph_views",
            Self::Search { .. } => "tos_corpus_search",
            Self::Resources { .. } => "tos_corpus_resources",
            Self::Node { .. } => "tos_corpus_node",
            Self::RelationPack { .. } => "tos_corpus_relation_pack",
            Self::GraphView { .. } => "tos_corpus_graph_view",
            Self::Packet { .. } => "tos_corpus_packet",
        }
    }
    fn validate(
        &self,
        context: &CorpusReadContext,
        budget: CorpusReadBudget,
    ) -> Result<(), SearchV2Error> {
        let string = |s: &str| s.len() <= budget.inspect.max_field_bytes;
        let optional = |s: &Option<String>| s.as_deref().is_none_or(string);
        let valid = match self {
            Self::Status | Self::Summary | Self::GraphViews => true,
            Self::Search {
                query,
                limit,
                resource_kind,
            } => string(query) && optional(resource_kind) && (1..=100).contains(limit),
            Self::Resources {
                resource_kind,
                owner_branch,
                limit,
            } => optional(resource_kind) && optional(owner_branch) && (1..=1000).contains(limit),
            Self::Node { node_id } => string(node_id),
            Self::RelationPack { pack_id } => string(pack_id),
            Self::GraphView { view_id, limit } => string(view_id) && (1..=1000).contains(limit),
            Self::Packet {
                query,
                view_id,
                limit,
            } => string(query) && optional(view_id) && (1..=100).contains(limit),
        };
        if !valid || !string(&context.tos_root) || !string(&context.index_path) {
            return Err(fail(
                SearchV2ErrorCode::InvalidRequest,
                "invalid corpus read request",
            ));
        }
        if budget.max_work_steps == 0 {
            return Err(budget_error());
        }
        Ok(())
    }
}
fn fail(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error::new(code, message)
}
fn budget_error() -> SearchV2Error {
    fail(
        SearchV2ErrorCode::BudgetExceeded,
        "corpus read budget exceeded",
    )
}
fn field<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap_or(&JsonValue::Null)
}
fn default(v: &JsonValue, key: &str, fallback: JsonValue) -> JsonValue {
    v.object_get(key).cloned().unwrap_or(fallback)
}
fn count(n: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn optional(v: &Option<String>) -> JsonValue {
    v.as_deref().map(text).unwrap_or(JsonValue::Null)
}
fn array(v: Vec<JsonValue>) -> JsonValue {
    JsonValue::Array(v)
}
fn supported(v: &str) -> bool {
    matches!(v, "corpus-topology" | "route-graph" | "promotion-flow")
}
fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(fields) = v else {
        return;
    };
    if let Some((_, current)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *current = value;
    } else {
        fields.push((JsonString::from_utf8(key), value));
    }
}
fn string_set(rows: &[JsonValue], key: &str) -> BTreeSet<String> {
    rows.iter()
        .filter_map(|r| field(r, key).as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn source_refs(rows: &[JsonValue]) -> Vec<JsonValue> {
    let mut refs = string_set(rows, "source_ref");
    for row in rows {
        if let Some(values) = field(row, "source_refs").as_array() {
            refs.extend(
                values
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    refs.into_iter().map(|s| text(&s)).collect()
}

pub(crate) fn controlled_status(header: &JsonValue, context: &CorpusReadContext,
    views: &[JsonValue]) -> JsonValue {
        object(vec![
            ("schema", text("tos_corpus_mcp_status_v1")),
            ("index_exists", JsonValue::Bool(true)),
            ("tos_root", text(&context.tos_root)),
            ("index_path", text(&context.index_path)),
            ("owner_repo", field(header, "owner_repo").clone()),
            ("surface_kind", field(header, "surface_kind").clone()),
            ("counts", default(header, "counts", object(vec![]))),
            (
                "graph_views",
                array(views.iter().map(|v| field(v, "view_id").clone()).collect()),
            ),
            (
                "authority_order",
                default(header, "authority_order", array(vec![])),
            ),
            (
                "runtime_projection_boundary",
                default(header, "runtime_projection_boundary", object(vec![])),
            ),
        ])
}

/// The three metadata packets reuse the maintained field projection. Callers
/// supply only Original rows authorized by their retained native owner.
pub(crate) fn controlled_metadata(header: &JsonValue, context: &CorpusReadContext,
    views: Vec<JsonValue>, branches: Vec<JsonValue>, request: &CorpusReadRequest)
    -> Result<JsonValue, SearchV2Error> {
    match request {
        CorpusReadRequest::Status => Ok(controlled_status(header, context, &views)),
        CorpusReadRequest::GraphViews => Ok(object(vec![
            ("schema", text("tos_corpus_mcp_graph_views_v1")), ("graph_views", array(views))])),
        CorpusReadRequest::Summary => Ok(object(vec![
            ("schema", text("tos_corpus_mcp_summary_v1")),
            ("status", controlled_status(header, context, &views)),
            ("counts", default(header, "counts", object(vec![]))),
            ("branches", array(branches)), ("graph_views", array(views)),
            ("runtime_projection_boundary", default(header, "runtime_projection_boundary", object(vec![]))),
            ("authority_order", default(header, "authority_order", array(vec![]))),
        ])),
        _ => Err(fail(SearchV2ErrorCode::InvalidRequest, "controlled corpus metadata operation unavailable")),
    }
}

/// The maintained Corpus kernel requires only authenticated Original rows and
/// interruption. Existing selected Reader and controlled Original owner each
/// implement these two operations; neither kernel can open a model or mint a cut.
pub(crate) trait CorpusOriginalRead {
    fn check_interrupt(&mut self) -> Result<(), SearchV2Error>;
    fn corpus_row(&mut self, receipt: &CorpusOriginalReceipt, collection: Collection,
        selector: &Selector, after: Option<u64>) -> Result<Option<(u64, JsonValue)>, SearchV2Error>;
    fn charge_kernel_work(&mut self, _steps: usize) -> Result<(), SearchV2Error> {
        self.check_interrupt()
    }
}
impl<'hold, A: InspectCurrentAuthority<'hold> + ?Sized> CorpusOriginalRead for Reader<'_, '_, A> {
    fn check_interrupt(&mut self) -> Result<(), SearchV2Error> { Reader::check_interrupt(self) }
    fn corpus_row(&mut self, receipt: &CorpusOriginalReceipt, collection: Collection,
        selector: &Selector, after: Option<u64>) -> Result<Option<(u64, JsonValue)>, SearchV2Error> {
        Reader::corpus_row(self, receipt, collection, selector, after)
    }
}

pub(crate) fn compute_controlled_corpus<R: CorpusOriginalRead + ?Sized>(
    read: &mut R, receipt: CorpusOriginalReceipt, header: JsonValue,
    context: &CorpusReadContext, request: &CorpusReadRequest, budget: CorpusReadBudget,
) -> Result<JsonValue, SearchV2Error> {
    request.validate(context, budget)?;
    let mut corpus = CorpusRead { read, receipt, header, context,
        remaining: budget.max_work_steps };
    corpus.packet(request)
}

struct CorpusRead<'a, R: CorpusOriginalRead + ?Sized> {
    read: &'a mut R,
    receipt: CorpusOriginalReceipt,
    header: JsonValue,
    context: &'a CorpusReadContext,
    remaining: u64,
}
impl<R: CorpusOriginalRead + ?Sized> CorpusRead<'_, R> {
    fn work(&mut self, steps: usize) -> Result<(), SearchV2Error> {
        self.remaining = self
            .remaining
            .checked_sub(u64::try_from(steps).map_err(|_| budget_error())?)
            .ok_or_else(budget_error)?;
        self.read.charge_kernel_work(steps)
    }
    fn rows(
        &mut self,
        collection: Collection,
        selector: Selector,
        limit: Option<usize>,
    ) -> Result<Vec<JsonValue>, SearchV2Error> {
        let mut rows = Vec::new();
        let mut after = None;
        while limit.is_none_or(|limit| rows.len() < limit) {
            let Some((ordinal, value)) =
                self.read
                    .corpus_row(&self.receipt, collection, &selector, after)?
            else {
                break;
            };
            if after.is_some_and(|previous| ordinal <= previous) {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "corpus original page order differs",
                ));
            }
            self.work(1)?;
            rows.push(value);
            after = Some(ordinal);
        }
        Ok(rows)
    }
    fn contains(&mut self, value: &JsonValue, needle: &str) -> Result<bool, SearchV2Error> {
        self.work(1)?;
        match value {
            JsonValue::String(s) => {
                let s = s.as_str().ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "corpus string is not UTF-8",
                    )
                })?;
                if s.is_empty() {
                    return Ok(needle.is_empty());
                }
                self.work(s.chars().count())?;
                let lower = python_lower_unicode16_v1(
                    s,
                    s.chars().count(),
                    s.chars().count().saturating_mul(3),
                    s.len().saturating_mul(3),
                )
                .map_err(|_| budget_error())?;
                Ok(lower.contains(needle))
            }
            JsonValue::Array(values) => {
                for v in values {
                    if self.contains(v, needle)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            JsonValue::Object(values) => {
                for (_, v) in values {
                    if self.contains(v, needle)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
    fn views(&mut self) -> Result<Vec<JsonValue>, SearchV2Error> {
        Ok(self
            .rows(Collection::GraphViews, Selector::All, None)?
            .into_iter()
            .filter(|v| field(v, "view_id").as_str().is_some_and(supported))
            .collect())
    }
    fn status(&self, views: &[JsonValue]) -> JsonValue {
        controlled_status(&self.header, self.context, views)
    }
    fn search(
        &mut self,
        query: &str,
        resource_kind: &Option<String>,
        limit: usize,
    ) -> Result<JsonValue, SearchV2Error> {
        self.work(query.chars().count())?;
        let lower = if query.is_empty() {
            String::new()
        } else {
            python_lower_unicode16_v1(
                query,
                query.chars().count(),
                query.chars().count().saturating_mul(3),
                query.len().saturating_mul(3),
            )
            .map_err(|_| budget_error())?
        };
        let needle = if lower.is_empty() {
            String::new()
        } else {
            python_strip_unicode16_v1(&lower, lower.chars().count())
                .map_err(|_| budget_error())?
                .to_owned()
        };
        let mut results = Vec::new();
        for (collection, name) in [
            (Collection::Nodes, "nodes"),
            (Collection::Resources, "resources"),
            (Collection::Manifests, "manifests"),
            (Collection::Branches, "branches"),
            (Collection::GraphViews, "graph_views"),
        ] {
            let mut after = None;
            while results.len() < limit {
                let Some((ordinal, value)) =
                    self.read
                        .corpus_row(&self.receipt, collection, &Selector::All, after)?
                else {
                    break;
                };
                if after.is_some_and(|previous| ordinal <= previous) {
                    return Err(fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "corpus search order differs",
                    ));
                }
                after = Some(ordinal);
                self.work(1)?;
                if value.as_object().is_none()
                    || resource_kind.as_deref().is_some_and(|s| {
                        !s.is_empty() && field(&value, "resource_kind").as_str() != Some(s)
                    })
                {
                    continue;
                }
                if !needle.is_empty() && !self.contains(&value, &needle)? {
                    continue;
                }
                results.push(object(vec![("collection", text(name)), ("item", value)]));
            }
            if results.len() == limit {
                break;
            }
        }
        Ok(object(vec![
            ("schema", text("tos_corpus_mcp_search_v1")),
            ("query", text(query)),
            ("resource_kind", optional(resource_kind)),
            ("result_count", count(results.len())),
            ("results", array(results)),
            (
                "authority_note",
                text(
                    "Tree-of-Sophia owns corpus meaning; this MCP packet is an abyss-stack access-plane view.",
                ),
            ),
        ]))
    }
    fn enrich_edges(&mut self, edges: &mut [JsonValue]) -> Result<(), SearchV2Error> {
        let ids: Vec<String> = edges
            .iter()
            .map(|edge| field(edge, "pack_id").as_str().unwrap_or("").to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let packs = self.rows(Collection::RelationPacks, Selector::PackIds(ids), None)?;
        let mut paths = BTreeMap::new();
        for pack in packs {
            if let (Some(id), Some(path)) = (
                field(&pack, "pack_id").as_str(),
                field(&pack, "path").as_str(),
            ) {
                if !path.is_empty() {
                    paths.insert(id.to_owned(), path.to_owned());
                }
            }
        }
        for edge in edges {
            self.work(1)?;
            if !truthy(field(edge, "source_ref")) {
                if let Some(path) = paths.get(field(edge, "pack_id").as_str().unwrap_or("")) {
                    set(edge, "source_ref", text(path));
                }
            }
        }
        Ok(())
    }
    fn node(&mut self, id: &str) -> Result<JsonValue, SearchV2Error> {
        let mut matches = self.rows(Collection::Nodes, Selector::NodeId(id.to_owned()), None)?;
        let mut edges = self.rows(
            Collection::RelationEdges,
            Selector::IncidentNode(id.to_owned()),
            None,
        )?;
        self.enrich_edges(&mut edges)?;
        if matches.is_empty() && !edges.is_empty() {
            matches.push(object(vec![
                ("node_id", text(id)),
                ("label", text(id)),
                ("node_type", text("relation-endpoint")),
                (
                    "owner_branches",
                    array(
                        string_set(&edges, "owner_branch")
                            .into_iter()
                            .map(|s| text(&s))
                            .collect(),
                    ),
                ),
                ("source_refs", array(source_refs(&edges))),
                (
                    "projection_posture",
                    text("identity materialized from indexed relation endpoints"),
                ),
            ]));
        }
        if matches.is_empty() {
            return Err(fail(
                SearchV2ErrorCode::UnknownIdentifier,
                "unknown ToS corpus node",
            ));
        }
        Ok(object(vec![
            ("schema", text("tos_corpus_mcp_node_v1")),
            ("node_id", text(id)),
            ("matches", array(matches)),
            ("related_edges", array(edges)),
            (
                "authority_note",
                text("Node authority stays in the source_path named by the index."),
            ),
        ]))
    }
    fn relation_pack(&mut self, id: &str) -> Result<JsonValue, SearchV2Error> {
        let packs = self.rows(
            Collection::RelationPacks,
            Selector::PackId(id.to_owned()),
            None,
        )?;
        if packs.is_empty() {
            return Err(fail(
                SearchV2ErrorCode::UnknownIdentifier,
                "unknown ToS corpus relation pack",
            ));
        }
        let mut edges = self.rows(
            Collection::RelationEdges,
            Selector::PackId(id.to_owned()),
            None,
        )?;
        self.enrich_edges(&mut edges)?;
        Ok(object(vec![
            ("schema", text("tos_corpus_mcp_relation_pack_v1")),
            ("pack_id", text(id)),
            ("packs", array(packs)),
            ("edges", array(edges)),
            (
                "authority_note",
                text("Relation-pack authority stays in the ToS path named by the pack."),
            ),
        ]))
    }
    fn graph_view(&mut self, id: &str, limit: usize) -> Result<JsonValue, SearchV2Error> {
        let view = self
            .rows(
                Collection::GraphViews,
                Selector::ViewId(id.to_owned()),
                Some(1),
            )?
            .into_iter()
            .next()
            .ok_or_else(|| {
                fail(
                    SearchV2ErrorCode::UnknownIdentifier,
                    "unknown ToS graph view",
                )
            })?;
        if !supported(id) {
            return Err(fail(
                SearchV2ErrorCode::UnknownIdentifier,
                "unsupported standalone ToS graph view",
            ));
        }
        let (items, nodes, edges) = if id == "corpus-topology" {
            let items = self.rows(Collection::Branches, Selector::All, Some(limit))?;
            let root_id = format!("view:{id}");
            let title = if truthy(field(&view, "title")) {
                field(&view, "title").clone()
            } else {
                text(id)
            };
            let mut nodes = vec![object(vec![
                ("node_id", text(&root_id)),
                ("label", title),
                ("node_type", text("corpus-root")),
                ("source_ref", field(&view, "entry_surface").clone()),
            ])];
            let mut edges = Vec::new();
            for branch in &items {
                self.work(1)?;
                if branch.as_object().is_none() || !truthy(field(branch, "id")) {
                    continue;
                }
                let branch_id = py_string(field(branch, "id"));
                let source_ref = if truthy(field(branch, "owner_surface")) {
                    field(branch, "owner_surface")
                } else {
                    field(branch, "path")
                }
                .clone();
                let mut node = branch.clone();
                for (key, value) in [
                    ("node_id", text(&branch_id)),
                    ("label", text(&branch_id)),
                    ("node_type", text("corpus-branch")),
                    ("source_ref", source_ref.clone()),
                ] {
                    set(&mut node, key, value);
                }
                nodes.push(node);
                edges.push(object(vec![
                    (
                        "edge_id",
                        text(&format!("corpus-edge:{root_id}:{branch_id}")),
                    ),
                    ("from_id", text(&root_id)),
                    ("to_id", text(&branch_id)),
                    ("predicate_id", text("contains")),
                    ("source_ref", source_ref),
                ]));
            }
            (items, nodes, edges)
        } else {
            let selector = if id == "route-graph" {
                let packs = self.rows(
                    Collection::RelationPacks,
                    Selector::OwnerBranch("ToS/canon".to_owned()),
                    None,
                )?;
                Selector::PackIds(string_set(&packs, "pack_id").into_iter().collect())
            } else {
                Selector::OwnerBranch("ToS/candidate-intake".to_owned())
            };
            let mut edges = self.rows(Collection::RelationEdges, selector, Some(limit))?;
            self.enrich_edges(&mut edges)?;
            let ids: Vec<String> = string_set(&edges, "from_id")
                .union(&string_set(&edges, "to_id"))
                .cloned()
                .collect();
            let selected_nodes = if ids.is_empty() {
                vec![]
            } else {
                self.rows(Collection::Nodes, Selector::NodeIds(ids.clone()), None)?
            };
            let nodes = if id == "route-graph" {
                selected_nodes
            } else {
                let mut indexed = BTreeMap::new();
                for node in selected_nodes {
                    if let Some(id) = field(&node, "node_id").as_str() {
                        indexed.insert(id.to_owned(), node);
                    }
                }
                let mut refs: BTreeMap<String, BTreeSet<String>> =
                    ids.iter().map(|id| (id.clone(), BTreeSet::new())).collect();
                for edge in &edges {
                    if let Some(source) =
                        field(edge, "source_ref").as_str().filter(|s| !s.is_empty())
                    {
                        for key in ["from_id", "to_id"] {
                            if let Some(set) =
                                field(edge, key).as_str().and_then(|id| refs.get_mut(id))
                            {
                                set.insert(source.to_owned());
                            }
                        }
                    }
                }
                ids.into_iter()
                    .map(|id| {
                        indexed.remove(&id).unwrap_or_else(|| {
                            object(vec![
                                ("node_id", text(&id)),
                                ("label", text(&id)),
                                ("node_type", text("candidate-endpoint")),
                                ("authority_layer", text("candidate_intake")),
                                ("owner_branch", text("ToS/candidate-intake")),
                                (
                                    "source_refs",
                                    array(
                                        refs.remove(&id)
                                            .unwrap_or_default()
                                            .into_iter()
                                            .map(|s| text(&s))
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                    })
                    .collect()
            };
            (edges.clone(), nodes, edges)
        };
        Ok(object(vec![
            ("schema", text("tos_corpus_mcp_graph_view_v1")),
            ("view", view),
            ("item_count", count(items.len())),
            ("items", array(items)),
            ("node_count", count(nodes.len())),
            ("edge_count", count(edges.len())),
            ("nodes", array(nodes)),
            ("edges", array(edges)),
            ("counts", default(&self.header, "counts", object(vec![]))),
            (
                "runtime_projection_boundary",
                default(&self.header, "runtime_projection_boundary", object(vec![])),
            ),
        ]))
    }
    fn packet(&mut self, request: &CorpusReadRequest) -> Result<JsonValue, SearchV2Error> {
        match request {
            CorpusReadRequest::Status | CorpusReadRequest::GraphViews | CorpusReadRequest::Summary => {
                let views = self.views()?;
                let branches = if matches!(request, CorpusReadRequest::Summary) {
                    self.rows(Collection::Branches, Selector::All, None)?
                } else { Vec::new() };
                controlled_metadata(&self.header, self.context, views, branches, request)
            }
            CorpusReadRequest::Search {
                query,
                limit,
                resource_kind,
            } => self.search(query, resource_kind, *limit),
            CorpusReadRequest::Resources {
                resource_kind,
                owner_branch,
                limit,
            } => {
                let rows = self
                    .rows(
                        Collection::Resources,
                        Selector::Resources {
                            resource_kind: resource_kind.clone().filter(|s| !s.is_empty()),
                            owner_branch: owner_branch.clone().filter(|s| !s.is_empty()),
                        },
                        Some(*limit),
                    )?
                    .into_iter()
                    .filter(|row| row.as_object().is_some())
                    .collect::<Vec<_>>();
                Ok(object(vec![
                    ("schema", text("tos_corpus_mcp_resources_v1")),
                    ("resource_kind", optional(resource_kind)),
                    ("owner_branch", optional(owner_branch)),
                    ("count", count(rows.len())),
                    ("resources", array(rows)),
                    (
                        "authority_order",
                        default(&self.header, "authority_order", array(vec![])),
                    ),
                ]))
            }
            CorpusReadRequest::Node { node_id } => self.node(node_id),
            CorpusReadRequest::RelationPack { pack_id } => self.relation_pack(pack_id),
            CorpusReadRequest::GraphView { view_id, limit } => self.graph_view(view_id, *limit),
            CorpusReadRequest::Packet {
                query,
                view_id,
                limit,
            } => {
                let search = if query.is_empty() {
                    object(vec![("result_count", count(0)), ("results", array(vec![]))])
                } else {
                    self.search(query, &None, *limit)?
                };
                let view = match view_id.as_deref().filter(|s| !s.is_empty()) {
                    Some(id) => self.graph_view(id, *limit)?,
                    None => JsonValue::Null,
                };
                Ok(object(vec![
                    ("schema", text("tos_corpus_mcp_packet_v1")),
                    ("query", text(query)),
                    ("view_id", optional(view_id)),
                    ("result_count", field(&search, "result_count").clone()),
                    ("results", field(&search, "results").clone()),
                    ("view", view),
                    ("counts", default(&self.header, "counts", object(vec![]))),
                    (
                        "authority_order",
                        default(&self.header, "authority_order", array(vec![])),
                    ),
                    (
                        "runtime_projection_boundary",
                        default(&self.header, "runtime_projection_boundary", object(vec![])),
                    ),
                ]))
            }
        }
    }
}

/// The caller supplies the actual same-release source-member context and
/// original-row grants. One existing disclosure hold covers the complete body.
pub fn execute_selected_corpus<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    request: &CorpusReadRequest,
    budget: CorpusReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    execute_selected_corpus_with_meter(model, bound, authority, context, request, budget, None)
}

pub fn execute_selected_corpus_metered<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    request: &CorpusReadRequest,
    budget: CorpusReadBudget,
    meter: &mut InspectVisitMeter,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    execute_selected_corpus_with_meter(
        model,
        bound,
        authority,
        context,
        request,
        budget,
        Some(meter),
    )
}

fn execute_selected_corpus_with_meter<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    request: &CorpusReadRequest,
    budget: CorpusReadBudget,
    mut meter: Option<&mut InspectVisitMeter>,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    request.validate(context, budget)?;
    execute_selected_carrier_packet_with_optional_meter(
        model,
        bound,
        authority,
        request.operation_id(),
        CORPUS_INTENDED_USE,
        budget.inspect,
        meter.take(),
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let header = read
                .corpus_row(&receipt, Collection::Header, &Selector::All, None)?
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus header absent",
                    )
                })?;
            if header.0 != 0 || header.1.as_object().is_none() {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus header invalid",
                ));
            }
            let mut corpus = CorpusRead {
                read,
                receipt,
                header: header.1,
                context,
                remaining: budget.max_work_steps,
            };
            corpus.packet(request)
        },
    )
}

/// Minimal health seed under the existing corpus-status scope. It exposes only
/// the source index schema and the first supported graph-view identity; the
/// original header and graph-view rows remain private to this reader.
pub fn execute_selected_corpus_health_seed<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    budget: CorpusReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    execute_selected_corpus_health_seed_with_meter(model, bound, authority, context, budget, None)
}

pub fn execute_selected_corpus_health_seed_metered<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    budget: CorpusReadBudget,
    meter: &mut InspectVisitMeter,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    execute_selected_corpus_health_seed_with_meter(
        model,
        bound,
        authority,
        context,
        budget,
        Some(meter),
    )
}

fn execute_selected_corpus_health_seed_with_meter<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    budget: CorpusReadBudget,
    mut meter: Option<&mut InspectVisitMeter>,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if budget.max_work_steps == 0 {
        return Err(budget_error());
    }
    execute_selected_carrier_packet_with_optional_meter(
        model,
        bound,
        authority,
        CorpusReadRequest::Status.operation_id(),
        CORPUS_INTENDED_USE,
        budget.inspect,
        meter.take(),
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let header = read
                .corpus_row(&receipt, Collection::Header, &Selector::All, None)?
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus header absent",
                    )
                })?;
            if header.0 != 0 || header.1.as_object().is_none() {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus header invalid",
                ));
            }
            let mut corpus = CorpusRead {
                read,
                receipt,
                header: header.1,
                context,
                remaining: budget.max_work_steps,
            };
            let views = corpus.views()?;
            controlled_health_seed(&corpus.header, &views, budget.inspect.max_field_bytes)
        },
    )
}

pub(crate) fn controlled_health_seed(header: &JsonValue, views: &[JsonValue],
    max_field_bytes: usize) -> Result<JsonValue, SearchV2Error> {
            let index_schema = field(header, "schema_version")
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus index schema absent",
                    )
                })?
                .to_owned();
            if index_schema.len() > max_field_bytes {
                return Err(budget_error());
            }
            let first_view = views.iter().filter(|v| field(v, "view_id").as_str().is_some_and(supported))
                .find_map(|view| field(view, "view_id").as_str().map(str::to_owned));
            let seed = object(vec![
                ("schema_version", text("tos_selected_corpus_health_seed_v1")),
                ("index_schema_version", text(&index_schema)),
                (
                    "first_graph_view_id",
                    first_view.as_deref().map(text).unwrap_or(JsonValue::Null),
                ),
            ]);
            Ok(seed)
}

fn bound_original_receipt<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    bound: &BoundCmpKnowledge<'_>,
) -> Result<CorpusOriginalReceipt, SearchV2Error> {
    let receipt = read.corpus_receipt()?;
    if (receipt.profile != tos_compiler::CORPUS_ORIGINAL_PROFILE
        && receipt.profile != tos_compiler::NATIVE_CORPUS_ORIGINAL_PROFILE)
        || receipt.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
        || receipt.source_cut != bound.selection().source_cut
        || receipt.membership_root != bound.selection().source_membership_root.to_hex()
    {
        return Err(fail(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected corpus component binding differs",
        ));
    }
    Ok(receipt)
}

/// The complete maintained GraphViews packet under the caller's remaining
/// original whole workspace. Other selected read routes keep their existing law.
pub fn execute_selected_corpus_graph_views_with_state<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    budget: CorpusReadBudget,
    remaining_state_bytes: usize,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    CorpusReadRequest::GraphViews.validate(context, budget)?;
    execute_selected_carrier_packet_with_state(
        model,
        bound,
        authority,
        CorpusReadRequest::GraphViews.operation_id(),
        CORPUS_INTENDED_USE,
        budget.inspect,
        remaining_state_bytes,
        budget.max_work_steps,
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let (ordinal, header) = read
                .corpus_row(&receipt, Collection::Header, &Selector::All, None)?
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus header absent",
                    )
                })?;
            if ordinal != 0 || header.as_object().is_none() {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus header invalid",
                ));
            }
            let expected = receipt
                .collections
                .iter()
                .find(|r| r.collection == Collection::GraphViews.as_str())
                .map(|r| r.rows)
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus graph-view receipt absent",
                    )
                })?;
            if expected > budget.max_work_steps {
                return Err(budget_error());
            }
            let capacity = usize::try_from(expected).map_err(|_| budget_error())?;
            let slots = capacity
                .checked_mul(std::mem::size_of::<JsonValue>())
                .ok_or_else(budget_error)?;
            read.reserve_state(slots)?;
            let mut views = Vec::new();
            views
                .try_reserve_exact(capacity)
                .map_err(|_| budget_error())?;
            let actual_slots = views
                .capacity()
                .checked_mul(std::mem::size_of::<JsonValue>())
                .ok_or_else(budget_error)?;
            read.reserve_state(actual_slots.checked_sub(slots).ok_or_else(budget_error)?)?;
            let mut packet = read.parse_state_packet(
                br#"{"schema":"tos_corpus_mcp_graph_views_v1","graph_views":[]}"#,
            )?;
            let mut after = None;
            let mut count = 0u64;
            loop {
                let Some((ordinal, value)) =
                    read.corpus_row(&receipt, Collection::GraphViews, &Selector::All, after)?
                else {
                    break;
                };
                if after.is_some_and(|previous| ordinal <= previous) {
                    return Err(fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "corpus original page order differs",
                    ));
                }
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= expected)
                    .ok_or_else(budget_error)?;
                if count > budget.max_work_steps {
                    return Err(budget_error());
                }
                // Same supported view filter as CorpusRead::views; inspect keys
                // by borrowed UTF-8 rather than allocating a lookup key buffer.
                let view_id = value
                    .as_object()
                    .and_then(|fields| {
                        fields
                            .iter()
                            .find(|(key, _)| key.as_str() == Some("view_id"))
                    })
                    .and_then(|(_, v)| v.as_str());
                if view_id.is_some_and(supported) {
                    views.push(value);
                }
                after = Some(ordinal);
                read.check_interrupt()?;
            }
            if count != expected {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus graph-view coverage incomplete",
                ));
            }
            let JsonValue::Object(fields) = &mut packet else {
                return Err(budget_error());
            };
            let (_, target) = fields
                .iter_mut()
                .find(|(key, _)| key.as_str() == Some("graph_views"))
                .ok_or_else(budget_error)?;
            *target = JsonValue::Array(views);
            // Header was genuinely consulted and remains charged until this
            // packet has been completed; no header projection is substituted.
            drop(header);
            Ok(packet)
        },
    )
}

/// Site-default identity projection, not the complete Summary packet. Consult
/// only cold-verified GraphViews index identities in encounter order, stopping
/// at the first supported view. The existing summary scope and disclosure hold
/// cover every consulted identity, including skipped/null entries.
pub fn execute_selected_corpus_view_ids<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: CorpusReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if budget.max_work_steps == 0 {
        return Err(budget_error());
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        CorpusReadRequest::Summary.operation_id(),
        CORPUS_INTENDED_USE,
        budget.inspect,
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let mut after = None;
            let mut remaining = budget.max_work_steps;
            let mut views = vec![];
            loop {
                if remaining == 0 {
                    return Err(budget_error());
                }
                let Some(row) = read.corpus_view_identity(&receipt, after)? else {
                    if after.is_none() {
                        return Err(fail(
                            SearchV2ErrorCode::Unavailable,
                            "selected corpus view identities unavailable",
                        ));
                    }
                    break;
                };
                remaining -= 1;
                after = Some(row.ordinal);
                if let Some(id) = row.view_id.filter(|id| supported(id)) {
                    views.push(object(vec![("view_id", text(&id))]));
                    break;
                }
            }
            Ok(object(vec![("graph_views", array(views))]))
        },
    )
}

/// The existing route-graph kernel under the same cumulative selected Reader.
pub(crate) fn graph_for_evidence<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    bound: &BoundCmpKnowledge<'_>,
    context: &CorpusReadContext,
    remaining: u64,
) -> Result<(JsonValue, u64), SearchV2Error> {
    let receipt = bound_original_receipt(read, bound)?;
    let (ordinal, header) = read
        .corpus_row(&receipt, Collection::Header, &Selector::All, None)?
        .ok_or_else(|| {
            fail(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "selected corpus header absent",
            )
        })?;
    if ordinal != 0 || header.as_object().is_none() {
        return Err(fail(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected corpus header invalid",
        ));
    }
    let mut corpus = CorpusRead {
        read,
        receipt,
        header,
        context,
        remaining,
    };
    let graph = corpus.graph_view("route-graph", 1000)?;
    Ok((graph, corpus.remaining))
}

/// Exact completed selected corpus metadata, including original graph-view rows.
/// The public projection's original rows remain under the same selected reader
/// and terminal disclosure lease; this does not materialize the full index.
pub fn execute_selected_corpus_header<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    context: &CorpusReadContext,
    budget: CorpusReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    CorpusReadRequest::Status.validate(context, budget)?;
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        "tos_corpus_header",
        CORPUS_INTENDED_USE,
        budget.inspect,
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let (ordinal, header) = read
                .corpus_row(&receipt, Collection::Header, &Selector::All, None)?
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus header absent",
                    )
                })?;
            if ordinal != 0 || header.as_object().is_none() {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus header invalid",
                ));
            }
            let expected = receipt
                .collections
                .iter()
                .find(|row| row.collection == Collection::GraphViews.as_str())
                .map(|row| row.rows)
                .ok_or_else(|| {
                    fail(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected corpus graph-view receipt absent",
                    )
                })?;
            if expected > budget.max_work_steps {
                return Err(budget_error());
            }
            let mut corpus = CorpusRead {
                read,
                receipt,
                header,
                context,
                remaining: budget.max_work_steps,
            };
            let views = corpus.rows(Collection::GraphViews, Selector::All, None)?;
            if views.len() as u64 != expected {
                return Err(fail(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected corpus graph-view coverage incomplete",
                ));
            }
            set(&mut corpus.header, "graph_views", JsonValue::Array(views));
            Ok(corpus.header)
        },
    )
}
