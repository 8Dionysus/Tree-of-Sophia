import {
  boundedClusters,
  boundedGraph,
  HttpError,
  itemId,
  objectArray,
  sourceRefs,
  stringArray,
  stringValue,
  type Item,
  uniqueValues,
} from "./common";
import {
  compactView,
  corpusItem,
  fullClusters,
  itemsByIds,
  jsonRows,
  maskFor,
  meta,
  metaItem,
  philosophyEdge,
  philosophyItem,
  philosophyNode,
  positions,
  rowJson,
  rows,
  viewMask,
  type EdgeRow,
} from "./store";

const CHALLENGE_PREDICATES = new Set(["contested_by", "uncertain_relation", "polemicizes_with"]);
const PATH_STATE_LIMIT = 50_000;
const PATH_FRONTIER_LIMIT = 5_000;

type ItemKind = "node" | "edge";
type SelectedItem = { item: Item; kind: ItemKind; viewMask: number; layerMask: number };

function properties(item: Item): Item {
  const value = item.properties;
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Item) : {};
}

function bool(value: unknown): boolean {
  return value === true;
}

function runtimeBoundary(top: Item): unknown {
  return top.runtime_projection_boundary ?? {};
}

async function selectedPhilosophyItem(db: D1Database, id: string): Promise<SelectedItem> {
  type SelectedRow = { json: string; view_mask: number; layer_mask: number };
  const node = await db
    .prepare("SELECT json, view_mask, layer_mask FROM philosophy_nodes WHERE id = ?")
    .bind(id)
    .first<SelectedRow>();
  if (node) return { item: JSON.parse(node.json) as Item, kind: "node", viewMask: node.view_mask, layerMask: node.layer_mask };
  const edge = await db
    .prepare("SELECT json, view_mask, layer_mask FROM philosophy_edges WHERE id = ?")
    .bind(id)
    .first<SelectedRow>();
  if (edge) return { item: JSON.parse(edge.json) as Item, kind: "edge", viewMask: edge.view_mask, layerMask: edge.layer_mask };
  throw new HttpError(404, `unknown ToS philosophy projection item: ${id}`);
}

export async function corpusSearch(db: D1Database, query: string, limit: number): Promise<Item> {
  const needle = query.trim().toLowerCase();
  const results = await rows<{ collection: string; json: string }>(
    db,
    `SELECT collection, json
       FROM corpus_items
      WHERE (? = '' OR instr(search_text, ?) > 0)
      ORDER BY CASE collection
        WHEN 'nodes' THEN 0 WHEN 'resources' THEN 1 WHEN 'manifests' THEN 2
        WHEN 'branches' THEN 3 WHEN 'graph_views' THEN 4 ELSE 5 END, ord
      LIMIT ?`,
    needle,
    needle,
    limit,
  );
  return {
    schema: "tos_corpus_mcp_search_v1",
    query,
    resource_kind: null,
    result_count: results.length,
    results: results.map((row) => ({ collection: row.collection, item: JSON.parse(row.json) })),
    authority_note: "Tree-of-Sophia owns corpus meaning; this MCP packet is an abyss-stack access-plane view.",
  };
}

export async function philosophySearch(db: D1Database, query: string, limit: number): Promise<Item> {
  const needle = query.trim().toLowerCase();
  const result: Item[] = [];
  const collections: Array<{ name: string; table: string; where: string }> = [
    { name: "views", table: "philosophy_aux", where: "collection = 'views'" },
    { name: "nodes", table: "philosophy_nodes", where: "1 = 1" },
    { name: "edges", table: "philosophy_edges", where: "1 = 1" },
    { name: "clusters", table: "philosophy_aux", where: "collection = 'clusters'" },
    { name: "review_packets", table: "philosophy_aux", where: "collection = 'review_packets'" },
    { name: "graph_layers", table: "philosophy_aux", where: "collection = 'graph_layers'" },
  ];
  for (const collection of collections) {
    const remaining = limit - result.length;
    if (remaining <= 0) break;
    const matches = await jsonRows(
      db,
      `SELECT json FROM ${collection.table}
        WHERE ${collection.where} AND (? = '' OR instr(search_text, ?) > 0)
        ORDER BY ord LIMIT ?`,
      needle,
      needle,
      remaining,
    );
    result.push(...matches.map((item) => ({ collection: collection.name, item })));
  }
  return {
    schema: "tos_philosophy_mcp_search_v1",
    query,
    result_count: result.length,
    results: result,
    authority_note: "Tree-of-Sophia owns philosophy meaning; this MCP search result is an access-plane packet.",
  };
}

export async function corpusNodePacket(db: D1Database, nodeId: string): Promise<Item> {
  let matches = await corpusItem(db, "nodes", nodeId);
  const relatedEdges = await jsonRows(
    db,
    "SELECT json FROM corpus_edges WHERE from_id = ? OR to_id = ? ORDER BY ord",
    nodeId,
    nodeId,
  );
  if (matches.length === 0 && relatedEdges.length > 0) {
    matches = [
      {
        node_id: nodeId,
        label: nodeId,
        node_type: "relation-endpoint",
        owner_branches: uniqueValues(relatedEdges, "owner_branch"),
        source_refs: sourceRefs(relatedEdges),
        projection_posture: "identity materialized from indexed relation endpoints",
      },
    ];
  }
  if (matches.length === 0) throw new HttpError(404, `unknown ToS corpus node: ${nodeId}`);
  return {
    schema: "tos_corpus_mcp_node_v1",
    node_id: nodeId,
    matches,
    related_edges: relatedEdges,
    authority_note: "Node authority stays in the source_path named by the index.",
  };
}

