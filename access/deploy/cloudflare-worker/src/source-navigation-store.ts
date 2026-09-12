import { stringArray, stringValue, type Item } from "./common.ts";
import { SourceNavigationError } from "./source-navigation.ts";
import { metaItem } from "./store.ts";

/*
 * Source navigation is stored as a row projection in D1.  The JSON payload
 * remains the owned navigation record; the scalar columns are only indexes
 * for bounded selection.  Keep this adapter separate from source-navigation
 * so the pure implementation remains useful for offline parity tests.
 */

type SourceJsonRow = { json: string };
type SourceNodeRow = SourceJsonRow & {
  node_id: string;
  node_kind: string;
  source_ref: string;
  label: string;
  identity_status: string;
  properties_json: string;
};
type SourceEdgeRow = SourceJsonRow & {
  edge_id: string;
  from_id: string;
  to_id: string;
  edge_kind: string;
  predicate_id: string;
  review_status: string;
  source_refs_json: string;
};
type SourceRightRow = SourceJsonRow & { rights_id: string; scope_refs_json: string };

const SOURCE_NODE_TABLE = "source_navigation_nodes";
const SOURCE_NODE_PAYLOAD_TABLE = "source_navigation_node_payload";
const SOURCE_EDGE_TABLE = "source_navigation_edges";
const SOURCE_EDGE_PAYLOAD_TABLE = "source_navigation_edge_payload";
const SOURCE_RIGHT_TABLE = "source_navigation_rights";
const SOURCE_RIGHT_PAYLOAD_TABLE = "source_navigation_rights_payload";

// One response is bounded to 300 source nodes.  A page is deliberately
// smaller than that bound so a high-degree node can never make one D1 `.all()`
// call unbounded.  We still walk every page: the pure contract returns every
// edge whose endpoints are admitted, and silently dropping a later page would
// make both the edge packet and dossier closure misleading.
export const SOURCE_NAVIGATION_PAGE_SIZE = 100;

const BIBLIOGRAPHIC_PREDICATES = ["has_expression", "embodied_by", "exemplified_by"] as const;
const LINK_PREDICATES = ["described_by", "metadata_at", "downloadable_at", "rights_statement_at"] as const;
const CHAIN_KINDS = [
  "branch",
  "era",
  "region",
  "tradition",
  "source_planting",
  "work",
  "expression",
  "edition",
  "item",
  "file",
  "link",
] as const;

function parseRow<T extends SourceJsonRow>(row: T, payload?: string): Item {
  const value: unknown = JSON.parse(payload ?? row.json);
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("source-navigation D1 row is not a JSON object");
  }
  return value as Item;
}

async function payloadFor(
  db: D1Database,
  table: string,
  id: string,
): Promise<string> {
  const rows = await db
    .prepare(`SELECT part, json_chunk FROM ${table} WHERE id = ? ORDER BY part`)
    .bind(id)
    .all<{ part: number; json_chunk: string }>();
  if (rows.results.length === 0) throw new Error(`source-navigation row ${id} has no payload`);
  for (const [index, row] of rows.results.entries()) {
    if (row.part !== index || typeof row.json_chunk !== "string") {
      throw new Error(`source-navigation row ${id} has incomplete payload chunks`);
    }
  }
  return rows.results.map((row) => row.json_chunk).join("");
}

async function parseHydrated<T extends SourceJsonRow>(db: D1Database, row: T, table: string, id: string): Promise<Item> {
  return parseRow(row, row.json === "" ? await payloadFor(db, table, id) : undefined);
}

function sortedById(items: Item[], key: string): Item[] {
  return [...items].sort((left, right) => stringValue(left[key]).localeCompare(stringValue(right[key])));
}

function intersects(values: unknown, selected: Set<string>): boolean {
  return stringArray(values).some((value) => selected.has(value));
}

function pageSize(limit: number): number {
  return Math.max(1, Math.min(SOURCE_NAVIGATION_PAGE_SIZE, limit));
}

