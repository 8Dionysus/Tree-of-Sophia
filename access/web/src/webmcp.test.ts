import { describe, expect, it, vi } from "vitest";
import { createPageCommandRegistry, type PageContextSnapshot } from "./page-commands";
import { createWebMCPAdapter, type WebMCPDocument } from "./webmcp";

type RegisteredTool = {
  name: string;
  execute: (input: Record<string, unknown>, options: { signal: AbortSignal }) => Promise<unknown>;
};

const workspaceNoopHandlers = {
  "tos.page.research-workspace": () => ({}),
  "tos.page.add-research-note": () => ({}),
  "tos.page.add-session-hypothesis": () => ({}),
  "tos.page.exclude-selected-edge": () => ({}),
  "tos.page.save-route-comparison": () => ({}),
  "tos.page.workspace-undo": () => ({}),
  "tos.page.workspace-redo": () => ({}),
  "tos.page.workspace-export": () => ({}),
  "tos.page.workspace-import": () => ({}),
};

function workspaceSummary() {
  return {
    schema: "tos_research_workspace_summary_v1" as const,
    session_id: "research:test",
    revision: 0,
    hypothesis_count: 0,
    excluded_edge_count: 0,
    comparison_count: 0,
    note_count: 0,
    journal_count: 0,
    can_undo: false,
    can_redo: false,
  };
}

