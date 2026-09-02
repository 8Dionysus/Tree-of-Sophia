import { describe, expect, it } from "vitest";
import { createToSQueryOperations } from "./query-operations";

describe("ToS query operations", () => {
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
});
