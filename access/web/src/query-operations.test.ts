import { describe, expect, it } from "vitest";
import { createToSQueryOperations } from "./query-operations";

const searchCapabilities = (indexed: boolean, compressed: boolean) => ({
  schema: "tos_knowledge_search_capabilities_v1",
  modes: {
    indexed: { available: indexed },
    compressed: { available: compressed },
  },
});

describe("ToS query operations", () => {
  it("reads the projection fingerprint used by traceable local proposals", async () => {
    let requestedUrl = "";
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requestedUrl = url;
      return { snapshot_review: { current_snapshot: { projection_fingerprint: "abc" } } } as T;
    });

    const result = await operations.invoke("tos.snapshot");

    expect(requestedUrl).toBe("/api/philosophy/snapshot");
    expect(result).toMatchObject({ snapshot_review: { current_snapshot: { projection_fingerprint: "abc" } } });
  });

  it("forwards cancellation and all path constraints to the HTTP seam", async () => {
    const calls: Array<{ url: string; options?: RequestInit }> = [];
    const operations = createToSQueryOperations(async <T>(url: string, options?: RequestInit) => {
      calls.push({ url, options });
      return { found: true } as T;
    });
    const controller = new AbortController();

    await operations.invoke("tos.path.find", {
      from_id: "a",
      to_id: "b",
      direction: "outgoing",
      view_id: "chronology",
      excluded_edge_ids: ["edge:direct"],
      alternative_limit: 3,
      max_depth: 7,
    }, { signal: controller.signal });

    expect(calls).toHaveLength(1);
    expect(calls[0].options?.signal).toBe(controller.signal);
    const url = new URL(calls[0].url, "http://tos.local");
    expect(Object.fromEntries(url.searchParams)).toMatchObject({
      from: "a",
      to: "b",
      direction: "outgoing",
      view_id: "chronology",
      exclude: "edge:direct",
      alternatives: "3",
      max_depth: "7",
    });
  });

  it("preserves explicit empty filters as a no-match request", async () => {
    let requestedUrl = "";
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requestedUrl = url;
      return { found: false } as T;
    });

    await operations.invoke("tos.path.find", {
      from_id: "a",
      to_id: "b",
      layers: [],
      predicates: [],
    });

    const url = new URL(requestedUrl, "http://tos.local");
    expect(url.searchParams.get("direction")).toBe("outgoing");
    expect(url.searchParams.get("layers")).toBe("__tos_none__");
    expect(url.searchParams.get("predicates")).toBe("__tos_none__");
  });

  it("routes epistemic inspection through the selected item and active view", async () => {
    let requestedUrl = "";
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requestedUrl = url;
      return { schema: "tos_philosophy_epistemic_packet_v1" } as T;
    });

    await operations.invoke("tos.epistemic.inspect", {
      item_id: "candidate-node:table-i-a01-node-016",
      view_id: "source-evidence",
      limit: 48,
    });

    const url = new URL(requestedUrl, "http://tos.local");
    expect(url.pathname).toBe("/api/philosophy/query/epistemic/candidate-node%3Atable-i-a01-node-016");
    expect(Object.fromEntries(url.searchParams)).toEqual({ view_id: "source-evidence", limit: "48" });
  });

  it("routes Evidence Lens inspection to the corpus route graph", async () => {
    let requestedUrl = "";
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requestedUrl = url;
      return { schema: "tos_evidence_lens_packet_v1" } as T;
    });

    await operations.invoke("tos.epistemic.inspect", {
      mode: "corpus",
      item_id: "m113",
      view_id: "route-graph",
    });

    const url = new URL(requestedUrl, "http://tos.local");
    expect(url.pathname).toBe("/api/corpus/query/epistemic/m113");
    expect(url.searchParams.get("view_id")).toBe("route-graph");
  });

  it("routes source-bound Zarathustra word analysis through the local capability", async () => {
    let requestedUrl = "";
    const controller = new AbortController();
    const operations = createToSQueryOperations(async <T>(url: string, options?: RequestInit) => {
      requestedUrl = url;
      expect(options?.signal).toBe(controller.signal);
      return { available: true } as T;
    });

    await operations.invoke("tos.zarathustra.word-analysis.prepare", {
      query: "судьбы",
      language: "ru",
      rank: 2,
      include_semantic_neighbors: true,
    }, { signal: controller.signal });

    const url = new URL(requestedUrl, "http://tos.local");
    expect(url.pathname).toBe("/api/zarathustra/word-analysis");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      query: "судьбы",
      language: "ru",
      rank: "2",
      include_semantic_neighbors: "true",
    });
  });

  it("routes source descent and dossiers through their backend surfaces", async () => {
    const requested: string[] = [];
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requested.push(url);
      return { schema: "ok" } as T;
    });

    await operations.invoke("tos.source.descend", {
      node_id: "philosophy.eras.bronze-age",
      max_depth: 7,
      limit: 250,
    });
    await operations.invoke("tos.dossier.inspect", {
      object_id: "tos.link.cdli.cdlb-2006-1.pdf",
      limit: 80,
    });

    const descent = new URL(requested[0], "http://tos.local");
    expect(descent.pathname).toBe("/api/source/navigation/philosophy.eras.bronze-age");
    expect(Object.fromEntries(descent.searchParams)).toEqual({ max_depth: "7", limit: "250" });
    const dossier = new URL(requested[1], "http://tos.local");
    expect(dossier.pathname).toBe("/api/source/dossiers/tos.link.cdli.cdlb-2006-1.pdf");
    expect(Object.fromEntries(dossier.searchParams)).toEqual({ limit: "80" });
  });

  it("routes indexed knowledge search through its explicit cursor API", async () => {
    let requestedUrl = "";
    const controller = new AbortController();
    const operations = createToSQueryOperations(async <T>(url: string, options?: RequestInit) => {
      expect(options?.signal).toBe(controller.signal);
      if (url === "/api/knowledge/search/capabilities") return searchCapabilities(true, true) as T;
      requestedUrl = url;
      return { schema: "tos_knowledge_search_indexed_v2" } as T;
    });

    const result = await operations.invoke("tos.knowledge.search", {
      query: "fate",
      limit: 12,
      cursor: "next-cursor",
      sources: ["canon", "philosophy"],
      kind_ids: ["concept"],
      predicate_ids: ["relates"],
    }, { signal: controller.signal });

    expect(result).toMatchObject({ schema: "tos_knowledge_search_indexed_v2", search_mode: "indexed" });
    const url = new URL(requestedUrl, "http://tos.local");
    expect(url.pathname).toBe("/api/knowledge/search");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      mode: "indexed",
      query: "fate",
      limit: "12",
      cursor: "next-cursor",
      sources: "canon,philosophy",
      kind_ids: "concept",
      predicate_ids: "relates",
    });
  });

  it("selects compressed when indexed is unavailable and preserves opaque cursor, filters and page data", async () => {
    const cursor = "opaque.cursor+/=%20 α";
    const payload = {
      page: { items: [{ id: "node-1", rank: 1 }], next_cursor: cursor },
      source_revision: "source-revision",
      filters: { sources: ["canon"], kind_ids: ["concept"] },
    };
    let requestedUrl = "";
    const operations = createToSQueryOperations(async <T>(url: string) => {
      if (url === "/api/knowledge/search/capabilities") return searchCapabilities(false, true) as T;
      requestedUrl = url;
      return payload as T;
    });

    const result = await operations.invoke("tos.knowledge.search", {
      query: "fate",
      limit: 12,
      cursor,
      sources: ["canon"],
      kind_ids: ["concept"],
      predicate_ids: ["relates"],
    });

    expect(result).toEqual({ ...payload, search_mode: "compressed" });
    const url = new URL(requestedUrl, "http://tos.local");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      mode: "compressed",
      query: "fate",
      limit: "12",
      cursor,
      sources: "canon",
      kind_ids: "concept",
      predicate_ids: "relates",
    });
  });

  it("honors an explicit advertised compressed mode even when indexed is preferred by default", async () => {
    const requested: string[] = [];
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requested.push(url);
      if (url === "/api/knowledge/search/capabilities") return searchCapabilities(true, true) as T;
      return { page: { items: [], next_cursor: null } } as T;
    });

    const result = await operations.invoke("tos.knowledge.search", { query: "fate", search_mode: "compressed" });

    expect(result).toMatchObject({ search_mode: "compressed" });
    expect(new URL(requested[1], "http://tos.local").searchParams.get("mode")).toBe("compressed");
  });

  it("refuses invalid, unavailable and engine-less selections before any search request", async () => {
    const cases = [
      { input: { search_mode: "legacy" }, capabilities: searchCapabilities(true, true), error: "must be indexed or compressed" },
      { input: { search_mode: "compressed" }, capabilities: searchCapabilities(true, false), error: "mode unavailable: compressed" },
      { input: {}, capabilities: searchCapabilities(false, false), error: "indexed and compressed engines are unavailable" },
    ];

    for (const { input, capabilities, error } of cases) {
      const requested: string[] = [];
      const operations = createToSQueryOperations(async <T>(url: string) => {
        requested.push(url);
        if (url === "/api/knowledge/search/capabilities") return capabilities as T;
        throw new Error("search must not be called");
      });

      await expect(operations.invoke("tos.knowledge.search", input)).rejects.toThrow(error);
      expect(requested.filter((url) => new URL(url, "http://tos.local").pathname === "/api/knowledge/search")).toHaveLength(0);
    }
  });

  it("forwards the same abort signal to capability discovery and preserves abort errors", async () => {
    const controller = new AbortController();
    controller.abort();
    const abortError = new Error("aborted");
    const requested: string[] = [];
    const operations = createToSQueryOperations(async <T>(url: string, options?: RequestInit) => {
      requested.push(url);
      expect(options?.signal).toBe(controller.signal);
      if (options?.signal?.aborted) throw abortError;
      return searchCapabilities(true, true) as T;
    });

    await expect(operations.invoke("tos.knowledge.search", { query: "fate" }, { signal: controller.signal })).rejects.toBe(abortError);
    expect(requested).toEqual(["/api/knowledge/search/capabilities"]);
  });

  it("does not retry or switch engines after a selected search backend refuses", async () => {
    const backendError = new Error("backend refused");
    const requested: string[] = [];
    const operations = createToSQueryOperations(async <T>(url: string) => {
      requested.push(url);
      if (url === "/api/knowledge/search/capabilities") return searchCapabilities(true, true) as T;
      throw backendError;
    });

    await expect(operations.invoke("tos.knowledge.search", { query: "fate" })).rejects.toBe(backendError);
    expect(requested).toEqual(["/api/knowledge/search/capabilities", "/api/knowledge/search?mode=indexed&query=fate&limit=40"]);
  });
});
