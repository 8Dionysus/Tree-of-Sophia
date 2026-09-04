import { HttpError, jsonResponse, listParam, parseItem, stringArray, type Item, withSecurity } from "./common";
import { maskFor, metaItem, positions, rows, viewMask } from "./store";

const TABLES = ["nodes", "edges", "clusters", "cluster-node-memberships", "cluster-edge-memberships"] as const;
const PAGE_SIZE = 200;
const SOURCE_PROJECTION_REF = "ToS/derived-exports/philosophy_graph_projection.min.json";

type ScaleTable = (typeof TABLES)[number];
type ScaleFormat = "jsonl" | "csv";
type Filter = {
  viewId: string | null;
  layers: string[];
  viewMask: number | null;
  layerMask: number | null;
};
type Sql = { text: string; bindings: unknown[] };
type JsonRow = { json: string };

function isTable(value: string): value is ScaleTable {
  return (TABLES as readonly string[]).includes(value);
}

async function resolveFilter(db: D1Database, search: URLSearchParams): Promise<Filter> {
  const viewId = (search.get("view_id") ?? "").trim() || null;
  const layers = [...new Set(listParam(search, "layers"))];
  const [selectedViewMask, layerPositions] = await Promise.all([
    viewMask(db, viewId),
    positions(db, "layer_positions"),
  ]);
  return {
    viewId,
    layers,
    viewMask: selectedViewMask,
    layerMask: maskFor(layers, layerPositions),
  };
}

function maskCondition(alias: string, filter: Filter): Sql {
  return {
    text: `(? = 0 OR (${alias}.view_mask & ?) != 0) AND (? = 0 OR (${alias}.layer_mask & ?) != 0)`,
    bindings: [filter.viewId ? 1 : 0, filter.viewMask ?? 0, filter.layers.length ? 1 : 0, filter.layerMask ?? 0],
  };
}

function nodeCondition(alias: string, filter: Filter): Sql {
  return maskCondition(alias, filter);
}

function edgeCondition(alias: string, filter: Filter): Sql {
  const edge = maskCondition(alias, filter);
  const left = nodeCondition("edge_left", filter);
  const right = nodeCondition("edge_right", filter);
  return {
    text: `${edge.text}
      AND EXISTS (
        SELECT 1 FROM philosophy_nodes edge_left
         WHERE edge_left.id = ${alias}.from_id AND ${left.text}
      )
      AND EXISTS (
        SELECT 1 FROM philosophy_nodes edge_right
         WHERE edge_right.id = ${alias}.to_id AND ${right.text}
      )`,
    bindings: [...edge.bindings, ...left.bindings, ...right.bindings],
  };
}

function membershipSelection(kind: "node" | "edge", filter: Filter, projection = "m.json"): Sql {
  const membershipTable = kind === "node" ? "philosophy_cluster_nodes" : "philosophy_cluster_edges";
  const itemTable = kind === "node" ? "philosophy_nodes" : "philosophy_edges";
  const membership = maskCondition("m", filter);
  const item = kind === "node" ? nodeCondition("i", filter) : edgeCondition("i", filter);
  return {
    text: `SELECT ${projection}
      FROM ${membershipTable} m
      JOIN ${itemTable} i ON i.id = m.item_id
     WHERE ${membership.text} AND ${item.text}`,
    bindings: [...membership.bindings, ...item.bindings],
  };
}

function selectedClusterIds(filter: Filter): Sql {
  const nodes = membershipSelection("node", filter, "m.cluster_id AS cluster_id");
  const edges = membershipSelection("edge", filter, "m.cluster_id AS cluster_id");
  return {
    text: `SELECT cluster_id FROM (${nodes.text} UNION ALL ${edges.text}) GROUP BY cluster_id`,
    bindings: [...nodes.bindings, ...edges.bindings],
  };
}

function assembledClusters(filter: Filter): Sql {
  const selected = selectedClusterIds(filter);
  return {
    text: `WITH selected_clusters AS (${selected.text})
      SELECT c.id, c.ord, c.sort_key, GROUP_CONCAT(c.json_chunk, '') AS json
        FROM philosophy_clusters c
        JOIN selected_clusters selected ON selected.cluster_id = c.id
       GROUP BY c.id, c.ord, c.sort_key`,
    bindings: selected.bindings,
  };
}