async function sourceNavigationHeader(db: D1Database): Promise<Item> {
  // `source_navigation_top` is the D1 metadata key.  The second spelling is
  // accepted for compiled snapshots that use the portable query-store name;
  // neither path loads the navigation collections.
  let header: Item;
  try {
    header = await metaItem(db, "source_navigation_top");
  } catch (firstError) {
    try {
      header = await metaItem(db, "source_navigation_header");
    } catch {
      throw firstError;
    }
  }
  if (header.schema_version !== "tos_source_navigation_v1") {
    throw new Error("ToS source-navigation metadata has an unsupported schema_version");
  }
  return header;
}

async function sourceNode(db: D1Database, id: string): Promise<Item | null> {
  const row = await db
    .prepare(`SELECT node_id, node_kind, source_ref, label, identity_status, properties_json, json
                FROM ${SOURCE_NODE_TABLE} WHERE node_id = ?`)
    .bind(id)
    .first<SourceNodeRow>();
  if (!row) return null;
  const node = await parseHydrated(db, row, SOURCE_NODE_PAYLOAD_TABLE, row.node_id);
  for (const field of ["node_kind", "source_ref", "label", "identity_status"] as const) {
    if (stringValue(node[field]) !== row[field]) throw new Error(`source-navigation node selection mismatch: ${row.node_id}`);
  }
  // `properties_json` is an indexed selection hint and is intentionally empty
  // for rows whose complete JSON would breach the D1 row budget.  In that
  // case the hydrated record is the authority for packet filtering.
  let properties: Item;
  if (row.properties_json === "") {
    properties = node.properties && typeof node.properties === "object" && !Array.isArray(node.properties)
      ? node.properties as Item
      : {};
  } else {
    const parsed: unknown = JSON.parse(row.properties_json);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error(`source-navigation node selection is invalid: ${row.node_id}`);
    }
    const hint = parsedObject(parsed);
    properties = hint;
    const fullProperties = node.properties && typeof node.properties === "object" && !Array.isArray(node.properties)
      ? node.properties as Item
      : {};
    for (const key of ["packet_id", "access_status"] as const) {
      const hasHint = Object.prototype.hasOwnProperty.call(hint, key);
      const hasFull = Object.prototype.hasOwnProperty.call(fullProperties, key);
      if (hasHint !== hasFull || (hasHint && JSON.stringify(hint[key]) !== JSON.stringify(fullProperties[key]))) {
        throw new Error(`source-navigation node selection mismatch: ${row.node_id}`);
      }
    }
  }
  return stringValue(properties.packet_id) ? null : node;
}