export async function corpusRelationPack(db: D1Database, packId: string): Promise<Item> {
  const packs = await jsonRows(db, "SELECT json FROM corpus_packs WHERE id = ? ORDER BY ord", packId);
  if (packs.length === 0) throw new HttpError(404, `unknown ToS corpus relation pack: ${packId}`);
  const edges = await jsonRows(db, "SELECT json FROM corpus_edges WHERE pack_id = ? ORDER BY ord", packId);
  return {
    schema: "tos_corpus_mcp_relation_pack_v1",
    pack_id: packId,
    packs,
    edges,
    authority_note: "Relation-pack authority stays in the ToS path named by the pack.",
  };
}

export async function corpusGraphView(db: D1Database, viewId: string, limit: number): Promise<Item> {
  const supported = new Set(["corpus-topology", "route-graph", "promotion-flow"]);
  const [views, top] = await Promise.all([
    corpusItem(db, "graph_views", viewId),
    metaItem(db, "corpus_top"),
  ]);
  const view = views[0];
  if (!view) throw new HttpError(404, `unknown ToS graph view: ${viewId}`);
  if (!supported.has(viewId)) throw new HttpError(404, `unsupported standalone ToS graph view: ${viewId}`);

  let items: Item[] = [];
  let graphNodes: Item[] = [];
  let graphEdges: Item[] = [];
  if (viewId === "corpus-topology") {
    items = await jsonRows(
      db,
      "SELECT json FROM corpus_items WHERE collection = 'branches' ORDER BY ord LIMIT ?",
      limit,
    );
    const rootId = `view:${viewId}`;
    graphNodes = [{
      node_id: rootId,
      label: view.title || viewId,
      node_type: "corpus-root",
      source_ref: view.entry_surface,
    }];
    for (const branch of items) {
      const branchId = stringValue(branch.id);
      if (!branchId) continue;
      const sourceRef = branch.owner_surface || branch.path;
      graphNodes.push({
        ...branch,
        node_id: branchId,
        label: branchId,
        node_type: "corpus-branch",
        source_ref: sourceRef,
      });
      graphEdges.push({
        edge_id: `corpus-edge:${rootId}:${branchId}`,
        from_id: rootId,
        to_id: branchId,
        predicate_id: "contains",
        source_ref: sourceRef,
      });
    }
  } else {
    if (viewId === "route-graph") {
      graphEdges = await jsonRows(
        db,
        `SELECT edge.json
           FROM corpus_edges edge
           JOIN corpus_packs pack ON pack.id = edge.pack_id
          WHERE json_extract(pack.json, '$.owner_branch') = 'ToS/canon'
          ORDER BY edge.ord LIMIT ?`,
        limit,
      );
    } else {
      graphEdges = await jsonRows(
        db,
        "SELECT json FROM corpus_edges WHERE owner_branch = 'ToS/candidate-intake' ORDER BY ord LIMIT ?",
        limit,
      );
    }
    items = graphEdges;
    const endpointIds = [...new Set(
      graphEdges.flatMap((edge) => [stringValue(edge.from_id), stringValue(edge.to_id)]).filter(Boolean),
    )];
    const indexedNodes = endpointIds.length
      ? await jsonRows(
        db,
        "SELECT json FROM corpus_items WHERE collection = 'nodes' AND id IN (SELECT value FROM json_each(?)) ORDER BY ord",
        JSON.stringify(endpointIds),
      )
      : [];
    if (viewId === "route-graph") {
      const selected = new Set(endpointIds);
      graphNodes = indexedNodes.filter((node) => selected.has(stringValue(node.node_id)));
    } else {
      const byId = new Map(indexedNodes.map((node) => [stringValue(node.node_id), node]));
      const endpointSourceRefs = new Map(endpointIds.map((id) => [id, new Set<string>()]));
      for (const edge of graphEdges) {
        const sourceRef = stringValue(edge.source_ref);
        if (!sourceRef) continue;
        for (const endpoint of [stringValue(edge.from_id), stringValue(edge.to_id)]) {
          endpointSourceRefs.get(endpoint)?.add(sourceRef);
        }
      }
      graphNodes = [...endpointIds].sort().map((id) => byId.get(id) ?? ({
        node_id: id,
        label: id,
        node_type: "candidate-endpoint",
        authority_layer: "candidate_intake",
        owner_branch: "ToS/candidate-intake",
        source_refs: [...(endpointSourceRefs.get(id) ?? [])].sort(),
      }));
    }
  }
  return {
    schema: "tos_corpus_mcp_graph_view_v1",
    view,
    item_count: items.length,
    items,
    node_count: graphNodes.length,
    edge_count: graphEdges.length,
    nodes: graphNodes,
    edges: graphEdges,
    counts: top.counts ?? {},
    runtime_projection_boundary: top.runtime_projection_boundary ?? {},
  };
}

