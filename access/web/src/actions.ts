export type ToSMode = "philosophy" | "corpus";

export type ToSActionId =
  | "tos.status"
  | "tos.search"
  | "tos.view.open"
  | "tos.node.inspect"
  | "tos.neighborhood"
  | "tos.path.find";

export type ToSActionInput = Record<string, unknown>;
export type ToSActionResult = Record<string, unknown>;
export type FetchJson = <T>(url: string, options?: RequestInit) => Promise<T>;

function boundedInt(value: unknown, fallback: number, minimum: number, maximum: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.max(minimum, Math.min(maximum, Math.trunc(parsed))) : fallback;
}

function requiredString(value: unknown, name: string): string {
  const result = String(value || "").trim();
  if (!result) throw new Error(`${name} is required`);
  return result;
}

function list(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String).filter(Boolean) : [];
}

function filterList(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const selected = list(value);
  return selected.length ? selected : ["__tos_none__"];
}

function params(values: Record<string, string | number | string[] | undefined>): string {
  const query = new URLSearchParams();
  Object.entries(values).forEach(([key, value]) => {
    if (Array.isArray(value)) {
      if (value.length) query.set(key, value.join(","));
    } else if (value !== undefined && value !== "") {
      query.set(key, String(value));
    }
  });
  const rendered = query.toString();
  return rendered ? `?${rendered}` : "";
}

export function createToSWebActions(fetchJson: FetchJson) {
  const invoke = async (actionId: ToSActionId, input: ToSActionInput = {}): Promise<ToSActionResult> => {
    const mode = input.mode === "corpus" ? "corpus" : "philosophy";
    switch (actionId) {
      case "tos.status": {
        const [corpus, philosophy] = await Promise.all([
          fetchJson<ToSActionResult>("/api/corpus/status"),
          fetchJson<ToSActionResult>("/api/philosophy/status"),
        ]);
        return { corpus, philosophy };
      }
      case "tos.search": {
        const query = String(input.query || "").trim();
        const limit = boundedInt(input.limit, 20, 1, 100);
        return fetchJson<ToSActionResult>(`/api/${mode}/search${params({ query, limit })}`);
      }
      case "tos.view.open": {
        const viewId = requiredString(input.view_id, "view_id");
        const limit = boundedInt(input.limit, mode === "corpus" ? 100 : 1000, 1, 1000);
        const route = mode === "corpus" ? "graph-views" : "views";
        return fetchJson<ToSActionResult>(`/api/${mode}/${route}/${encodeURIComponent(viewId)}${params({ limit })}`);
      }
      case "tos.node.inspect": {
        const nodeId = requiredString(input.node_id, "node_id");
        return fetchJson<ToSActionResult>(`/api/${mode}/nodes/${encodeURIComponent(nodeId)}`);
      }
      case "tos.neighborhood": {
        const nodeId = requiredString(input.node_id, "node_id");
        return fetchJson<ToSActionResult>(
          `/api/philosophy/query/neighborhood/${encodeURIComponent(nodeId)}${params({
            depth: boundedInt(input.depth, 1, 1, 3),
            limit: boundedInt(input.limit, 80, 1, 300),
            layers: filterList(input.layers),
            predicates: filterList(input.predicates),
          })}`,
        );
      }
      case "tos.path.find": {
        return fetchJson<ToSActionResult>(
          `/api/philosophy/query/paths${params({
            from: requiredString(input.from_id, "from_id"),
            to: requiredString(input.to_id, "to_id"),
            max_depth: boundedInt(input.max_depth, 6, 1, 8),
            layers: filterList(input.layers),
            predicates: filterList(input.predicates),
          })}`,
        );
      }
    }
  };

  return { invoke };
}