function parsedObject(value: unknown): Item {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

type EdgeDirection = "incoming" | "outgoing";

function semanticClause(): { sql: string; values: string[] } {
  const predicates = [...BIBLIOGRAPHIC_PREDICATES, ...LINK_PREDICATES];
  return {
    sql: `(e.edge_kind = 'authored_item_manifest'
            OR (e.edge_kind = 'evidence_claim' AND e.predicate_id IN (${predicates.map(() => "?").join(", ")})))`,
    values: predicates,
  };
}

type EdgePageRow = { id: string; item: Item };

async function sourceEdgePage(
  db: D1Database,
  direction: EdgeDirection,
  nodeId: string,
  semantic: boolean,
  cursor: string | null,
  limit: number,
): Promise<EdgePageRow[]> {
  const endpoint = direction === "outgoing" ? "e.from_id" : "e.to_id";
  const other = direction === "outgoing" ? "e.to_id" : "e.from_id";
  const keyset = cursor === null ? "" : " AND e.edge_id > ?";
  const semanticFilter = semantic ? ` AND ${semanticClause().sql}` : "";
  const values: unknown[] = [nodeId];
  if (cursor !== null) values.push(cursor);
  if (semantic) values.push(...semanticClause().values);
  values.push(limit);
  const rows = await db
    .prepare(`SELECT e.edge_id, e.from_id, e.to_id, e.edge_kind, e.predicate_id,
                     e.review_status, e.source_refs_json, e.json
                FROM ${SOURCE_EDGE_TABLE} e
                JOIN ${SOURCE_NODE_TABLE} other_node
                  ON other_node.node_id = ${other}
               WHERE ${endpoint} = ?${keyset}${semanticFilter}
               ORDER BY e.edge_id
               LIMIT ?`)
    .bind(...values)
    .all<SourceEdgeRow>();
  const result: EdgePageRow[] = [];
  for (const row of rows.results) {
    const item = await parseHydrated(db, row, SOURCE_EDGE_PAYLOAD_TABLE, row.edge_id);
    for (const field of ["edge_id", "from_id", "to_id", "edge_kind", "predicate_id", "review_status"] as const) {
      if (stringValue(item[field]) !== row[field]) throw new Error(`source-navigation edge selection mismatch: ${row.edge_id}`);
    }
    if (row.source_refs_json !== "" && JSON.stringify(item.source_refs ?? []) !== row.source_refs_json) {
      throw new Error(`source-navigation edge selection mismatch: ${row.edge_id}`);
    }
    result.push({ id: row.edge_id, item });
  }
  return result;
}

async function sourceEdges(
  db: D1Database,
  direction: EdgeDirection,
  nodeId: string,
  semantic = false,
  limit = SOURCE_NAVIGATION_PAGE_SIZE,
): Promise<Item[]> {
  const result: Item[] = [];
  const size = pageSize(limit);
  let cursor: string | null = null;
  while (true) {
    const page = await sourceEdgePage(db, direction, nodeId, semantic, cursor, size);
    result.push(...page.map((row) => row.item));
    if (page.length < size) break;
    const next = page[page.length - 1]?.id ?? "";
    // The authored projection requires a stable nonempty edge_id.  Failing
    // closed here prevents an invalid row from causing an endless keyset loop
    // while still keeping every valid page bounded.
    if (!next || next === cursor) throw new Error("source-navigation edge has no stable edge_id");
    cursor = next;
  }
  // D1's binary text collation and JS localeCompare are not interchangeable
  // for arbitrary Unicode IDs.  Pages are bounded, while this final ordering
  // preserves the source-navigation engine's established deterministic order.
  return sortedById(result, "edge_id");
}

async function sourceRights(db: D1Database, ids: Iterable<string>, limit: number): Promise<Item[]> {
  const values = [...new Set(ids)].filter(Boolean);
  if (values.length === 0) return [];
  const size = pageSize(limit);
  const result: Item[] = [];
  let cursor: string | null = null;
  const encodedIds = JSON.stringify(values);
  while (true) {
    const keyset: string = cursor === null ? "" : " AND r.rights_id > ?";
    const bindings: unknown[] = [encodedIds];
    if (cursor !== null) bindings.push(cursor);
    bindings.push(size);
    const rows: D1Result<SourceRightRow> = await db
      .prepare(`SELECT r.rights_id, r.scope_refs_json, r.json
                  FROM ${SOURCE_RIGHT_TABLE} r
                 WHERE (r.scope_refs_json = '' OR EXISTS (
                   SELECT 1 FROM json_each(r.scope_refs_json) scope
                    WHERE scope.value IN (SELECT value FROM json_each(?))
                 ))${keyset}
                 ORDER BY r.rights_id
                 LIMIT ?`)
      .bind(...bindings)
      .all<SourceRightRow>();
    for (const row of rows.results) {
      const item = await parseHydrated(db, row, SOURCE_RIGHT_PAYLOAD_TABLE, row.rights_id);
      if (stringValue(item.rights_id) !== row.rights_id) throw new Error(`source-navigation rights selection mismatch: ${row.rights_id}`);
      if (row.scope_refs_json !== "" && JSON.stringify(item.scope_refs ?? []) !== row.scope_refs_json) {
        throw new Error(`source-navigation rights selection mismatch: ${row.rights_id}`);
      }
      result.push(item);
    }
    if (rows.results.length < size) break;
    const next: string = rows.results[rows.results.length - 1]?.rights_id ?? "";
    if (!next || next === cursor) throw new Error("source-navigation rights row has no stable rights_id");
    cursor = next;
  }
  return sortedById(result, "rights_id");
}

/** Execute the source descent route against indexed D1 rows. */
export async function sourceDescendD1(
  db: D1Database,
  nodeId: string,
  maxDepth: number,
  limit: number,
): Promise<Item> {
  const navigation = await sourceNavigationHeader(db);
  const root = await sourceNode(db, nodeId);
  if (!root) throw new SourceNavigationError(404, `unknown ToS source-navigation node: ${nodeId}`);

  const nodesById = new Map<string, Item>([[nodeId, root]]);
  const nodeCache = new Map<string, Item | null>([[nodeId, root]]);
  const queue: Array<[string, number]> = [[nodeId, 0]];
  const depths = new Map<string, number>([[nodeId, 0]]);
  const selectedEdges: Item[] = [];
  const selectedEdgeIds = new Set<string>();
  let truncated = false;
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const [current, depth] = queue[cursor]!;
    if (depth >= maxDepth) continue;
    for (const edge of await sourceEdges(db, "outgoing", current, false, limit)) {
      const target = stringValue(edge.to_id);
      if (!target) continue;
      if (!nodesById.has(target)) {
        let targetNode = nodeCache.get(target);
        if (!nodeCache.has(target)) {
          targetNode = await sourceNode(db, target);
          nodeCache.set(target, targetNode);
        }
        if (!targetNode) continue;
        if (depths.size >= limit) {
          truncated = true;
          continue;
        }
        nodesById.set(target, targetNode);
      }
      const edgeId = stringValue(edge.edge_id);
      if (!selectedEdgeIds.has(edgeId)) {
        selectedEdgeIds.add(edgeId);
        selectedEdges.push(edge);
      }
      if (!depths.has(target)) {
        depths.set(target, depth + 1);
        queue.push([target, depth + 1]);
      }
    }
  }
  const selectedNodes = [...depths.entries()]
    .sort(([leftId, leftDepth], [rightId, rightDepth]) => leftDepth - rightDepth || leftId.localeCompare(rightId))
    .map(([id, depth]) => ({ ...nodesById.get(id), depth }));
  return {
    schema: "tos_source_descent_v1",
    root_id: nodeId,
    max_depth: maxDepth,
    limit,
    truncated,
    counts: { nodes: selectedNodes.length, edges: selectedEdges.length },
    nodes: selectedNodes,
    edges: selectedEdges,
    authority_note: navigation.authority_boundary,
  };
}