export async function philosophyNodePacket(db: D1Database, nodeId: string): Promise<Item> {
  const node = await philosophyNode(db, nodeId);
  if (!node) throw new HttpError(404, `unknown ToS philosophy node: ${nodeId}`);
  const relatedEdges = await jsonRows(
    db,
    "SELECT json FROM philosophy_edges WHERE from_id = ? OR to_id = ? ORDER BY ord",
    nodeId,
    nodeId,
  );
  return {
    schema: "tos_philosophy_mcp_node_v1",
    node_id: nodeId,
    node,
    related_edges: relatedEdges,
    source_refs: sourceRefs([node, ...relatedEdges]),
    authority_note: "Node source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
  };
}

export async function philosophyEdgePacket(db: D1Database, edgeId: string): Promise<Item> {
  const edge = await philosophyEdge(db, edgeId);
  if (!edge) throw new HttpError(404, `unknown ToS philosophy edge: ${edgeId}`);
  const endpointIds = [stringValue(edge.from_id), stringValue(edge.to_id)].filter(Boolean);
  const endpoints = await itemsByIds(db, "philosophy_nodes", endpointIds);
  return {
    schema: "tos_philosophy_mcp_edge_v1",
    edge_id: edgeId,
    edge,
    endpoints,
    source_refs: sourceRefs([edge, ...endpoints]),
    authority_note: "Edge source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
  };
}

export async function philosophyClusters(
  db: D1Database,
  viewId: string | null,
  clusterKind: string | null,
  limit: number,
): Promise<Item> {
  const [top, selectedViewMask] = await Promise.all([metaItem(db, "philosophy_top"), viewMask(db, viewId)]);
  const clusters = await fullClusters(db, { viewMask: selectedViewMask, kind: clusterKind, limit });
  return {
    schema: "tos_philosophy_mcp_clusters_v1",
    view_id: viewId,
    cluster_kind: clusterKind,
    clusters,
    cluster_count: clusters.length,
    counts: top.counts ?? {},
    source_refs: sourceRefs(clusters),
    runtime_projection_boundary: runtimeBoundary(top),
  };
}

export async function philosophyView(db: D1Database, viewId: string, limit: number): Promise<Item> {
  const [top, view, selectedViewMask] = await Promise.all([
    metaItem(db, "philosophy_top"),
    compactView(db, viewId),
    viewMask(db, viewId),
  ]);
  if (selectedViewMask === null) throw new HttpError(404, `unknown ToS philosophy graph view: ${viewId}`);

  const edgeFetchLimit = Math.min(13_307, Math.max(1000, limit * 8));
  const [candidateNodes, candidateEdges, clusters, reviewRows] = await Promise.all([
    jsonRows(db, "SELECT json FROM philosophy_nodes WHERE (view_mask & ?) != 0 ORDER BY ord LIMIT ?", selectedViewMask, Math.max(limit * 2, limit)),
    jsonRows(db, "SELECT json FROM philosophy_edges WHERE (view_mask & ?) != 0 ORDER BY ord LIMIT ?", selectedViewMask, edgeFetchLimit),
    fullClusters(db, { viewMask: selectedViewMask, limit: 1000 }),
    jsonRows(db, "SELECT json FROM philosophy_review_packets WHERE view_id = ?", viewId),
  ]);
  const bounded = boundedGraph(candidateNodes, candidateEdges, limit);
  const boundedView: Item = {
    ...view,
    node_ids: bounded.nodes.map((node) => stringValue(node.node_id)).filter(Boolean),
    edge_ids: bounded.edges.map((edge) => stringValue(edge.edge_id)).filter(Boolean),
  };
  const reviewPacket = reviewRows[0];
  if (!reviewPacket) throw new HttpError(404, `unknown ToS philosophy review packet view: ${viewId}`);
  return {
    schema: "tos_philosophy_mcp_view_v1",
    view: boundedView,
    node_count: bounded.nodes.length,
    edge_count: bounded.edges.length,
    available_node_count: Number(view.node_count ?? candidateNodes.length),
    available_edge_count: Number(view.edge_count ?? candidateEdges.length),
    limit,
    nodes: bounded.nodes,
    edges: bounded.edges,
    clusters: boundedClusters(clusters, bounded.nodes, bounded.edges).slice(0, limit),
    review_packet: {
      schema: "tos_philosophy_mcp_review_packet_v1",
      packet: reviewPacket,
      runtime_projection_boundary: runtimeBoundary(top),
      authority_note: "Tree-of-Sophia owns review packet semantics; MCP serves the compact access packet.",
    },
    source_refs: view.source_refs ?? [],
    runtime_projection_boundary: runtimeBoundary(top),
  };
}

async function filterMasks(db: D1Database, layers: string[]): Promise<number | null> {
  return maskFor(layers, await positions(db, "layer_positions"));
}

function predicateSql(filters: string[]): { sql: string; binding: string } {
  return filters.length === 0
    ? { sql: "1 = 1", binding: "[]" }
    : { sql: "predicate_id IN (SELECT value FROM json_each(?))", binding: JSON.stringify(filters) };
}