describe("WebMCP page-command binding", () => {
  it("reports product-shell connection and selection-bound tool counts", async () => {
    const current: PageContextSnapshot = {
      mode: "philosophy",
      view_id: "chronology",
      graph_mode: "nodes",
      selected: null,
      path_start_node_id: null,
      active_layers: ["source-relation"],
      active_predicates: ["relates"],
      deep_link: "http://tos.local/?view=chronology",
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
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
    const seen: Array<ReturnType<typeof adapter.status>> = [];
    const unsubscribe = adapter.subscribeStatus((status) => seen.push(status));

    await adapter.start();
    expect(adapter.status()).toMatchObject({
      supported: true,
      registered: true,
      selection_tool_count: 0,
      context_revision: 0,
    });
    expect(adapter.status().stable_tool_count).toBeGreaterThan(0);

    current.selected = { id: "node:a", kind: "node", reroutable: true };
    registry.notifyStateChange();
    await adapter.refresh();
    expect(adapter.status().selection_tool_count).toBeGreaterThan(0);
    expect(adapter.status().tool_count).toBe(
      adapter.status().stable_tool_count + adapter.status().selection_tool_count,
    );
    expect(adapter.status().context_revision).toBe(1);
    expect(seen.length).toBeGreaterThan(1);

    unsubscribe();
    adapter.stop();
    expect(adapter.status()).toMatchObject({ registered: false, tool_count: 0 });
  });

  it("reports an immediately usable no-WebMCP fallback state", async () => {
    const current: PageContextSnapshot = {
      mode: "philosophy",
      view_id: "chronology",
      graph_mode: "nodes",
      selected: null,
      path_start_node_id: null,
      active_layers: [],
      active_predicates: [],
      deep_link: "http://tos.local/",
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
    });
    const adapter = createWebMCPAdapter(registry, {} as WebMCPDocument);

    await adapter.start();
    expect(adapter.status()).toMatchObject({
      supported: false,
      registered: false,
      stable_tool_count: 0,
      selection_tool_count: 0,
      tool_count: 0,
      context_revision: 0,
    });
    adapter.stop();
  });

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
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
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
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
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

  it("offers compact Evidence Lens output for a selected corpus relation", async () => {
    const current: PageContextSnapshot = {
      mode: "corpus",
      view_id: "route-graph",
      graph_mode: "nodes",
      selected: { id: "m113", kind: "edge", from_id: "principle", to_id: "event" },
      path_start_node_id: null,
      active_layers: [],
      active_predicates: [],
      deep_link: "http://tos.local/?mode=corpus&view=route-graph&focus=m113",
      research_workspace: workspaceSummary(),
    };
    const noop = vi.fn();
    const inspect = vi.fn(() => ({
      schema: "tos_evidence_lens_packet_v1",
      agent_summary: {
        selection: "m113",
        finding: "Retained canon route with open modern evidence closure.",
        posture: "canon-retained-evidence-open",
        can_conclude: true,
        page_updated: true,
        next_actions: ["inspect source anchors"],
      },
      routes: Array.from({ length: 40 }, (_, index) => ({ ref: `large-${index}` })),
    }));
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": noop,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": inspect,
      "tos.page.clear-focus": noop,
      ...workspaceNoopHandlers,
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
    const tool = tools.get("tos.page.inspect-epistemic");
    expect(tool).toBeDefined();
    const result = await tool?.execute({}, { signal: new AbortController().signal }) as {
      content: Array<{ text: string }>;
    };
    const text = result.content[0].text;
    expect(text.length).toBeLessThan(1500);
    expect(text).toContain("canon-retained-evidence-open");
    expect(text).not.toContain("large-39");
    adapter.stop();
  });

  it("keeps epistemic inspection available when visual graph filters are empty", async () => {
    const current: PageContextSnapshot = {
      mode: "philosophy",
      view_id: "chronology",
      graph_mode: "nodes",
      selected: { id: "candidate-node:a", kind: "node", reroutable: true },
      path_start_node_id: null,
      active_layers: [],
      active_predicates: [],
      deep_link: "http://tos.local/?view=chronology",
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
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

    expect(tools.has("tos.page.inspect-epistemic")).toBe(true);
    expect(tools.has("tos.page.show-neighborhood")).toBe(false);
    expect(tools.has("tos.page.start-path")).toBe(false);
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
      research_workspace: workspaceSummary(),
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
      ...workspaceNoopHandlers,
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

  it("exposes a local-only research workspace and selection-bound research actions", async () => {
    const current = {
      mode: "philosophy",
      view_id: "source-evidence",
      graph_mode: "nodes",
      selected: {
        id: "edge:candidate",
        kind: "edge",
        from_id: "node:a",
        to_id: "node:b",
        reroutable: true,
      },
      path_start_node_id: null,
      active_layers: ["candidate"],
      active_predicates: ["contested_by"],
      deep_link: "http://tos.local/?view=source-evidence&focus=edge%3Acandidate",
      research_workspace: workspaceSummary(),
    } as PageContextSnapshot;
    const calls: Array<[string, Record<string, unknown>]> = [];
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": noop,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": () => ({
        found: true,
        from_id: "node:a",
        to_id: "node:b",
        path_count: 1,
        paths: [{ node_ids: ["node:a", "node:via", "node:b"], edge_ids: ["edge:a-via", "edge:via-b"], nodes: Array.from({ length: 40 }, (_, index) => ({ label: `large-node-${index}` })) }],
        excluded_edge_ids: ["edge:candidate"],
      }),
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
      "tos.page.research-workspace": () => ({
        local_only: true,
        packet: {
          hypotheses: Array.from({ length: 40 }, (_, index) => ({ id: `hypothesis:${index}`, title: `Hypothesis ${index}`, body: `large-hypothesis-${index}`, posture: { session_hypothesis: true, source: false, reviewed: false, canon: false } })),
          excluded_edge_ids: ["edge:candidate"],
          route_snapshots: [],
          notes: [],
          journal: [],
        },
      }),
      "tos.page.add-research-note": (input) => calls.push(["note", input]),
      "tos.page.add-session-hypothesis": (input) => calls.push(["hypothesis", input]),
      "tos.page.exclude-selected-edge": (input) => calls.push(["exclude", input]),
      "tos.page.save-route-comparison": (input) => calls.push(["comparison", input]),
      "tos.page.workspace-undo": () => calls.push(["undo", {}]),
      "tos.page.workspace-redo": () => calls.push(["redo", {}]),
      "tos.page.workspace-export": () => ({ schema_version: "tos_research_session_packet_v1" }),
      "tos.page.workspace-import": () => calls.push(["import", {}]),
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

    expect(tools.has("tos.page.research-workspace")).toBe(true);
    expect(tools.has("tos.page.add-research-note")).toBe(true);
    expect(tools.has("tos.page.workspace-undo")).toBe(true);
    expect(tools.has("tos.page.workspace-redo")).toBe(true);
    expect(tools.has("tos.page.add-session-hypothesis")).toBe(true);
    expect(tools.has("tos.page.exclude-selected-edge")).toBe(true);
    expect(tools.has("tos.page.save-route-comparison")).toBe(true);

    const workspaceResult = await tools.get("tos.page.research-workspace")?.execute(
      {},
      { signal: new AbortController().signal },
    ) as { content: Array<{ text: string }> };
    expect(workspaceResult.content[0].text.length).toBeLessThan(1500);
    expect(workspaceResult.content[0].text).toContain("Hypothesis 39");
    expect(workspaceResult.content[0].text).not.toContain("large-hypothesis-0");

    const pathResult = await tools.get("tos.page.reroute-without-selection")?.execute(
      {},
      { signal: new AbortController().signal },
    ) as { content: Array<{ text: string }> };
    expect(pathResult.content[0].text.length).toBeLessThan(1500);
    expect(pathResult.content[0].text).toContain("alternative route found");
    expect(pathResult.content[0].text).not.toContain("large-node-39");
    await adapter.refresh();

    await tools.get("tos.page.add-session-hypothesis")?.execute(
      { statement: "Treat the disputed relation as a working alternative." },
      { signal: new AbortController().signal },
    );
    expect(calls[0][0]).toBe("hypothesis");
    expect(calls[0][1]).toMatchObject({
      statement: "Treat the disputed relation as a working alternative.",
      context_revision: 1,
    });
    adapter.stop();
  });
});
