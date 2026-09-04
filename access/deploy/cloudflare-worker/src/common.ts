export type Item = Record<string, unknown>;

export class HttpError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export const SECURITY_HEADERS: Readonly<Record<string, string>> = {
  "Content-Security-Policy":
    "default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'; worker-src 'self'",
  "Permissions-Policy":
    "tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
  "Origin-Agent-Cluster": "?1",
  "Referrer-Policy": "no-referrer",
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
};

export function boundedInt(
  value: string | null,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isFinite(parsed) ? Math.max(minimum, Math.min(maximum, parsed)) : fallback;
}

export function listParam(search: URLSearchParams, key: string): string[] {
  return (search.get(key) ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.length > 0)
    : [];
}

export function objectArray(value: unknown): Item[] {
  return Array.isArray(value)
    ? value.filter((item): item is Item => Boolean(item) && typeof item === "object" && !Array.isArray(item))
    : [];
}

export function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function itemId(item: Item): string {
  return stringValue(item.node_id || item.edge_id || item.id);
}

export function parseItem(value: string): Item {
  const parsed: unknown = JSON.parse(value);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("edge read-model row is not a JSON object");
  }
  return parsed as Item;
}

export function sourceRefs(items: Item[]): string[] {
  const refs = new Set<string>();
  for (const item of items) {
    const sourceRef = stringValue(item.source_ref);
    if (sourceRef) refs.add(sourceRef);
    for (const ref of stringArray(item.source_refs)) refs.add(ref);
  }
  return [...refs].sort();
}

export function uniqueValues(items: Item[], key: string): string[] {
  return [...new Set(items.map((item) => stringValue(item[key])).filter(Boolean))].sort();
}

export function withSecurity(response: Response): Response {
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) headers.set(name, value);
  return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
}

export function jsonResponse(payload: unknown, status = 200, method = "GET"): Response {
  const headers = new Headers(SECURITY_HEADERS);
  headers.set("Content-Type", "application/json; charset=utf-8");
  headers.set("Cache-Control", "no-store");
  return new Response(method === "HEAD" ? null : JSON.stringify(payload), { status, headers });
}

export function matchesMask(rowMask: number, filterMask: number | null): boolean {
  return filterMask === null || (rowMask & filterMask) !== 0;
}

export function allowedPredicate(predicate: string, filters: string[]): boolean {
  return filters.length === 0 || filters.includes(predicate);
}

export function boundedGraph(nodes: Item[], edges: Item[], limit: number): { nodes: Item[]; edges: Item[] } {
  const nodesById = new Map(nodes.map((node) => [stringValue(node.node_id), node]));
  const selectedNodeIds = new Set<string>();
  const selectedEdges: Item[] = [];
  for (const edge of edges) {
    const left = stringValue(edge.from_id);
    const right = stringValue(edge.to_id);
    if (!nodesById.has(left) || !nodesById.has(right)) continue;
    const additions = new Set([left, right].filter((id) => !selectedNodeIds.has(id)));
    if (selectedNodeIds.size + additions.size > limit) continue;
    additions.forEach((id) => selectedNodeIds.add(id));
    selectedEdges.push(edge);
    if (selectedEdges.length >= limit) break;
  }
  for (const node of nodes) {
    if (selectedNodeIds.size >= limit) break;
    const id = stringValue(node.node_id);
    if (id) selectedNodeIds.add(id);
  }
  return {
    nodes: nodes.filter((node) => selectedNodeIds.has(stringValue(node.node_id))),
    edges: selectedEdges,
  };
}

export function boundedClusters(clusters: Item[], nodes: Item[], edges: Item[]): Item[] {
  const nodeIds = new Set(nodes.map((node) => stringValue(node.node_id)));
  const edgeIds = new Set(edges.map((edge) => stringValue(edge.edge_id)));
  const result: Item[] = [];
  for (const cluster of clusters) {
    const originalNodeIds = stringArray(cluster.member_node_ids);
    const originalEdgeIds = stringArray(cluster.member_edge_ids);
    const memberNodeIds = originalNodeIds.filter((id) => nodeIds.has(id));
    const memberEdgeIds = originalEdgeIds.filter((id) => edgeIds.has(id));
    if (memberNodeIds.length === 0 && memberEdgeIds.length === 0) continue;
    const properties =
      cluster.properties && typeof cluster.properties === "object" && !Array.isArray(cluster.properties)
        ? { ...(cluster.properties as Item) }
        : {};
    if ("member_count" in properties) properties.member_count = memberNodeIds.length;
    if ("edge_count" in properties) properties.edge_count = memberEdgeIds.length;
    result.push({
      ...cluster,
      member_node_ids: memberNodeIds,
      member_edge_ids: memberEdgeIds,
      available_member_node_count: originalNodeIds.length,
      available_member_edge_count: originalEdgeIds.length,
      properties,
    });
  }
  return result;
}