export async function philosophyNeighborhood(
  db: D1Database,
  nodeId: string,
  depth: number,
  layers: string[],
  predicates: string[],
  limit: number,
): Promise<Item> {
  const [node, top, layerMask] = await Promise.all([
    philosophyNode(db, nodeId),
    metaItem(db, "philosophy_top"),
    filterMasks(db, layers),
  ]);
  if (!node) throw new HttpError(404, `unknown ToS philosophy node: ${nodeId}`);
  const predicate = predicateSql(predicates);
  const selectedIds = new Set([nodeId]);
  const discoveryOrder = [nodeId];
  let frontier = [nodeId];
  const traversalEdges: Item[] = [];
  const selectedEdgeIds = new Set<string>();

  for (let level = 0; level < depth && frontier.length > 0 && discoveryOrder.length - 1 < limit; level += 1) {
    const edgeRows = await rows<EdgeRow>(
      db,
      `SELECT id, ord, from_id, to_id, predicate_id, view_mask, layer_mask, json
         FROM philosophy_edges
        WHERE (from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?)))
          AND (? IS NULL OR (layer_mask & ?) != 0)
          AND ${predicate.sql}
        ORDER BY ord`,
      JSON.stringify(frontier),
      JSON.stringify(frontier),
      layerMask,
      layerMask,
      ...(predicates.length === 0 ? [] : [predicate.binding]),
    );
    const candidateIds = new Set<string>();
    for (const edge of edgeRows) {
      if (frontier.includes(edge.from_id) && !selectedIds.has(edge.to_id)) candidateIds.add(edge.to_id);
      if (frontier.includes(edge.to_id) && !selectedIds.has(edge.from_id)) candidateIds.add(edge.from_id);
    }
    const allowedRows = await rows<{ id: string; ord: number }>(
      db,
      `SELECT id, ord FROM philosophy_nodes
        WHERE id IN (SELECT value FROM json_each(?))
          AND (? IS NULL OR (layer_mask & ?) != 0)
        ORDER BY ord`,
      JSON.stringify([...candidateIds]),
      layerMask,
      layerMask,
    );
    const order = new Map(allowedRows.map((row) => [row.id, row.ord]));
    const candidates = new Map<string, EdgeRow>();
    for (const edge of edgeRows) {
      if (frontier.includes(edge.from_id) && !selectedIds.has(edge.to_id) && order.has(edge.to_id) && !candidates.has(edge.to_id)) {
        candidates.set(edge.to_id, edge);
      }
      if (frontier.includes(edge.to_id) && !selectedIds.has(edge.from_id) && order.has(edge.from_id) && !candidates.has(edge.from_id)) {
        candidates.set(edge.from_id, edge);
      }
    }
    const nextFrontier: string[] = [];
    for (const candidateId of [...candidates.keys()].sort((left, right) => (order.get(left) ?? 1e9) - (order.get(right) ?? 1e9) || left.localeCompare(right))) {
      if (discoveryOrder.length - 1 >= limit) break;
      selectedIds.add(candidateId);
      discoveryOrder.push(candidateId);
      nextFrontier.push(candidateId);
      const edge = candidates.get(candidateId);
      if (edge && !selectedEdgeIds.has(edge.id)) {
        traversalEdges.push(rowJson(edge));
        selectedEdgeIds.add(edge.id);
      }
    }
    frontier = nextFrontier;
  }

  const neighbors = await itemsByIds(db, "philosophy_nodes", discoveryOrder.slice(1));
  const retainedEdges = [...traversalEdges];
  if (retainedEdges.length < limit) {
    const enclosed = await jsonRows(
      db,
      `SELECT json FROM philosophy_edges
        WHERE from_id IN (SELECT value FROM json_each(?))
          AND to_id IN (SELECT value FROM json_each(?))
          AND (? IS NULL OR (layer_mask & ?) != 0)
          AND ${predicate.sql}
        ORDER BY ord`,
      JSON.stringify([...selectedIds]),
      JSON.stringify([...selectedIds]),
      layerMask,
      layerMask,
      ...(predicates.length === 0 ? [] : [predicate.binding]),
    );
    for (const edge of enclosed) {
      if (retainedEdges.length >= limit) break;
      const edgeId = stringValue(edge.edge_id);
      if (!selectedEdgeIds.has(edgeId)) {
        retainedEdges.push(edge);
        selectedEdgeIds.add(edgeId);
      }
    }
  }
  return {
    schema: "tos_philosophy_mcp_neighborhood_v1",
    node,
    neighbors,
    edges: retainedEdges,
    depth,
    layers: [...new Set(layers)].sort(),
    predicates: [...new Set(predicates)].sort(),
    limit,
    source_refs: sourceRefs([node, ...neighbors, ...retainedEdges]),
    runtime_projection_boundary: runtimeBoundary(top),
  };
}

type PathState = {
  current: string;
  nodeIds: string[];
  edgeIds: string[];
  traversal: Item[];
};

