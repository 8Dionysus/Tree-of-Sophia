import { describe, expect, it } from "vitest";
import { createToSQueryOperations } from "./query-operations";

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
});
