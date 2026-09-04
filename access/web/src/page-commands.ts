export type PageSelection = {
  id: string;
  kind: "node" | "edge" | "cluster" | "item";
  label?: string;
  from_id?: string;
  to_id?: string;
  reroutable?: boolean;
};

export type ResearchWorkspaceSummary = {
  schema: "tos_research_workspace_summary_v1";
  session_id: string;
  revision: number;
  hypothesis_count: number;
  excluded_edge_count: number;
  comparison_count: number;
  note_count: number;
  journal_count: number;
  can_undo: boolean;
  can_redo: boolean;
};

export type PageContextSnapshot = {
  mode: "philosophy" | "corpus";
  view_id: string;
  graph_mode: "clusters" | "nodes";
  selected: PageSelection | null;
  path_start_node_id: string | null;
  active_layers: string[];
  active_predicates: string[];
  deep_link: string;
  research_workspace: ResearchWorkspaceSummary;
};

export type PageContext = PageContextSnapshot & {
  schema: "tos_page_context_v1";
  revision: number;
  pending_command_ids: string[];
};

export type PageCommandId =
  | "tos.page.context"
  | "tos.page.open-view"
  | "tos.page.search"
  | "tos.page.select"
  | "tos.page.show-neighborhood"
  | "tos.page.start-path"
  | "tos.page.find-path"
  | "tos.page.reroute-without-selection"
  | "tos.page.inspect-epistemic"
  | "tos.page.research-workspace"
  | "tos.page.add-research-note"
  | "tos.page.add-session-hypothesis"
  | "tos.page.exclude-selected-edge"
  | "tos.page.save-route-comparison"
  | "tos.page.workspace-undo"
  | "tos.page.workspace-redo"
  | "tos.page.workspace-export"
  | "tos.page.workspace-import"
  | "tos.page.clear-focus"
  | "tos.page.cancel";

export type HandledPageCommandId = Exclude<PageCommandId, "tos.page.context" | "tos.page.cancel">;
export type PageCommandInput = Record<string, unknown> & { context_revision?: number };
export type PageCommandExecution = { signal: AbortSignal };
export type PageCommandHandler = (
  input: PageCommandInput,
  execution: PageCommandExecution,
) => Promise<unknown> | unknown;
export type PageCommandHandlers = Record<HandledPageCommandId, PageCommandHandler>;
export type PageContextListener = (context: PageContext) => void;

export class StalePageContextError extends Error {
  constructor(expected: number, actual: number) {
    super(`stale page context revision: expected ${expected}, current ${actual}`);
    this.name = "StalePageContextError";
  }
}

export function isPageCommandCancellation(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError";
}

export function requireKnownViewId(viewId: string, knownViewIds: Iterable<string>): void {
  if (!viewId || !new Set(knownViewIds).has(viewId)) {
    throw new Error(`unknown Tree of Sophia view: ${viewId || "(empty)"}`);
  }
}

export function reloadableFocusId(
  selected: PageSelection | null,
  selectedGraphId: string | null,
  reloadableIds: Iterable<string>,
): string {
  const allowed = new Set(reloadableIds);
  const selectedId = selected?.id || "";
  if (selectedId && allowed.has(selectedId)) return selectedId;
  return selectedGraphId && allowed.has(selectedGraphId) ? selectedGraphId : "";
}

export function createPageCommandRegistry(
  readContext: () => PageContextSnapshot,
  handlers: PageCommandHandlers,
) {
  let revision = 0;
  const listeners = new Set<PageContextListener>();
  const pending = new Map<string, AbortController>();
  const readOnlyCommands = new Set<PageCommandId>([
    "tos.page.research-workspace",
    "tos.page.workspace-export",
  ]);

  const context = (): PageContext => ({
    schema: "tos_page_context_v1",
    revision,
    pending_command_ids: [...pending.keys()].sort(),
    ...readContext(),
  });

  const publish = (): PageContext => {
    const current = context();
    listeners.forEach((listener) => listener(current));
    return current;
  };

  const cancel = (commandId?: string): string[] => {
    const cancelled: string[] = [];
    for (const [pendingId, controller] of pending) {
      if (commandId && pendingId !== commandId) continue;
      controller.abort(new DOMException(`cancelled ${pendingId}`, "AbortError"));
      pending.delete(pendingId);
      cancelled.push(pendingId);
    }
    if (cancelled.length) publish();
    return cancelled.sort();
  };

  const invoke = async (
    commandId: PageCommandId,
    input: PageCommandInput = {},
    options: { signal?: AbortSignal } = {},
  ): Promise<Record<string, unknown>> => {
    const expectedRevision = input.context_revision;
    if (expectedRevision !== undefined && expectedRevision !== revision) {
      throw new StalePageContextError(expectedRevision, revision);
    }
    if (commandId === "tos.page.context") return context();
    if (commandId === "tos.page.cancel") {
      const command = typeof input.command_id === "string" ? input.command_id : undefined;
      const cancelled = cancel(command);
      return {
        schema: "tos_page_command_result_v1",
        command_id: commandId,
        cancelled_command_ids: cancelled,
        context: context(),
      };
    }
    if (readOnlyCommands.has(commandId)) {
      const value = await handlers[commandId as HandledPageCommandId](input, {
        signal: options.signal || new AbortController().signal,
      });
      return {
        schema: "tos_page_command_result_v1",
        command_id: commandId,
        context_revision: revision,
        context: context(),
        value,
      };
    }

    cancel();
    const controller = new AbortController();
    const abortFromCaller = () => controller.abort(options.signal?.reason);
    if (options.signal?.aborted) abortFromCaller();
    else options.signal?.addEventListener("abort", abortFromCaller, { once: true });
    pending.set(commandId, controller);

    try {
      const value = await handlers[commandId as HandledPageCommandId](input, { signal: controller.signal });
      controller.signal.throwIfAborted();
      if (expectedRevision !== undefined && expectedRevision !== revision) {
        throw new StalePageContextError(expectedRevision, revision);
      }
      if (pending.get(commandId) === controller) pending.delete(commandId);
      revision += 1;
      const current = publish();
      return {
        schema: "tos_page_command_result_v1",
        command_id: commandId,
        context_revision: revision,
        context: current,
        value,
      };
    } finally {
      options.signal?.removeEventListener("abort", abortFromCaller);
      if (pending.get(commandId) === controller) pending.delete(commandId);
    }
  };

  const subscribe = (listener: PageContextListener): (() => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };

  const notifyStateChange = (): PageContext => {
    cancel();
    revision += 1;
    return publish();
  };

  return { context, invoke, subscribe, cancel, notifyStateChange };
}

export type PageCommandRegistry = ReturnType<typeof createPageCommandRegistry>;