export async function philosophyPath(
  db: D1Database,
  options: {
    fromId: string;
    toId: string;
    layers: string[];
    predicates: string[];
    maxDepth: number;
    direction: string;
    viewId: string | null;
    excludedEdgeIds: string[];
    alternativeLimit: number;
  },
): Promise<Item> {
  const { fromId, toId, layers, predicates, maxDepth, direction, viewId, alternativeLimit } = options;
  if (!new Set(["outgoing", "incoming", "either"]).has(direction)) {
    throw new HttpError(400, "direction must be outgoing, incoming, or either");
  }
  const [fromNode, toNode, top, selectedViewMask, layerMask] = await Promise.all([
    philosophyNode(db, fromId),
    philosophyNode(db, toId),
    metaItem(db, "philosophy_top"),
    viewMask(db, viewId),
    filterMasks(db, layers),
  ]);
  if (!fromNode) throw new HttpError(404, `unknown ToS philosophy node: ${fromId}`);
  if (!toNode) throw new HttpError(404, `unknown ToS philosophy node: ${toId}`);
  if (selectedViewMask !== null) {
    const available = await rows<{ id: string }>(
      db,
      "SELECT id FROM philosophy_nodes WHERE id IN (SELECT value FROM json_each(?)) AND (view_mask & ?) != 0",
      JSON.stringify([fromId, toId]),
      selectedViewMask,
    );
    if (available.length < 2 && fromId !== toId) {
      return emptyPathPacket(options, top, 1, 1, false);
    }
  }
  const excluded = new Set(options.excludedEdgeIds.filter(Boolean));
  const predicate = predicateSql(predicates);
  let queue: PathState[] = [{ current: fromId, nodeIds: [fromId], edgeIds: [], traversal: [] }];
  const completed: PathState[] = [];
  let exploredStates = 0;
  let enqueuedStates = 1;
  let maxFrontierSize = 1;
  let explorationTruncated = false;

  while (queue.length > 0 && completed.length < alternativeLimit) {
    if (exploredStates >= PATH_STATE_LIMIT) {
      explorationTruncated = true;
      break;
    }
    const currentIds = [...new Set(queue.map((state) => state.current))];
    const edgeRows = await rows<EdgeRow>(
      db,
      `SELECT id, ord, from_id, to_id, predicate_id, view_mask, layer_mask
         FROM philosophy_edges
        WHERE (from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?)))
          AND (? IS NULL OR (view_mask & ?) != 0)
          AND (? IS NULL OR (layer_mask & ?) != 0)
          AND ${predicate.sql}
        ORDER BY id, from_id, to_id`,
      JSON.stringify(currentIds),
      JSON.stringify(currentIds),
      selectedViewMask,
      selectedViewMask,
      layerMask,
      layerMask,
      ...(predicates.length === 0 ? [] : [predicate.binding]),
    );
    const adjacency = new Map<string, Array<{ neighbor: string; edgeId: string; direction: string }>>();
    for (const edge of edgeRows) {
      if (excluded.has(edge.id)) continue;
      if (direction === "outgoing" || direction === "either") {
        const values = adjacency.get(edge.from_id) ?? [];
        values.push({ neighbor: edge.to_id, edgeId: edge.id, direction: "forward" });
        adjacency.set(edge.from_id, values);
      }
      if ((direction === "incoming" || direction === "either") && (edge.from_id !== edge.to_id || direction === "incoming")) {
        const values = adjacency.get(edge.to_id) ?? [];
        values.push({ neighbor: edge.from_id, edgeId: edge.id, direction: "reverse" });
        adjacency.set(edge.to_id, values);
      }
    }
    const nextQueue: PathState[] = [];
    for (const state of queue) {
      if (exploredStates >= PATH_STATE_LIMIT || completed.length >= alternativeLimit) break;
      exploredStates += 1;
      if (state.current === toId) {
        completed.push(state);
        continue;
      }
      if (state.edgeIds.length >= maxDepth) continue;
      for (const step of adjacency.get(state.current) ?? []) {
        if (state.nodeIds.includes(step.neighbor)) continue;
        if (enqueuedStates >= PATH_STATE_LIMIT || nextQueue.length >= PATH_FRONTIER_LIMIT) {
          explorationTruncated = true;
          break;
        }
        nextQueue.push({
          current: step.neighbor,
          nodeIds: [...state.nodeIds, step.neighbor],
          edgeIds: [...state.edgeIds, step.edgeId],
          traversal: [
            ...state.traversal,
            {
              edge_id: step.edgeId,
              from_node_id: state.current,
              to_node_id: step.neighbor,
              edge_direction: step.direction,
            },
          ],
        });
        enqueuedStates += 1;
      }
    }
    queue = nextQueue;
    maxFrontierSize = Math.max(maxFrontierSize, queue.length);
  }

  const allNodeIds = [...new Set(completed.flatMap((path) => path.nodeIds))];
  const allEdgeIds = [...new Set(completed.flatMap((path) => path.edgeIds))];
  const [nodeItems, edgeItems] = await Promise.all([
    itemsByIds(db, "philosophy_nodes", allNodeIds),
    itemsByIds(db, "philosophy_edges", allEdgeIds),
  ]);
  const nodesById = new Map(nodeItems.map((item) => [stringValue(item.node_id), item]));
  const edgesById = new Map(edgeItems.map((item) => [stringValue(item.edge_id), item]));
  const paths = completed.map((path, index) => {
    const nodes = path.nodeIds.map((id) => nodesById.get(id)).filter((item): item is Item => Boolean(item));
    const edges = path.edgeIds.map((id) => edgesById.get(id)).filter((item): item is Item => Boolean(item));
    return {
      path_index: index,
      node_ids: path.nodeIds,
      edge_ids: path.edgeIds,
      nodes,
      edges,
      traversal: path.traversal,
      source_refs: sourceRefs([...nodes, ...edges]),
    };
  });
  const primary = paths[0] ?? { nodes: [], edges: [] };
  return {
    schema: "tos_philosophy_mcp_path_v2",
    from_id: fromId,
    to_id: toId,
    found: paths.length > 0,
    path_count: paths.length,
    paths,
    nodes: primary.nodes,
    edges: primary.edges,
    max_depth: maxDepth,
    direction,
    view_id: viewId,
    excluded_edge_ids: [...excluded].sort(),
    alternative_limit: alternativeLimit,
    exploration_truncated: explorationTruncated,
    explored_state_count: exploredStates,
    enqueued_state_count: enqueuedStates,
    frontier_limit: PATH_FRONTIER_LIMIT,
    max_frontier_size: maxFrontierSize,
    layers: [...new Set(layers)].sort(),
    predicates: [...new Set(predicates)].sort(),
    source_refs: sourceRefs([...nodeItems, ...edgeItems]),
    runtime_projection_boundary: runtimeBoundary(top),
    authority_note: "Tree-of-Sophia owns graph meaning; MCP serves a bounded path packet.",
  };
}