/** Execute the Work/Link dossier route against indexed D1 rows. */
export async function sourceDossierD1(db: D1Database, objectId: string, limit: number): Promise<Item> {
  const navigation = await sourceNavigationHeader(db);
  const selected = await sourceNode(db, objectId);
  if (!selected) throw new SourceNavigationError(404, `unknown ToS dossier object: ${objectId}`);
  const selectedKind = stringValue(selected.node_kind);
  if (selectedKind !== "work" && selectedKind !== "link") {
    throw new SourceNavigationError(400, "dossiers are currently available for Work and Link objects");
  }

  const nodesById = new Map<string, Item>([[objectId, selected]]);
  const nodeCache = new Map<string, Item | null>([[objectId, selected]]);
  const incomingCache = new Map<string, Item[]>();
  const semanticOutgoingCache = new Map<string, Item[]>();
  const incoming = async (id: string): Promise<Item[]> => {
    const cached = incomingCache.get(id);
    if (cached) return cached;
    const edges = await sourceEdges(db, "incoming", id, false, limit);
    incomingCache.set(id, edges);
    return edges;
  };
  const semanticOutgoing = async (id: string): Promise<Item[]> => {
    const cached = semanticOutgoingCache.get(id);
    if (cached) return cached;
    const edges = await sourceEdges(db, "outgoing", id, true, limit);
    semanticOutgoingCache.set(id, edges);
    return edges;
  };
  const loadNode = async (id: string): Promise<Item | null> => {
    if (nodeCache.has(id)) return nodeCache.get(id) ?? null;
    const node = await sourceNode(db, id);
    nodeCache.set(id, node);
    return node;
  };

  const componentIds = new Set<string>([objectId]);
  const componentEdges = new Map<string, Item>();
  let truncated = false;
  const admit = async (nodeId: string): Promise<boolean> => {
    if (componentIds.has(nodeId)) return true;
    const node = await loadNode(nodeId);
    if (!node) return false;
    if (componentIds.size >= limit) {
      truncated = true;
      return false;
    }
    componentIds.add(nodeId);
    nodesById.set(nodeId, node);
    return true;
  };

  let forwardRoots = new Set<string>(selectedKind === "work" ? [objectId] : []);
  if (selectedKind === "link") {
    const lineageQueue = [objectId];
    const visitedLineage = new Set<string>();
    for (let cursor = 0; cursor < lineageQueue.length; cursor += 1) {
      const current = lineageQueue[cursor]!;
      if (visitedLineage.has(current)) continue;
      visitedLineage.add(current);
      const currentKind = stringValue(nodesById.get(current)?.node_kind);
      if (currentKind === "work") {
        forwardRoots.add(current);
        continue;
      }
      const allowed = currentKind === "link" ? new Set<string>(LINK_PREDICATES) : new Set<string>(BIBLIOGRAPHIC_PREDICATES);
      for (const edge of await incoming(current)) {
        if (edge.edge_kind !== "evidence_claim" || !allowed.has(stringValue(edge.predicate_id))) continue;
        const parent = stringValue(edge.from_id);
        if (!await admit(parent)) continue;
        componentEdges.set(stringValue(edge.edge_id), edge);
        lineageQueue.push(parent);
      }
    }
    if (forwardRoots.size === 0) {
      forwardRoots = new Set([...componentIds].filter((id) => stringValue(nodesById.get(id)?.node_kind) !== "link"));
    }
  }

  const forwardQueue = [...forwardRoots].sort();
  const visitedForward = new Set<string>();
  for (let cursor = 0; cursor < forwardQueue.length; cursor += 1) {
    const current = forwardQueue[cursor]!;
    if (visitedForward.has(current)) continue;
    visitedForward.add(current);
    for (const edge of await semanticOutgoing(current)) {
      const target = stringValue(edge.to_id);
      if (!await admit(target)) continue;
      componentEdges.set(stringValue(edge.edge_id), edge);
      forwardQueue.push(target);
    }
  }

  const ancestorQueue: string[] = [];
  const workIds = [...componentIds]
    .filter((id) => stringValue(nodesById.get(id)?.node_kind) === "work")
    .sort();
  for (const workId of workIds) {
    for (const edge of await incoming(workId)) {
      if (edge.edge_kind !== "authored_source_planting") continue;
      const parent = stringValue(edge.from_id);
      if (!await admit(parent)) continue;
      componentEdges.set(stringValue(edge.edge_id), edge);
      ancestorQueue.push(parent);
    }
  }

  const visitedAncestors = new Set<string>();
  for (let cursor = 0; cursor < ancestorQueue.length; cursor += 1) {
    const current = ancestorQueue[cursor]!;
    if (visitedAncestors.has(current)) continue;
    visitedAncestors.add(current);
    const currentKind = stringValue(nodesById.get(current)?.node_kind);
    for (const edge of await incoming(current)) {
      const isBranchParent = edge.edge_kind === "authored_branch_hierarchy";
      const isPlantingParent = currentKind === "source_planting"
        && edge.edge_kind === "authored_source_planting"
        && edge.predicate_id === "has_source_planting";
      if (!isBranchParent && !isPlantingParent) continue;
      const parent = stringValue(edge.from_id);
      if (!await admit(parent)) continue;
      componentEdges.set(stringValue(edge.edge_id), edge);
      ancestorQueue.push(parent);
    }
  }

  const componentNodes = [...componentIds].sort().map((id) => nodesById.get(id) as Item);
  const chain: Record<string, Item[]> = {};
  for (const kind of CHAIN_KINDS) chain[kind] = componentNodes.filter((node) => node.node_kind === kind);
  const componentOutgoing = new Map<string, Item[]>();
  for (const edge of componentEdges.values()) {
    const bucket = componentOutgoing.get(stringValue(edge.from_id)) ?? [];
    bucket.push(edge);
    componentOutgoing.set(stringValue(edge.from_id), bucket);
  }
  const treePaths: Item[] = [];
  for (const era of chain.era ?? []) {
    const eraId = stringValue(era.node_id);
    const frontier: Array<[string, string[], string[]]> = [[eraId, [eraId], []]];
    const seen = new Set([eraId]);
    for (let cursor = 0; cursor < frontier.length; cursor += 1) {
      const [current, nodePath, edgePath] = frontier[cursor]!;
      if (current === objectId) {
        treePaths.push({ node_ids: nodePath, edge_ids: edgePath });
        break;
      }
      for (const edge of sortedById(componentOutgoing.get(current) ?? [], "edge_id")) {
        const target = stringValue(edge.to_id);
        if (!target || seen.has(target)) continue;
        seen.add(target);
        frontier.push([target, [...nodePath, target], [...edgePath, stringValue(edge.edge_id)] ]);
      }
    }
  }

  const rights = (await sourceRights(db, componentIds, limit))
    .filter((record) => intersects(record.scope_refs, componentIds));
  let decisionScopeIds = new Set([objectId]);
  if (selectedKind === "link") {
    decisionScopeIds = new Set(
      [...componentEdges.values()]
        .filter((edge) => edge.to_id === objectId && edge.edge_kind === "evidence_claim")
        .map((edge) => stringValue(edge.from_id)),
    );
  }
  const decisionRights = rights.filter((record) => intersects(record.scope_refs, decisionScopeIds));
  const dossierLinks = selectedKind === "work" ? (chain.link ?? []) : [selected];
  const linkStatuses = new Set(dossierLinks.map((node) => stringValue((node.properties as Item | undefined)?.access_status) || "unknown"));
  let technicalAccess = "unknown";
  if (linkStatuses.has("open_download")) technicalAccess = "downloadable";
  else if (linkStatuses.has("open_view")) technicalAccess = "viewable";
  else if (linkStatuses.has("metadata_only")) technicalAccess = "metadata_only";
  else if (["restricted", "login_required", "unavailable"].some((status) => linkStatuses.has(status))) {
    technicalAccess = "restricted_or_unavailable";
  }

  const positiveRights = decisionRights.filter((record) =>
    ["licensed", "public_domain_reviewed"].includes(stringValue(record.assessment_status))
    && ["authorized", "authorized_with_conditions"].includes(stringValue(record.redistribution_posture)),
  );
  const reviewedPositive = positiveRights.filter((record) =>
    ["accepted", "accepted_with_limits"].includes(stringValue(record.review_status)),
  );
  const rightsPosture = reviewedPositive.length > 0
    ? "reviewed_reuse_route"
    : positiveRights.length > 0
      ? "candidate_requires_human_review"
      : decisionRights.length > 0
        ? "not_cleared"
        : "unknown";
  const gaps: string[] = [];
  if (decisionRights.length === 0) gaps.push("no associated public rights record");
  if (positiveRights.length > 0 && reviewedPositive.length === 0) gaps.push("positive rights route exists but has no accepted human review");
  if ((chain.link ?? []).length === 0) gaps.push("no first-class associated Link record");

  const sourceRefSet = new Set<string>();
  for (const node of componentNodes) {
    const sourceRef = stringValue(node.source_ref);
    if (sourceRef) sourceRefSet.add(sourceRef);
  }
  for (const edge of componentEdges.values()) {
    for (const sourceRef of stringArray(edge.source_refs)) sourceRefSet.add(sourceRef);
  }
  for (const record of rights) {
    const sourceRef = stringValue(record.source_ref);
    if (sourceRef) sourceRefSet.add(sourceRef);
  }

  return {
    schema: "tos_source_dossier_v1",
    object_id: objectId,
    object: selected,
    agent_summary: {
      technical_access: technicalAccess,
      rights_posture: rightsPosture,
      human_review_required: reviewedPositive.length === 0,
      can_conclude_legal_openness: reviewedPositive.length > 0,
      availability_is_license: false,
      rights_scope_refs: [...decisionScopeIds].sort(),
      gaps,
    },
    chain,
    tree_paths: treePaths,
    relations: [...componentEdges.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([, edge]) => edge),
    rights: sortedById(rights, "rights_id"),
    source_refs: [...sourceRefSet].sort(),
    truncated,
    authority_note: navigation.authority_boundary,
  };
}
