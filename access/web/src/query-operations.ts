import {chooseKnowledgeSearchMode} from "./knowledge-search.ts";

type WordQuerySession = {query(length: number): boolean; language(value: Uint16Array): boolean; number(value: number): number; free(): void};
type PathQuerySession = {required(length: number): boolean; depth(value: number): number; direction(value: Uint16Array): boolean; alternatives(value: number): number; filter_empty(length: number): boolean; empty_filter(): string; free(): void};
type QueryRequestRules = {mode(corpus: boolean): string; required(length: number): boolean; optional(length: number): boolean; cursor_action(tag: number): number; page_cursor_action(tag: number, length: number): number; page_direction(value: Uint16Array): boolean; search_mode_allowed(tag: number): boolean; filter_empty(length: number): boolean; empty_filter(): string; number(field: string, value: number): number};
let RequestRules: QueryRequestRules | undefined;
function requestRules(): QueryRequestRules {
  if (!RequestRules) throw new Error("Query request Rust runtime is required");
  return RequestRules;
}
let PathQueryRules: (new () => PathQuerySession) | undefined;
let WordQueryRules: (new () => WordQuerySession) | undefined;
export function installQueryRequestRules(runtime: {WordQuerySession?: new () => WordQuerySession; PathQuerySession?: new () => PathQuerySession; QueryRequestRules?: QueryRequestRules}) {
  if (!runtime.WordQuerySession) throw new Error("Word query Rust runtime is required");
  WordQueryRules = runtime.WordQuerySession;
  if (!runtime.PathQuerySession) throw new Error("Path query Rust runtime is required");
  PathQueryRules = runtime.PathQuerySession;
  if (!runtime.QueryRequestRules) throw new Error("Query request Rust runtime is required");
  RequestRules = runtime.QueryRequestRules;
}

export function pageIntegerRule(value: number, profile: string): number {
  return requestRules().number(profile, value);
}
export function pageCursorRule(tag: number, length: number): number {
  return requestRules().page_cursor_action(tag, length);
}
export function pageDirectionRule(value: Uint16Array): boolean {
  return requestRules().page_direction(value);
}

export type ToSMode = "philosophy" | "corpus";

export type ToSQueryOperationId =
  | "tos.status"
  | "tos.snapshot"
  | "tos.search"
  | "tos.knowledge.search"
  | "tos.source-gaps.search"
  | "tos.source.descend"
  | "tos.dossier.inspect"
  | "tos.view.open"
  | "tos.node.inspect"
  | "tos.neighborhood"
  | "tos.epistemic.inspect"
  | "tos.path.find"
  | "tos.zarathustra.word-analysis.prepare";

export type ToSQueryInput = Record<string, unknown>;
export type ToSQueryResult = Record<string, unknown>;
export type ToSQueryOptions = { signal?: AbortSignal };
export type FetchJson = <T>(url: string, options?: RequestInit) => Promise<T>;

function boundedInt(value: unknown, field: string): number {
  return requestRules().number(field, Number(value));
}

function requiredString(value: unknown, name: string): string {
  const result = String(value || "").trim();
  if (!requestRules().required(result.length)) throw new Error(`${name} is required`);
  return result;
}

function optionalString(value: unknown): string | undefined {
  const result = String(value || "").trim();
  return requestRules().optional(result.length) ? result : undefined;
}

function optionalOpaqueString(value: unknown): string | undefined {
  const tag = value === undefined || value === null ? 0 : typeof value === "string" ? 1 : 2;
  const action = requestRules().cursor_action(tag);
  if (action === 0) return undefined;
  if (action === 2) throw new Error("knowledge search cursor must be a string");
  return value as string;
}

function list(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String).filter(Boolean) : [];
}

function filterList(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const selected = list(value);
  return requestRules().filter_empty(selected.length) ? [requestRules().empty_filter()] : selected;
}