function emptyPathPacket(
  options: {
    fromId: string;
    toId: string;
    layers: string[];
    predicates: string[];
    maxDepth: number;
    direction: string;
    viewId: string | null;
    excludedEdgeIds: string[];
    alternativeLimit: number;
  },
  top: Item,
  exploredStates: number,
  enqueuedStates: number,
  truncated: boolean,
): Item {
  return {
    schema: "tos_philosophy_mcp_path_v2",
    from_id: options.fromId,
    to_id: options.toId,
    found: false,
    path_count: 0,
    paths: [],
    nodes: [],
    edges: [],
    max_depth: options.maxDepth,
    direction: options.direction,
    view_id: options.viewId,
    excluded_edge_ids: [...new Set(options.excludedEdgeIds)].sort(),
    alternative_limit: options.alternativeLimit,
    exploration_truncated: truncated,
    explored_state_count: exploredStates,
    enqueued_state_count: enqueuedStates,
    frontier_limit: PATH_FRONTIER_LIMIT,
    max_frontier_size: 1,
    layers: [...new Set(options.layers)].sort(),
    predicates: [...new Set(options.predicates)].sort(),
    source_refs: [],
    runtime_projection_boundary: runtimeBoundary(top),
    authority_note: "Tree-of-Sophia owns graph meaning; MCP serves a bounded path packet.",
  };
}

export async function philosophyEpistemic(
  db: D1Database,
  itemIdValue: string,
  viewId: string | null,
  limit: number,
): Promise<Item> {
  const [selected, top, selectedViewMask] = await Promise.all([
    selectedPhilosophyItem(db, itemIdValue),
    metaItem(db, "philosophy_top"),
    viewMask(db, viewId),
  ]);
  if (selectedViewMask !== null && (selected.viewMask & selectedViewMask) === 0) {
    throw new HttpError(404, `ToS philosophy projection item is not present in view ${viewId}: ${itemIdValue}`);
  }
  const selectedNodeIds =
    selected.kind === "node"
      ? [itemIdValue]
      : [stringValue(selected.item.from_id), stringValue(selected.item.to_id)].filter(Boolean);
  const candidates = await jsonRows(
    db,
    `SELECT json FROM philosophy_edges
      WHERE (? IS NULL OR (view_mask & ?) != 0)
        AND (
          id = ? OR from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?))
        )
      ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, id`,
    selectedViewMask,
    selectedViewMask,
    itemIdValue,
    JSON.stringify(selectedNodeIds),
    JSON.stringify(selectedNodeIds),
    itemIdValue,
  );
  const selectedEdgeIsContext = selected.kind === "edge" && !CHALLENGE_PREDICATES.has(stringValue(selected.item.predicate_id));
  const challengeCapacity = Math.max(0, limit - (selectedEdgeIsContext ? 1 : 0));
  const availableChallenges = candidates.filter((edge) => CHALLENGE_PREDICATES.has(stringValue(edge.predicate_id)));
  const challengeRelations = availableChallenges.slice(0, challengeCapacity);
  const challengeIds = new Set(challengeRelations.map((edge) => stringValue(edge.edge_id)));
  const contextRelations = candidates.filter((edge) => !challengeIds.has(stringValue(edge.edge_id))).slice(0, limit - challengeRelations.length);
  const selectedRelations = [...challengeRelations, ...contextRelations];
  const relatedNodeIds = [...new Set(selectedRelations.flatMap((edge) => [stringValue(edge.from_id), stringValue(edge.to_id)]).filter(Boolean))];
  const neighborNodes = (await itemsByIds(db, "philosophy_nodes", relatedNodeIds)).filter(
    (node) => selected.kind !== "node" || stringValue(node.node_id) !== itemIdValue,
  );
  const surrounding = [...challengeRelations, ...contextRelations, ...neighborNodes];
  const fieldItems = surrounding.filter((item) => itemId(item) !== itemIdValue);
  const fieldProperties = fieldItems.map(properties);
  const selectedProperties = properties(selected.item);
  const confidence = (item: Item): string => stringValue(item.confidence || item.master_confidence);
  return {
    schema: "tos_philosophy_epistemic_packet_v1",
    item_id: itemIdValue,
    view_id: viewId,
    selection: selected.item,
    challenge_relations: challengeRelations,
    context_relations: contextRelations,
    neighbor_nodes: neighborNodes,
    selection_posture: {
      authority_posture: selectedProperties.authority_posture ?? null,
      canon_status: selectedProperties.canon_status ?? null,
      review_posture: selectedProperties.review_posture ?? null,
      confidence: selectedProperties.confidence ?? selectedProperties.master_confidence ?? null,
      priority: selectedProperties.priority ?? null,
      claim_evidence_closed: bool(selectedProperties.claim_evidence_closed),
    },
    field_posture: {
      authority_postures: uniqueValues(fieldProperties, "authority_posture"),
      canon_statuses: uniqueValues(fieldProperties, "canon_status"),
      review_postures: uniqueValues(fieldProperties, "review_posture"),
      confidence_values: [...new Set(fieldProperties.map(confidence).filter(Boolean))].sort(),
    },
    coverage: {
      posture: "partial",
      challenge_state:
        challengeRelations.length < availableChallenges.length
          ? "projected_signals_truncated"
          : availableChallenges.length > 0
            ? "projected_signals"
            : "none_in_projection_scope",
      available_challenge_relations: availableChallenges.length,
      returned_challenge_relations: challengeRelations.length,
      missing_surfaces: [
        "claim-level support and counterevidence",
        "source-visible review decisions",
        "rights and publication decisions",
      ],
    },
    authority_boundary: { is_source: false, is_canon: false, is_semantic_truth: false, is_rights_clearance: false },
    counts: {
      challenge_relations: challengeRelations.length,
      available_challenge_relations: availableChallenges.length,
      context_relations: contextRelations.length,
      neighbor_nodes: neighborNodes.length,
      source_refs: sourceRefs([selected.item, ...surrounding]).length,
    },
    source_refs: sourceRefs([selected.item, ...surrounding]),
    challenge_predicates: [...CHALLENGE_PREDICATES].sort(),
    runtime_projection_boundary: runtimeBoundary(top),
    authority_note:
      "This packet exposes projected challenge signals and source-return routes. A contested_by, uncertain_relation, or polemicizes_with candidate is not adjudicated counterevidence; ToS source, claim, review, rights, and canon owners remain authoritative.",
  };
}

