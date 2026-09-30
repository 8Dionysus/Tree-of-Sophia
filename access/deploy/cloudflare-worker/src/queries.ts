import {PhysicalKeys,workerClassicRules,classicDistinct,classicNeedle,classicBoundedGraph as boundedGraph,classicBoundedClusters as boundedClusters,classicSourceRefs as sourceRefs,classicUniqueValues as uniqueValues} from "./worker-classic.ts";
import {
  HttpError,
  itemId,
  objectArray,
  stringArray,
  stringValue,
  type Item,
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
  const needle = classicNeedle(query);
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
  const needle = classicNeedle(query);
  const result: Item[] = [];
  const rules=new (workerClassicRules().WorkerSearch)(limit);
  const collections: Array<{ name: string; table: string; where: string }> = JSON.parse(workerClassicRules().WorkerProfile.search_collections());
  try { for (const collection of collections) {
    const remaining = rules.remaining();
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
    rules.observed(matches.length);
  }}finally{rules.free();}
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
  if (workerClassicRules().WorkerProfile.missing(matches.length) && relatedEdges.length > 0) {
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
  if (workerClassicRules().WorkerProfile.missing(matches.length)) throw new HttpError(404, `unknown ToS corpus node: ${nodeId}`);
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
  if (workerClassicRules().WorkerProfile.missing(packs.length)) throw new HttpError(404, `unknown ToS corpus relation pack: ${packId}`);
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
  const graphMode=workerClassicRules().WorkerEndpoints.graph_mode(viewId);
  const [views, top] = await Promise.all([
    corpusItem(db, "graph_views", viewId),
    metaItem(db, "corpus_top"),
  ]);
  const view = views[0];
  if (!view) throw new HttpError(404, `unknown ToS graph view: ${viewId}`);
  if (graphMode===3) throw new HttpError(404, `unsupported standalone ToS graph view: ${viewId}`);

  let items: Item[] = [];
  let graphNodes: Item[] = [];
  let graphEdges: Item[] = [];
  if (graphMode===0) {
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
    if (graphMode===1) {
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
    const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerEndpoints)();
    try {
      for(const edge of graphEdges){for(const endpoint of [stringValue(edge.from_id),stringValue(edge.to_id)])rules.endpoint(keys.key(endpoint),Boolean(endpoint));}
      const endpointIds=keys.strings(rules.ids());
      const indexedNodes=endpointIds.length?await jsonRows(db,
        "SELECT json FROM corpus_items WHERE collection = 'nodes' AND id IN (SELECT value FROM json_each(?)) ORDER BY ord",
        JSON.stringify(endpointIds)):[];
      if(graphMode===1){graphNodes=indexedNodes.filter(node=>rules.contains(keys.key(stringValue(node.node_id))));}
      else {
        const byId=new Map(indexedNodes.map(node=>[stringValue(node.node_id),node]));
        for(const edge of graphEdges){const ref=stringValue(edge.source_ref);if(!ref)continue;for(const endpoint of [stringValue(edge.from_id),stringValue(edge.to_id)])rules.reference(keys.key(endpoint),keys.key(ref),true);}
        graphNodes=endpointIds.sort().map(id=>byId.get(id)??({node_id:id,label:id,node_type:"candidate-endpoint",authority_layer:"candidate_intake",owner_branch:"ToS/candidate-intake",source_refs:keys.strings(rules.references(keys.key(id))).sort()}));
      }
    }finally{rules.free();}
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

  const edgeFetchLimit = workerClassicRules().WorkerProfile.edge_fetch_limit(limit);
  const [candidateNodes, candidateEdges, clusters, reviewRows] = await Promise.all([
    jsonRows(db, "SELECT json FROM philosophy_nodes WHERE (view_mask & ?) != 0 ORDER BY ord LIMIT ?", selectedViewMask, workerClassicRules().WorkerProfile.node_fetch_limit(limit)),
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
  const keys=new PhysicalKeys();
  const rules=new (workerClassicRules().WorkerNeighborhood)(keys.key(nodeId),depth,limit);
  const physicalEdges=new Map<number,Item>();
  let nextPhysicalEdge=0;
  let neighbors:Item[];
  let retainedEdges:Item[];
  try {
    while(rules.active()) {
      const frontier=keys.strings(rules.frontier());
      const edgeRows=await rows<EdgeRow>(db,
        `SELECT id, ord, from_id, to_id, predicate_id, view_mask, layer_mask, json
           FROM philosophy_edges
          WHERE (from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?)))
            AND (? IS NULL OR (layer_mask & ?) != 0) AND ${predicate.sql}
          ORDER BY ord`,
        JSON.stringify(frontier),JSON.stringify(frontier),layerMask,layerMask,
        ...(predicates.length===0?[]:[predicate.binding]));
      // Preserve lazy original-row decoding: Rust chooses the traversal rows.
      const base=nextPhysicalEdge;
      nextPhysicalEdge+=edgeRows.length;
      for(let index=0;index<edgeRows.length;index+=1){const edge=edgeRows[index]!;rules.observe_edge(keys.key(edge.id),keys.key(edge.from_id),keys.key(edge.to_id),base+index);}
      const allowedRows=await rows<{id:string;ord:number}>(db,
        `SELECT id, ord FROM philosophy_nodes WHERE id IN (SELECT value FROM json_each(?))
           AND (? IS NULL OR (layer_mask & ?) != 0) ORDER BY ord`,
        JSON.stringify(keys.strings(rules.candidates())),layerMask,layerMask);
      allowedRows.sort((left,right)=>left.ord-right.ord||left.id.localeCompare(right.id));
      rules.finish_level(keys.observe(allowedRows.map(row=>row.id)));
      for(const row of rules.edges()){if(row>=base)physicalEdges.set(row,rowJson(edgeRows[row-base]!));}
    }
    neighbors=await itemsByIds(db,"philosophy_nodes",keys.strings(rules.neighbors()));
    if(rules.enclosed_needed()){
      const selected=keys.strings(rules.selected());
      const enclosed=await jsonRows(db,
        `SELECT json FROM philosophy_edges WHERE from_id IN (SELECT value FROM json_each(?))
           AND to_id IN (SELECT value FROM json_each(?)) AND (? IS NULL OR (layer_mask & ?) != 0)
           AND ${predicate.sql} ORDER BY ord`,
        JSON.stringify(selected),JSON.stringify(selected),layerMask,layerMask,
        ...(predicates.length===0?[]:[predicate.binding]));
      for(const edge of enclosed){if(!rules.enclosed_needed())break;const index=nextPhysicalEdge++;physicalEdges.set(index,edge);rules.observe_enclosed(keys.key(stringValue(edge.edge_id)),index);}
    }
    retainedEdges=Array.from(rules.edges(),index=>physicalEdges.get(index) as Item);
  }finally{rules.free();}
  return {
    schema: "tos_philosophy_mcp_neighborhood_v1",
    node,
    neighbors,
    edges: retainedEdges,
    depth,
    layers: classicDistinct(layers).sort(),
    predicates: classicDistinct(predicates).sort(),
    limit,
    source_refs: sourceRefs([node, ...neighbors, ...retainedEdges]),
    runtime_projection_boundary: runtimeBoundary(top),
  };
}

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
  const directionCode=workerClassicRules().WorkerProfile.direction(direction);
  if (directionCode===3) {
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
    if (workerClassicRules().WorkerProfile.endpoints_missing(available.length,fromId===toId)) {
      return emptyPathPacket(options, top, 1, 1, false);
    }
  }
  const excluded = classicDistinct(options.excludedEdgeIds.filter(Boolean));
  const predicate = predicateSql(predicates);
  const keys=new PhysicalKeys();
  const rules=new (workerClassicRules().WorkerPath)(keys.key(fromId),keys.key(toId),maxDepth,
    directionCode,alternativeLimit,keys.observe([...excluded]));
  const completed:Array<{nodeIds:string[];edgeIds:string[];traversal:Item[]}>=[];
  let exploredStates:number,enqueuedStates:number,maxFrontierSize:number,explorationTruncated:boolean;
  try {
    while(rules.active()){
      const currentIds=keys.strings(rules.current_ids());
      const edgeRows=await rows<EdgeRow>(db,
        `SELECT id, ord, from_id, to_id, predicate_id, view_mask, layer_mask
           FROM philosophy_edges
          WHERE (from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?)))
            AND (? IS NULL OR (view_mask & ?) != 0) AND (? IS NULL OR (layer_mask & ?) != 0)
            AND ${predicate.sql} ORDER BY id, from_id, to_id`,
        JSON.stringify(currentIds),JSON.stringify(currentIds),selectedViewMask,selectedViewMask,
        layerMask,layerMask,...(predicates.length===0?[]:[predicate.binding]));
      for(const edge of edgeRows)rules.observe_edge(keys.key(edge.id),keys.key(edge.from_id),keys.key(edge.to_id));
      rules.finish_level();
    }
    for(let index=0;index<rules.path_count();index+=1){
      const nodeIds=keys.strings(rules.path_nodes(index));
      const edgeIds=keys.strings(rules.path_edges(index));
      const directions=rules.path_directions(index);
      completed.push({nodeIds,edgeIds,traversal:edgeIds.map((edge_id,step)=>({edge_id,from_node_id:nodeIds[step],to_node_id:nodeIds[step+1],edge_direction:directions[step]===0?"forward":"reverse"}))});
    }
    exploredStates=rules.explored();enqueuedStates=rules.enqueued();maxFrontierSize=rules.max_frontier();explorationTruncated=rules.truncated();
  }finally{rules.free();}
  const allNodeIds = classicDistinct(completed.flatMap((path) => path.nodeIds));
  const allEdgeIds = classicDistinct(completed.flatMap((path) => path.edgeIds));
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
    layers: classicDistinct(layers).sort(),
    predicates: classicDistinct(predicates).sort(),
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
    excluded_edge_ids: classicDistinct(options.excludedEdgeIds).sort(),
    alternative_limit: options.alternativeLimit,
    exploration_truncated: truncated,
    explored_state_count: exploredStates,
    enqueued_state_count: enqueuedStates,
    frontier_limit: PATH_FRONTIER_LIMIT,
    max_frontier_size: 1,
    layers: classicDistinct(options.layers).sort(),
    predicates: classicDistinct(options.predicates).sort(),
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
  if (selectedViewMask !== null && workerClassicRules().WorkerProfile.outside_view(selected.viewMask,selectedViewMask)) {
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
  const selectedEdgeIsContext = selected.kind === "edge" && !workerClassicRules().WorkerEpistemic.challenge(stringValue(selected.item.predicate_id));
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerEpistemic)(limit,selectedEdgeIsContext);
  let availableChallengeCount:number,challengeState:string,challengeRelations:Item[],contextRelations:Item[],neighborNodes:Item[];
  try {
    for(let index=0;index<candidates.length;index+=1){const edge=candidates[index]!;rules.observe_relation(keys.key(stringValue(edge.edge_id)),index,workerClassicRules().WorkerEpistemic.challenge(stringValue(edge.predicate_id)));}
    rules.finish_relations();
    challengeRelations=Array.from(rules.challenge_rows(),index=>candidates[index]!);
    contextRelations=Array.from(rules.context_rows(),index=>candidates[index]!);
    const endpoints=new (workerClassicRules().WorkerEndpoints)();
    let relatedNodeIds:string[];
    try{for(const edge of [...challengeRelations,...contextRelations])for(const id of [stringValue(edge.from_id),stringValue(edge.to_id)]){endpoints.endpoint(keys.key(id),Boolean(id));}relatedNodeIds=keys.strings(endpoints.ids());}finally{endpoints.free();}
    neighborNodes=(await itemsByIds(db,"philosophy_nodes",relatedNodeIds)).filter(node=>rules.neighbor(keys.key(stringValue(node.node_id)),selected.kind==="node",keys.key(itemIdValue)));
    availableChallengeCount=rules.available();challengeState=rules.challenge_state();
  }finally{rules.free();}
  const surrounding = [...challengeRelations, ...contextRelations, ...neighborNodes];
  const fieldKeys=new PhysicalKeys();
  const fieldSelection=fieldKeys.key(itemIdValue);
  const fieldItems = surrounding.filter((item) => workerClassicRules().WorkerProfile.different(fieldKeys.key(itemId(item)),fieldSelection));
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
      confidence_values: classicDistinct(fieldProperties.map(confidence).filter(Boolean)).sort(),
    },
    coverage: {
      posture: "partial",
      challenge_state: challengeState,
      available_challenge_relations: availableChallengeCount,
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
      available_challenge_relations: availableChallengeCount,
      context_relations: contextRelations.length,
      neighbor_nodes: neighborNodes.length,
      source_refs: sourceRefs([selected.item, ...surrounding]).length,
    },
    source_refs: sourceRefs([selected.item, ...surrounding]),
    challenge_predicates: JSON.parse(workerClassicRules().WorkerProfile.challenge_predicates()),
    runtime_projection_boundary: runtimeBoundary(top),
    authority_note:
      "This packet exposes projected challenge signals and source-return routes. A contested_by, uncertain_relation, or polemicizes_with candidate is not adjudicated counterevidence; ToS source, claim, review, rights, and canon owners remain authoritative.",
  };
}

async function corpusEpistemic(db: D1Database, itemIdValue: string, viewId: string | null, limit: number): Promise<Item> {
  const selectedView = viewId || "route-graph";
  if (!workerClassicRules().WorkerProfile.supported_corpus_evidence(selectedView)) throw new HttpError(404, "corpus Evidence Lens currently supports the route-graph view");
  const edges = await jsonRows(db, "SELECT json FROM corpus_edges WHERE owner_branch = 'ToS/canon' ORDER BY ord LIMIT 1000");
  const endpointIds = classicDistinct(edges.flatMap((edge) => [stringValue(edge.from_id), stringValue(edge.to_id)]).filter(Boolean));
  const nodes = await jsonRows(
    db,
    "SELECT json FROM corpus_items WHERE collection = 'nodes' AND id IN (SELECT value FROM json_each(?)) ORDER BY ord",
    JSON.stringify(endpointIds),
  );
  const selectionKeys=new PhysicalKeys();
  const selectedIdentity=selectionKeys.key(itemIdValue);
  const selection = [...nodes, ...edges].find((item) => workerClassicRules().WorkerProfile.same(selectionKeys.key(itemId(item)),selectedIdentity));
  if (!selection) throw new HttpError(404, `unknown ToS corpus route-graph item: ${itemIdValue}`);
  const selectionIsNode = Boolean(selection.node_id);
  const selectedNodeIds = selectionIsNode
    ? [itemIdValue]
    : [stringValue(selection.from_id), stringValue(selection.to_id)].filter(Boolean);
  const keys=new PhysicalKeys(),selectionKey=keys.key(itemIdValue),nodeKeys=keys.observe(selectedNodeIds);
  const relationCandidates=edges.filter(edge=>workerClassicRules().WorkerEpistemic.corpus_candidate(keys.key(stringValue(edge.edge_id)),selectionKey,keys.key(stringValue(edge.from_id)),keys.key(stringValue(edge.to_id)),nodeKeys))
    .sort((left,right)=>workerClassicRules().WorkerProfile.priority(itemId(left)===itemIdValue,itemId(right)===itemIdValue)||itemId(left).localeCompare(itemId(right)));
  const rules=new (workerClassicRules().WorkerEpistemic)(limit,false);
  let contextRelations:Item[],neighborNodes:Item[];
  try {
    for(let index=0;index<relationCandidates.length;index+=1)rules.observe_relation(keys.key(stringValue(relationCandidates[index]!.edge_id)),index,false);
    rules.finish_relations();contextRelations=Array.from(rules.context_rows(),index=>relationCandidates[index]!);
    for(const edge of contextRelations)for(const id of [stringValue(edge.from_id),stringValue(edge.to_id)])rules.observe_endpoint(keys.key(id),Boolean(id));
    neighborNodes=nodes.filter(node=>rules.corpus_neighbor(keys.key(stringValue(node.node_id)),selectionIsNode,selectionKey));
  }finally{rules.free();}

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
  const keys=new PhysicalKeys(),rules=new (workerClassicRules().WorkerEvidence)(mode==="philosophy"?0:1,keys.key(itemIdValue));
  const scenes=objectArray(evidence.scenes);
  let scene:Item|undefined;
  try {
    for(let index=0;index<scenes.length;index+=1){
      for(const route of objectArray(scenes[index]!.selections)){
        rules.route(index,stringValue(route.mode),keys.observe(stringArray(route.item_ids)));
        if(rules.scene()>=0)break;
      }
      if(rules.scene()>=0)break;
    }
    const sceneIndex=rules.scene();scene=sceneIndex<0?undefined:scenes[sceneIndex];
  const selection = context.selection as Item;
  let finding: string;
  let findingRu: string;
  let posture: string;
  let conclusion: Item;
  let routesValue: Item[];
  let gaps: string[];
  let gapsRu: string[];
  let sourceAnchors: Item[];
  if (!rules.curated()) {
    const fallback=JSON.parse(workerClassicRules().WorkerEvidence.fallback(workerClassicRules().WorkerEvidence.canon_layer(stringValue(selection.authority_layer)))) as Item;
    finding=fallback.finding as string;findingRu=fallback.finding_ru as string;posture=fallback.posture as string;
    conclusion=fallback.conclusion as Item;routesValue=[];
    gaps=fallback.gaps as string[];gapsRu=fallback.gaps_ru as string[];sourceAnchors=[];
  } else {
    finding = stringValue(scene!.finding);
    const observedFindingRu=stringValue(scene!.finding_ru);
    findingRu = workerClassicRules().WorkerEvidence.field_fallback(observedFindingRu.length)?finding:observedFindingRu;
    posture = stringValue(scene!.posture);
    const observedConclusion=scene!.conclusion;
    conclusion = workerClassicRules().WorkerEvidence.conclusion_record(Boolean(observedConclusion),typeof observedConclusion==="object")?(observedConclusion as Item):{};
    routesValue = objectArray(scene!.routes);
    gaps = stringArray(scene!.gaps);
    gapsRu = stringArray(scene!.gaps_ru);
    if (workerClassicRules().WorkerEvidence.field_fallback(gapsRu.length)) gapsRu = gaps;
    sourceAnchors = objectArray(scene!.source_anchors);
    const coverage = context.coverage as Item;
    coverage.missing_surfaces = gaps;
    coverage.posture = workerClassicRules().WorkerEvidence.coverage_posture();
  }
  const routeCounts: Record<string, number> = {};
  for (const route of routesValue) {
    const kind = stringValue(route.route_kind) || "other";
    const current=routeCounts[kind] ?? 0;
    routeCounts[kind] = typeof current==="number"?rules.count_route(current):current+1;
  }
  const projectionRefs = stringArray(context.source_refs);
  const lensSourceRefs = classicDistinct([...projectionRefs, ...stringArray(scene?.source_refs)]).sort();
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
  }finally{rules.free();}
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
  const [revision, knowledge] = await Promise.all([
    meta<{ sha256: string }>(db, "data_revision"),
    meta<Item>(db, "knowledge_top"),
  ]);
  if (!workerClassicRules().WorkerHealth.schema(stringValue(knowledge.schema))) throw new Error("knowledge read model schema is not current");
  const counts = knowledge.counts && typeof knowledge.counts === "object" && !Array.isArray(knowledge.counts)
    ? knowledge.counts as Item
    : {};
  const coverage = counts.display_coverage && typeof counts.display_coverage === "object" && !Array.isArray(counts.display_coverage)
    ? counts.display_coverage as Item
    : {};
  const expectedCoverage: Record<string, unknown> = {
    node_titles: counts.nodes,
    node_summaries: counts.nodes,
    relation_labels: counts.relations,
    relation_statements: counts.relations,
    relation_explanations: counts.relations,
  };
  if (Object.entries(expectedCoverage).some(([key, value]) => {if(!workerClassicRules().WorkerHealth.expected(typeof value==="number"))return true;const actual=coverage[key];return !workerClassicRules().WorkerHealth.coverage(value as number,typeof actual==="number"?actual:NaN,true);})) {
    throw new Error("knowledge read model display coverage is incomplete");
  }
  const healthCoverage = Object.fromEntries(
    Object.keys(expectedCoverage).map((key) => [key, coverage[key]]),
  );
  return {
    service: "tree-of-sophia-access",
    ok: true,
    write_enabled: false,
    errors: [],
    runtime: "cloudflare-worker",
    data_revision: revision.sha256,
    knowledge_schema: knowledge.schema,
    knowledge_counts: {
      nodes: counts.nodes,
      relations: counts.relations,
      display_coverage: healthCoverage,
    },
  };
}
