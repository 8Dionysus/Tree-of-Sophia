import type { PageCommandId, PageCommandRegistry, PageContext } from "./page-commands";

type JsonSchema = Record<string, unknown>;

type WebMCPTool = {
  name: string;
  title?: string;
  description: string;
  inputSchema?: JsonSchema;
  annotations?: { readOnlyHint?: boolean; untrustedContentHint?: boolean };
  execute: (input: Record<string, unknown>, options: { signal: AbortSignal }) => Promise<unknown>;
};

type ModelContextLike = {
  registerTool(tool: WebMCPTool, options?: { signal?: AbortSignal }): Promise<void>;
};

export type WebMCPDocument = Document & { modelContext?: ModelContextLike };

const emptySchema: JsonSchema = { type: "object", properties: {}, additionalProperties: false };

function objectSchema(properties: Record<string, unknown>, required: string[] = []): JsonSchema {
  return {
    type: "object",
    properties,
    ...(required.length ? { required } : {}),
    additionalProperties: false,
  };
}

function toolResult(value: unknown): Record<string, unknown> {
  return {
    content: [{ type: "text", text: JSON.stringify(value) }],
  };
}

function commandTool(
  registry: PageCommandRegistry,
  commandId: PageCommandId,
  definition: Omit<WebMCPTool, "execute">,
  capturedRevision?: number,
  compact?: (value: Record<string, unknown>) => unknown,
): WebMCPTool {
  return {
    ...definition,
    execute: async (input, options) => {
      const commandInput = capturedRevision === undefined
        ? input
        : { ...input, context_revision: capturedRevision };
      const value = await registry.invoke(commandId, commandInput, { signal: options.signal });
      return toolResult(compact ? compact(value) : value);
    },
  };
}

function compactEvidenceResult(result: Record<string, unknown>): unknown {
  const value = result.value as Record<string, unknown> | undefined;
  const summary = value?.agent_summary as Record<string, unknown> | undefined;
  if (!summary) return result;
  const context = result.context as Record<string, unknown> | undefined;
  return {
    ...summary,
    context_revision: result.context_revision,
    deep_link: context?.deep_link,
  };
}

function stableTools(registry: PageCommandRegistry): WebMCPTool[] {
  return [
    commandTool(registry, "tos.page.context", {
      name: "tos.page.context",
      title: "Read Tree of Sophia page context",
      description: "Return the current ToS view, selected object, filters, path start, deep link, and context revision.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: true },
    }),
    commandTool(registry, "tos.page.open-view", {
      name: "tos.page.open-view",
      title: "Open a Tree of Sophia view",
      description: "Open a named philosophy or corpus view on the shared ToS page, optionally focusing one object.",
      inputSchema: objectSchema(
        {
          mode: { type: "string", enum: ["philosophy", "corpus"] },
          view_id: { type: "string", minLength: 1 },
          graph_mode: { type: "string", enum: ["clusters", "nodes"] },
          focus_id: { type: "string" },
        },
        ["mode", "view_id"],
      ),
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.search", {
      name: "tos.page.search",
      title: "Search on the Tree of Sophia page",
      description: "Search the active ToS surface and show the results in the shared page inspector.",
      inputSchema: objectSchema({ query: { type: "string" } }, ["query"]),
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.select", {
      name: "tos.page.select",
      title: "Select an object on the Tree of Sophia page",
      description: "Select a node, edge, cluster, or result already present in the active ToS view by its stable ID.",
      inputSchema: objectSchema({ item_id: { type: "string", minLength: 1 } }, ["item_id"]),
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.clear-focus", {
      name: "tos.page.clear-focus",
      title: "Leave the current graph focus",
      description: "Clear neighborhood, path, and expanded-cluster focus while preserving the active ToS view.",
      inputSchema: emptySchema,
      annotations: { readOnlyHint: false },
    }),
    commandTool(registry, "tos.page.cancel", {
      name: "tos.page.cancel",
      title: "Cancel an active ToS page operation",
      description: "Cancel active page commands, or one command identified by command_id.",
      inputSchema: objectSchema({ command_id: { type: "string" } }),
      annotations: { readOnlyHint: false },
    }),
  ];
}