function tableSelection(table: Exclude<ScaleTable, "clusters">, filter: Filter): Sql {
  if (table === "nodes") {
    const condition = nodeCondition("n", filter);
    return {
      text: `SELECT n.json FROM philosophy_nodes n WHERE ${condition.text}`,
      bindings: condition.bindings,
    };
  }
  if (table === "edges") {
    const condition = edgeCondition("e", filter);
    return {
      text: `SELECT e.json FROM philosophy_edges e WHERE ${condition.text}`,
      bindings: condition.bindings,
    };
  }
  return membershipSelection(table === "cluster-node-memberships" ? "node" : "edge", filter);
}

function tableOrder(table: Exclude<ScaleTable, "clusters">, filter: Filter): string {
  if (table === "nodes" || table === "edges") return `${table === "nodes" ? "n" : "e"}.ord`;
  return `${filter.viewId ? "m.sort_key" : "m.cluster_ord"}, m.cluster_id, m.member_ord`;
}

async function membershipPage(
  db: D1Database,
  kind: "node" | "edge",
  filter: Filter,
  clusterIds: string[],
): Promise<Array<{ cluster_id: string; item_id: string }>> {
  if (clusterIds.length === 0) return [];
  const selected = membershipSelection(kind, filter, "m.cluster_id, m.item_id");
  return rows<{ cluster_id: string; item_id: string }>(
    db,
    `${selected.text}
      AND m.cluster_id IN (SELECT value FROM json_each(?))
      ORDER BY m.cluster_id, m.member_ord`,
    ...selected.bindings,
    JSON.stringify(clusterIds),
  );
}

async function clusterPage(db: D1Database, filter: Filter, offset: number, limit: number): Promise<Item[]> {
  const assembled = assembledClusters(filter);
  const order = filter.viewId ? "sort_key, id" : "ord";
  const raw = await rows<{ id: string; json: string }>(
    db,
    `${assembled.text} ORDER BY ${order} LIMIT ? OFFSET ?`,
    ...assembled.bindings,
    limit,
    offset,
  );
  const ids = raw.map((row) => row.id);
  const [nodeMemberships, edgeMemberships] = await Promise.all([
    membershipPage(db, "node", filter, ids),
    membershipPage(db, "edge", filter, ids),
  ]);
  const nodeIds = new Map<string, string[]>();
  const edgeIds = new Map<string, string[]>();
  for (const row of nodeMemberships) nodeIds.set(row.cluster_id, [...(nodeIds.get(row.cluster_id) ?? []), row.item_id]);
  for (const row of edgeMemberships) edgeIds.set(row.cluster_id, [...(edgeIds.get(row.cluster_id) ?? []), row.item_id]);

  return raw.map((row) => {
    const cluster = parseItem(row.json);
    const originalNodeIds = stringArray(cluster.member_node_ids);
    const originalEdgeIds = stringArray(cluster.member_edge_ids);
    const memberNodeIds = nodeIds.get(row.id) ?? [];
    const memberEdgeIds = edgeIds.get(row.id) ?? [];
    const properties = cluster.properties && typeof cluster.properties === "object" && !Array.isArray(cluster.properties)
      ? { ...(cluster.properties as Item) }
      : {};
    if (Object.prototype.hasOwnProperty.call(properties, "member_count")) properties.member_count = memberNodeIds.length;
    if (Object.prototype.hasOwnProperty.call(properties, "edge_count")) properties.edge_count = memberEdgeIds.length;
    return {
      ...cluster,
      member_node_ids: memberNodeIds,
      member_edge_ids: memberEdgeIds,
      available_member_node_count: originalNodeIds.length,
      available_member_edge_count: originalEdgeIds.length,
      properties,
    };
  });
}

async function rowPage(db: D1Database, table: ScaleTable, filter: Filter, offset: number, limit: number): Promise<Item[]> {
  if (table === "clusters") return clusterPage(db, filter, offset, limit);
  const selected = tableSelection(table, filter);
  const found = await rows<JsonRow>(
    db,
    `${selected.text} ORDER BY ${tableOrder(table, filter)} LIMIT ? OFFSET ?`,
    ...selected.bindings,
    limit,
    offset,
  );
  return found.map((row) => parseItem(row.json));
}

async function tableCount(db: D1Database, table: ScaleTable, filter: Filter): Promise<number> {
  let selected: Sql;
  if (table === "clusters") {
    selected = selectedClusterIds(filter);
  } else {
    selected = tableSelection(table, filter);
  }
  const row = await db.prepare(`SELECT COUNT(*) AS count FROM (${selected.text})`).bind(...selected.bindings).first<{ count: number }>();
  return Number(row?.count ?? 0);
}