function params(values: Record<string, string | number | boolean | string[] | undefined>): string {
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

export function createToSQueryOperations(fetchJson: FetchJson) {
  const invoke = async (
    operationId: ToSQueryOperationId,
    input: ToSQueryInput = {},
    options: ToSQueryOptions = {},
  ): Promise<ToSQueryResult> => {
    const mode = requestRules().mode(input.mode === "corpus");
    const request = options.signal ? { signal: options.signal } : undefined;
    switch (operationId) {
      case "tos.status": {
        const [corpus, philosophy] = await Promise.all([
          fetchJson<ToSQueryResult>("/api/corpus/status", request),
          fetchJson<ToSQueryResult>("/api/philosophy/status", request),
        ]);
        return { corpus, philosophy };
      }
      case "tos.snapshot":
        return fetchJson<ToSQueryResult>("/api/philosophy/snapshot", request);
      case "tos.search": {
        const query = String(input.query || "").trim();
        const limit = boundedInt(input.limit, "search-limit");
        return fetchJson<ToSQueryResult>(`/api/${mode}/search${params({ query, limit })}`, request);
      }
      case "tos.knowledge.search": {
        const query = String(input.query || "").trim();
        const limit = boundedInt(input.limit, "knowledge-limit");
        const sources = Array.isArray(input.sources) ? list(input.sources) : undefined;
        const kindIds = Array.isArray(input.kind_ids) ? list(input.kind_ids) : undefined;
        const predicateIds = Array.isArray(input.predicate_ids) ? list(input.predicate_ids) : undefined;
        const requestedMode = input.search_mode;
        const requestedTag = requestedMode === undefined ? 0 : requestedMode === "indexed" ? 1 : requestedMode === "compressed" ? 2 : 3;
        if (!requestRules().search_mode_allowed(requestedTag)) {
          throw new Error(`knowledge search mode must be indexed or compressed: ${String(requestedMode)}`);
        }
        const capabilities = await fetchJson<unknown>("/api/knowledge/search/capabilities", request);
        const searchMode = chooseKnowledgeSearchMode(capabilities, requestedMode, query);
        const cursor = optionalOpaqueString(input.cursor);
        const payload = await fetchJson<ToSQueryResult>(`/api/knowledge/search${params({
          mode: searchMode,
          query,
          limit,
          sources,
          kind_ids: kindIds,
          predicate_ids: predicateIds,
          cursor,
        })}`, request);
        return { ...payload, search_mode: searchMode };
      }
      case "tos.source-gaps.search": {
        const query = String(input.query || "").trim();
        const limit = boundedInt(input.limit, "gaps-limit");
        return fetchJson<ToSQueryResult>(`/api/source-gaps${params({ query, limit })}`, request);
      }
      case "tos.source.descend": {
        const nodeId = requiredString(input.node_id, "node_id");
        return fetchJson<ToSQueryResult>(
          `/api/source/navigation/${encodeURIComponent(nodeId)}${params({
            max_depth: boundedInt(input.max_depth, "descent-depth"),
            limit: boundedInt(input.limit, "source-limit"),
          })}`,
          request,
        );
      }
      case "tos.dossier.inspect": {
        const objectId = requiredString(input.object_id, "object_id");
        return fetchJson<ToSQueryResult>(
          `/api/source/dossiers/${encodeURIComponent(objectId)}${params({
            limit: boundedInt(input.limit, "source-limit"),
          })}`,
          request,
        );
      }
      case "tos.view.open": {
        const viewId = requiredString(input.view_id, "view_id");
        const limit = boundedInt(input.limit, mode === "corpus" ? "view-corpus" : "view-philosophy");
        const route = mode === "corpus" ? "graph-views" : "views";
        return fetchJson<ToSQueryResult>(`/api/${mode}/${route}/${encodeURIComponent(viewId)}${params({ limit })}`, request);
      }
      case "tos.node.inspect": {
        const nodeId = requiredString(input.node_id, "node_id");
        return fetchJson<ToSQueryResult>(`/api/${mode}/nodes/${encodeURIComponent(nodeId)}`, request);
      }
      case "tos.neighborhood": {
        const nodeId = requiredString(input.node_id, "node_id");
        return fetchJson<ToSQueryResult>(
          `/api/philosophy/query/neighborhood/${encodeURIComponent(nodeId)}${params({
            depth: boundedInt(input.depth, "neighborhood-depth"),
            limit: boundedInt(input.limit, "neighborhood-limit"),
            layers: filterList(input.layers),
            predicates: filterList(input.predicates),
          })}`,
          request,
        );
      }
      case "tos.epistemic.inspect": {
        const itemId = requiredString(input.item_id, "item_id");
        return fetchJson<ToSQueryResult>(
          `/api/${mode}/query/epistemic/${encodeURIComponent(itemId)}${params({
            view_id: optionalString(input.view_id),
            limit: boundedInt(input.limit, "epistemic-limit"),
          })}`,
          request,
        );
      }
      case "tos.path.find": {
        if (!PathQueryRules) throw new Error("Path query Rust runtime is required");
        const rules = new PathQueryRules();
        let url: string;
        try {
          const from = String(input.from_id || "").trim();
          if (!rules.required(from.length)) throw new Error("from_id is required");
          const to = String(input.to_id || "").trim();
          if (!rules.required(to.length)) throw new Error("to_id is required");
          const maxDepth = rules.depth(Number(input.max_depth));
          const direction = String(input.direction || "outgoing").trim().toLowerCase();
          const units = new Uint16Array(direction.length);
          for (let i = 0; i < direction.length; i += 1) units[i] = direction.charCodeAt(i);
          if (!rules.direction(units)) throw new Error(`unsupported value: ${direction}`);
          const viewId = optionalString(input.view_id);
          const exclude = list(input.excluded_edge_ids);
          const alternatives = rules.alternatives(Number(input.alternative_limit));
          const filter = (value: unknown): string[] | undefined => {
            if (!Array.isArray(value)) return undefined;
            const selected = list(value);
            return rules.filter_empty(selected.length) ? [rules.empty_filter()] : selected;
          };
          url = `/api/philosophy/query/paths${params({from, to, max_depth: maxDepth,
            direction, view_id: viewId, exclude, alternatives,
            layers: filter(input.layers), predicates: filter(input.predicates)})}`;
        } finally { rules.free(); }
        return fetchJson<ToSQueryResult>(url, request);
      }
      case "tos.zarathustra.word-analysis.prepare": {
        if (!WordQueryRules) throw new Error("Word query Rust runtime is required");
        const rules = new WordQueryRules();
        let url: string;
        try {
          const query = String(input.query || "").trim();
          if (!rules.query(query.length)) throw new Error("query is required");
          const language = String(input.language || "ru").trim().toLowerCase();
          const units = new Uint16Array(language.length);
          for (let i = 0; i < language.length; i += 1) units[i] = language.charCodeAt(i);
          if (!rules.language(units)) throw new Error(`unsupported value: ${language}`);
          const rank = rules.number(Number(input.rank));
          url = `/api/zarathustra/word-analysis${params({query, language, rank,
            include_semantic_neighbors: input.include_semantic_neighbors === true})}`;
        } finally { rules.free(); }
        return fetchJson<ToSQueryResult>(url, request);
      }
    }
  };

  return { invoke };
}