async function corpusEpistemic(db: D1Database, itemIdValue: string, viewId: string | null, limit: number): Promise<Item> {
  const selectedView = viewId || "route-graph";
  if (selectedView !== "route-graph") throw new HttpError(404, "corpus Evidence Lens currently supports the route-graph view");
  const edges = await jsonRows(db, "SELECT json FROM corpus_edges WHERE owner_branch = 'ToS/canon' ORDER BY ord LIMIT 1000");
  const endpointIds = [...new Set(edges.flatMap((edge) => [stringValue(edge.from_id), stringValue(edge.to_id)]).filter(Boolean))];
  const nodes = await jsonRows(
    db,
    "SELECT json FROM corpus_items WHERE collection = 'nodes' AND id IN (SELECT value FROM json_each(?)) ORDER BY ord",
    JSON.stringify(endpointIds),
  );
  const selection = [...nodes, ...edges].find((item) => itemId(item) === itemIdValue);
  if (!selection) throw new HttpError(404, `unknown ToS corpus route-graph item: ${itemIdValue}`);
  const selectionIsNode = Boolean(selection.node_id);
  const selectedNodeIds = selectionIsNode
    ? [itemIdValue]
    : [stringValue(selection.from_id), stringValue(selection.to_id)].filter(Boolean);
  const relationCandidates = edges
    .filter(
      (edge) =>
        stringValue(edge.edge_id) === itemIdValue ||
        selectedNodeIds.includes(stringValue(edge.from_id)) ||
        selectedNodeIds.includes(stringValue(edge.to_id)),
    )
    .sort((left, right) => Number(itemId(left) !== itemIdValue) - Number(itemId(right) !== itemIdValue) || itemId(left).localeCompare(itemId(right)));
  const contextRelations = relationCandidates.slice(0, limit);
  const relatedNodeIds = new Set(contextRelations.flatMap((edge) => [stringValue(edge.from_id), stringValue(edge.to_id)]).filter(Boolean));
  if (selectionIsNode) relatedNodeIds.delete(itemIdValue);
  const neighborNodes = nodes.filter((node) => relatedNodeIds.has(stringValue(node.node_id)));
  return {
    selection,
    challenge_relations: [],
    context_relations: contextRelations,
    neighbor_nodes: neighborNodes,
    selection_posture: {
      authority_posture: selection.authority_layer ?? null,
      canon_status: selection.status ?? null,
      review_posture: null,
      confidence: selection.confidence ?? null,
      priority: null,
      claim_evidence_closed: false,
    },
    field_posture: {
      authority_postures: uniqueValues(contextRelations, "authority_layer"),
      canon_statuses: uniqueValues(contextRelations, "status"),
      review_postures: [],
      confidence_values: uniqueValues(contextRelations, "confidence"),
    },
    coverage: {
      posture: "partial",
      challenge_state: "none_in_projection_scope",
      available_challenge_relations: 0,
      returned_challenge_relations: 0,
      missing_surfaces: ["curated Evidence Lens scene lookup pending"],
    },
    source_refs: sourceRefs([selection, ...contextRelations, ...neighborNodes]),
  };
}

