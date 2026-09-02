import { describe, expect, it, vi } from "vitest";
import { createPageCommandRegistry, type PageContextSnapshot } from "./page-commands";
import { createWebMCPAdapter, type WebMCPDocument } from "./webmcp";

type RegisteredTool = {
  name: string;
  execute: (input: Record<string, unknown>, options: { signal: AbortSignal }) => Promise<unknown>;
};

describe("WebMCP page-command binding", () => {
  it("uses the same command for human and agent actuation and fails stale deixis closed", async () => {
    const current: PageContextSnapshot = {
      mode: "philosophy",
      view_id: "chronology",
      graph_mode: "nodes",
      selected: null,
      path_start_node_id: null,
      active_layers: ["source-relation"],
      active_predicates: ["relates"],
      deep_link: "http://tos.local/?view=chronology",
    };
    const calls: string[] = [];
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": (input) => {
        const id = String(input.item_id);
        calls.push(id);
        current.selected = { id, kind: "node", label: id };
      },
      "tos.page.show-neighborhood": () => calls.push(`neighborhood:${current.selected?.id}`),
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const tools = new Map<string, RegisteredTool>();
    const modelContext = {
      registerTool: vi.fn(async (tool: RegisteredTool, options?: { signal?: AbortSignal }) => {
        tools.set(tool.name, tool);
        options?.signal?.addEventListener("abort", () => {
          if (tools.get(tool.name) === tool) tools.delete(tool.name);
        }, { once: true });
      }),
    };
    const adapter = createWebMCPAdapter(registry, { modelContext } as unknown as WebMCPDocument);
    await adapter.start();

    await registry.invoke("tos.page.select", { item_id: "node:human" });
    await adapter.refresh();
    const staleNeighborhoodTool = tools.get("tos.page.show-neighborhood");
    const staleEpistemicTool = tools.get("tos.page.inspect-epistemic");
    expect(staleNeighborhoodTool).toBeDefined();
    expect(staleEpistemicTool).toBeDefined();

    const selectTool = tools.get("tos.page.select");
    await selectTool?.execute({ item_id: "node:agent" }, { signal: new AbortController().signal });
    await adapter.refresh();
    expect(calls.slice(0, 2)).toEqual(["node:human", "node:agent"]);

    await expect(
      staleNeighborhoodTool?.execute({}, { signal: new AbortController().signal }),
    ).rejects.toThrow("stale page context revision");
    await expect(
      staleEpistemicTool?.execute({}, { signal: new AbortController().signal }),
    ).rejects.toThrow("stale page context revision");
    adapter.stop();
  });

  it("does not register philosophy route tools outside a bounded philosophy context", async () => {
    const current: PageContextSnapshot = {
      mode: "corpus",
      view_id: "corpus-topology",
      graph_mode: "nodes",
      selected: { id: "corpus-node", kind: "node" },
      path_start_node_id: null,
      active_layers: [],
      active_predicates: [],
      deep_link: "http://tos.local/?mode=corpus",
    };
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": noop,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const tools = new Map<string, RegisteredTool>();
    const modelContext = {
      registerTool: vi.fn(async (tool: RegisteredTool, options?: { signal?: AbortSignal }) => {
        tools.set(tool.name, tool);
        options?.signal?.addEventListener("abort", () => tools.delete(tool.name), { once: true });
      }),
    };
    const adapter = createWebMCPAdapter(registry, { modelContext } as unknown as WebMCPDocument);

    await adapter.start();

    expect(tools.has("tos.page.show-neighborhood")).toBe(false);
    expect(tools.has("tos.page.start-path")).toBe(false);
    expect(tools.has("tos.page.inspect-epistemic")).toBe(false);
    adapter.stop();
  });

  it("does not offer projection rerouting for an aggregate cluster edge", async () => {
    const current: PageContextSnapshot = {
      mode: "philosophy",
      view_id: "chronology",
      graph_mode: "clusters",
      selected: {
        id: "cluster-relation:0",
        kind: "edge",
        from_id: "cluster:a",
        to_id: "cluster:b",
        reroutable: false,
      },
      path_start_node_id: null,
      active_layers: ["source-relation"],
      active_predicates: ["relates"],
      deep_link: "http://tos.local/?view=chronology&graph=clusters",
    };
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": noop,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const tools = new Map<string, RegisteredTool>();
    const modelContext = {
      registerTool: vi.fn(async (tool: RegisteredTool, options?: { signal?: AbortSignal }) => {
        tools.set(tool.name, tool);
        options?.signal?.addEventListener("abort", () => tools.delete(tool.name), { once: true });
      }),
    };
    const adapter = createWebMCPAdapter(registry, { modelContext } as unknown as WebMCPDocument);

    await adapter.start();

    expect(tools.has("tos.page.reroute-without-selection")).toBe(false);
    expect(tools.has("tos.page.inspect-epistemic")).toBe(false);
    adapter.stop();
  });
});