async function csvColumns(db: D1Database, table: ScaleTable, filter: Filter): Promise<string[]> {
  let selected: Sql;
  if (table === "clusters") {
    const assembled = assembledClusters(filter);
    selected = { text: `SELECT json FROM (${assembled.text})`, bindings: assembled.bindings };
  } else {
    selected = tableSelection(table, filter);
  }
  const keys = await rows<{ key: string }>(
    db,
    `SELECT DISTINCT fields.key AS key FROM (${selected.text}) selected, json_each(selected.json) fields ORDER BY fields.key`,
    ...selected.bindings,
  );
  const result = new Set(keys.map((row) => row.key));
  if (table === "clusters") {
    result.add("available_member_edge_count");
    result.add("available_member_node_count");
  }
  return [...result].sort();
}

function csvCell(value: unknown): string {
  let text: string;
  if (value === null || value === undefined) text = "";
  else if (typeof value === "object") text = JSON.stringify(value);
  else if (typeof value === "boolean") text = value ? "True" : "False";
  else text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function encodeRow(row: Item, format: ScaleFormat, columns: string[]): string {
  if (format === "jsonl") return `${JSON.stringify(row)}\n`;
  return `${columns.map((column) => csvCell(row[column])).join(",")}\r\n`;
}

function exportStream(db: D1Database, table: ScaleTable, filter: Filter, format: ScaleFormat, columns: string[]): ReadableStream {
  const encoder = new TextEncoder();
  let offset = 0;
  let headerPending = format === "csv";
  return new ReadableStream({
    async pull(controller) {
      try {
        if (headerPending) {
          controller.enqueue(encoder.encode(`${columns.map(csvCell).join(",")}\r\n`));
          headerPending = false;
          return;
        }
        const page = await rowPage(db, table, filter, offset, PAGE_SIZE);
        if (page.length === 0) {
          controller.close();
          return;
        }
        controller.enqueue(encoder.encode(page.map((row) => encodeRow(row, format, columns)).join("")));
        offset += page.length;
        if (page.length < PAGE_SIZE) controller.close();
      } catch (error) {
        controller.error(error);
      }
    },
  });
}

export async function scaleManifest(db: D1Database, search: URLSearchParams): Promise<Item> {
  const filter = await resolveFilter(db, search);
  const counts: Record<ScaleTable, number> = {
    nodes: 0,
    edges: 0,
    clusters: 0,
    "cluster-node-memberships": 0,
    "cluster-edge-memberships": 0,
  };
  for (const table of TABLES) counts[table] = await tableCount(db, table, filter);
  const top = await metaItem(db, "philosophy_top");
  return {
    schema: "tos_philosophy_mcp_scale_manifest_v1",
    view_id: filter.viewId,
    layers: [...filter.layers].sort(),
    tables: Object.fromEntries(
      TABLES.map((table) => [
        table,
        {
          row_count: counts[table],
          packet_route: "tos_philosophy_graph_scale_rows",
          packet_route_args: { table, view_id: filter.viewId, layers: [...filter.layers].sort() },
        },
      ]),
    ),
    source_projection_ref: SOURCE_PROJECTION_REF,
    runtime_projection_boundary: top.runtime_projection_boundary ?? {},
    authority_note: "Scale manifests are MCP navigation packets; ToS derived exports remain authoritative.",
  };
}

export async function scaleExportResponse(request: Request, db: D1Database, url: URL): Promise<Response> {
  if (url.pathname === "/api/philosophy/scale-export/manifest") {
    return jsonResponse(await scaleManifest(db, url.searchParams), 200, request.method);
  }
  const match = /^\/api\/philosophy\/scale-export\/([^/]+)\.(jsonl|csv)$/.exec(url.pathname);
  const requestedTable = match?.[1] ?? "";
  if (!match || !isTable(requestedTable)) throw new HttpError(404, "unknown scale export format");
  const table = requestedTable;
  const format = match[2] as ScaleFormat;
  const filter = await resolveFilter(db, url.searchParams);
  const columns = format === "csv" && request.method !== "HEAD" ? await csvColumns(db, table, filter) : [];
  const headers = new Headers({
    "Cache-Control": "public, max-age=300, must-revalidate",
    "Content-Disposition": `attachment; filename="tos-philosophy-${table}.${format}"`,
    "Content-Type": format === "jsonl" ? "application/x-ndjson; charset=utf-8" : "text/csv; charset=utf-8",
  });
  const body = request.method === "HEAD" ? null : exportStream(db, table, filter, format, columns);
  return withSecurity(new Response(body, { status: 200, headers }));
}