export async function evidenceLens(
  db: D1Database,
  mode: "philosophy" | "corpus",
  itemIdValue: string,
  viewId: string | null,
  limit: number,
): Promise<Item> {
  const context =
    mode === "philosophy"
      ? await philosophyEpistemic(db, itemIdValue, viewId, limit)
      : await corpusEpistemic(db, itemIdValue, viewId, limit);
  const evidence = await metaItem(db, "evidence_projection");
  const scene = objectArray(evidence.scenes).find((candidate) =>
    objectArray(candidate.selections).some(
      (route) => stringValue(route.mode) === mode && stringArray(route.item_ids).includes(itemIdValue),
    ),
  );
  const selection = context.selection as Item;
  let finding: string;
  let findingRu: string;
  let posture: string;
  let conclusion: Item;
  let routesValue: Item[];
  let gaps: string[];
  let gapsRu: string[];
  let sourceAnchors: Item[];
  if (!scene) {
    finding = "No curated Evidence Lens route is published for this selection.";
    findingRu = "Для выбранного объекта ещё не опубликован курируемый маршрут Evidence Lens.";
    posture = "projection-only";
    conclusion = {
      can_conclude: false,
      canon_membership: selection.authority_layer === "canon",
      claim_evidence_closed: false,
      allowed: ["inspect the projection context and its source-return references"],
      allowed_ru: ["исследовать контекст проекции и её ссылки возврата к источникам"],
      not_allowed: ["infer evidence closure from projection membership"],
      not_allowed_ru: ["выводить доказательную замкнутость из присутствия в проекции"],
    };
    routesValue = [];
    gaps = ["curated source, review, rights, and claim/evidence routes"];
    gapsRu = ["курируемые маршруты к source, review, rights и claim/evidence"];
    sourceAnchors = [];
  } else {
    finding = stringValue(scene.finding);
    findingRu = stringValue(scene.finding_ru) || finding;
    posture = stringValue(scene.posture);
    conclusion = scene.conclusion && typeof scene.conclusion === "object" ? (scene.conclusion as Item) : {};
    routesValue = objectArray(scene.routes);
    gaps = stringArray(scene.gaps);
    gapsRu = stringArray(scene.gaps_ru);
    if (gapsRu.length === 0) gapsRu = gaps;
    sourceAnchors = objectArray(scene.source_anchors);
    const coverage = context.coverage as Item;
    coverage.missing_surfaces = gaps;
    coverage.posture = "curated-route";
  }
  const routeCounts: Record<string, number> = {};
  for (const route of routesValue) {
    const kind = stringValue(route.route_kind) || "other";
    routeCounts[kind] = (routeCounts[kind] ?? 0) + 1;
  }
  const projectionRefs = stringArray(context.source_refs);
  const lensSourceRefs = [...new Set([...projectionRefs, ...stringArray(scene?.source_refs)])].sort();
  const boundary = evidence.authority_boundary && typeof evidence.authority_boundary === "object" ? (evidence.authority_boundary as Item) : {};
  return {
    schema: "tos_evidence_lens_packet_v1",
    mode,
    item_id: itemIdValue,
    view_id: viewId,
    selection,
    scene: scene ?? null,
    finding,
    finding_ru: findingRu,
    posture,
    conclusion,
    source_anchors: sourceAnchors,
    routes: routesValue,
    gaps,
    gaps_ru: gapsRu,
    challenge_relations: context.challenge_relations,
    context_relations: context.context_relations,
    neighbor_nodes: context.neighbor_nodes,
    selection_posture: context.selection_posture,
    field_posture: context.field_posture,
    coverage: context.coverage,
    counts: {
      routes: routesValue.length,
      source_anchors: sourceAnchors.length,
      gaps: gaps.length,
      challenge_relations: objectArray(context.challenge_relations).length,
      context_relations: objectArray(context.context_relations).length,
    },
    source_refs: lensSourceRefs,
    authority_boundary: boundary,
    authority_note: stringValue(boundary.note),
    agent_summary: {
      selection: itemIdValue,
      finding,
      finding_ru: findingRu,
      posture,
      can_conclude: bool(conclusion.can_conclude),
      canon_membership: bool(conclusion.canon_membership),
      claim_evidence_closed: bool(conclusion.claim_evidence_closed),
      route_counts: routeCounts,
      gap_count: gaps.length,
      page_updated: true,
      next_actions: [
        "inspect the full route cards on the page",
        "open the referenced owner surface before making a stronger claim",
      ],
    },
  };
}

export async function philosophyPacket(db: D1Database, query: string, viewId: string | null, limit: number): Promise<Item> {
  const [top, search, view] = await Promise.all([
    metaItem(db, "philosophy_top"),
    query ? philosophySearch(db, query, limit) : Promise.resolve({ result_count: 0, results: [] } as Item),
    viewId ? philosophyView(db, viewId, limit) : Promise.resolve(null),
  ]);
  const compact = view
    ? {
        view: view.view,
        nodes: view.nodes,
        edges: view.edges,
        clusters: objectArray(view.clusters).slice(0, limit),
        review_packet: view.review_packet,
        source_refs: view.source_refs,
      }
    : null;
  return {
    schema: "tos_philosophy_mcp_packet_v1",
    query,
    view_id: viewId,
    result_count: search.result_count,
    results: search.results,
    view: compact,
    counts: top.counts ?? {},
    runtime_projection_boundary: runtimeBoundary(top),
    authority_note: "Packets are access aids; ToS owns meaning and Neo4j/UI/MCP remain projections.",
  };
}

export async function buildHealth(db: D1Database): Promise<Item> {
  const revision = await meta<{ sha256: string }>(db, "data_revision");
  return {
    service: "tree-of-sophia-access",
    ok: true,
    write_enabled: false,
    errors: [],
    runtime: "cloudflare-worker",
    data_revision: revision.sha256,
  };
}