function dynamicTools(registry: PageCommandRegistry, context: PageContext): WebMCPTool[] {
  const selected = context.selected;
  if (!selected) return [];
  const evidenceAvailable = context.mode === "philosophy"
    || (context.mode === "corpus" && context.view_id === "route-graph");
  if (!evidenceAvailable) return [];
  const tools: WebMCPTool[] = [];
  if ((selected.kind === "node" || selected.kind === "edge") && selected.reroutable !== false) {
    tools.push(
      commandTool(registry, "tos.page.inspect-epistemic", {
        name: "tos.page.inspect-epistemic",
        title: "Open Evidence Lens for this selection",
        description: `Show why the selected ${selected.kind} ${selected.id} may be presented, which owner routes support it, and which conclusions remain forbidden.`,
        inputSchema: objectSchema({ limit: { type: "integer", minimum: 1, maximum: 200 } }),
        annotations: { readOnlyHint: false, untrustedContentHint: true },
      }, context.revision, compactEvidenceResult),
    );
  }
  if (context.mode !== "philosophy") return tools;
  if (context.active_layers.length === 0 || context.active_predicates.length === 0) return tools;
  if (selected.kind === "node") {
    tools.push(
      commandTool(registry, "tos.page.show-neighborhood", {
        name: "tos.page.show-neighborhood",
        title: "Show this node's neighborhood",
        description: `Show the filtered neighborhood of the currently selected node ${selected.id} in the shared graph.`,
        inputSchema: objectSchema({ depth: { type: "integer", minimum: 1, maximum: 3 } }),
        annotations: { readOnlyHint: false },
      }, context.revision),
      commandTool(registry, "tos.page.start-path", {
        name: "tos.page.start-path",
        title: "Start a path from this node",
        description: `Use the currently selected node ${selected.id} as the deictic start of the next path query.`,
        inputSchema: emptySchema,
        annotations: { readOnlyHint: false },
      }, context.revision),
    );
    if (context.path_start_node_id && context.path_start_node_id !== selected.id) {
      tools.push(
        commandTool(registry, "tos.page.find-path", {
          name: "tos.page.find-path-to-selection",
          title: "Find paths to this node",
          description: `Find deterministic paths from ${context.path_start_node_id} to the currently selected node ${selected.id}.`,
          inputSchema: objectSchema({
            direction: { type: "string", enum: ["outgoing", "incoming", "either"], default: "outgoing" },
            max_depth: { type: "integer", minimum: 1, maximum: 8 },
            alternative_limit: { type: "integer", minimum: 1, maximum: 5 },
            excluded_edge_ids: { type: "array", items: { type: "string" }, maxItems: 64 },
            constrain_to_view: { type: "boolean" },
          }),
          annotations: { readOnlyHint: false },
        }, context.revision),
      );
    }
  }
  if (
    selected.kind === "edge" &&
    selected.reroutable !== false &&
    selected.from_id &&
    selected.to_id
  ) {
    tools.push(
      commandTool(registry, "tos.page.reroute-without-selection", {
        name: "tos.page.reroute-without-selection",
        title: "Find alternatives without this edge",
        description: `Find deterministic alternative paths from ${selected.from_id} to ${selected.to_id}, excluding the currently selected edge ${selected.id}.`,
        inputSchema: objectSchema({
          direction: { type: "string", enum: ["outgoing", "incoming", "either"], default: "outgoing" },
          max_depth: { type: "integer", minimum: 1, maximum: 8 },
          alternative_limit: { type: "integer", minimum: 1, maximum: 5 },
          constrain_to_view: { type: "boolean" },
        }),
        annotations: { readOnlyHint: false },
      }, context.revision),
    );
  }
  return tools;
}

export function createWebMCPAdapter(registry: PageCommandRegistry, targetDocument: WebMCPDocument) {
  let stableController: AbortController | null = null;
  let dynamicController: AbortController | null = null;
  let unsubscribe: (() => void) | null = null;
  let refreshGeneration = 0;
  let refreshQueue = Promise.resolve();
  let registrationError: string | null = null;
  let probeTimer: ReturnType<typeof setTimeout> | null = null;
  let probeAttempts = 0;

  const registerAll = async (tools: WebMCPTool[], controller: AbortController): Promise<void> => {
    const modelContext = targetDocument.modelContext;
    if (!modelContext) return;
    for (const tool of tools) {
      await modelContext.registerTool(tool, { signal: controller.signal });
    }
  };

  const refresh = (context: PageContext = registry.context()): Promise<void> => {
    const generation = ++refreshGeneration;
    refreshQueue = refreshQueue.then(async () => {
      if (!targetDocument.modelContext || generation !== refreshGeneration) return;
      dynamicController?.abort();
      const controller = new AbortController();
      dynamicController = controller;
      try {
        await registerAll(dynamicTools(registry, context), controller);
        registrationError = null;
      } catch (error) {
        controller.abort();
        registrationError = String(error);
      }
    });
    return refreshQueue;
  };

  const start = async (): Promise<void> => {
    if (stableController) return;
    if (!targetDocument.modelContext) {
      if (probeAttempts < 20 && !probeTimer) {
        probeAttempts += 1;
        probeTimer = setTimeout(() => {
          probeTimer = null;
          void start();
        }, 250);
      }
      return;
    }
    stableController = new AbortController();
    try {
      await registerAll(stableTools(registry), stableController);
      unsubscribe = registry.subscribe((context) => void refresh(context));
      await refresh();
    } catch (error) {
      registrationError = String(error);
      stableController.abort();
      stableController = null;
    }
  };

  const stop = (): void => {
    refreshGeneration += 1;
    unsubscribe?.();
    unsubscribe = null;
    dynamicController?.abort();
    stableController?.abort();
    if (probeTimer) clearTimeout(probeTimer);
    probeTimer = null;
    dynamicController = null;
    stableController = null;
  };

  const status = () => ({
    supported: Boolean(targetDocument.modelContext),
    registered: Boolean(stableController && !stableController.signal.aborted),
    registration_error: registrationError,
  });

  return { start, stop, refresh, status };
}
