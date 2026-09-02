import { describe, expect, it, vi } from "vitest";
import {
  createPageCommandRegistry,
  isPageCommandCancellation,
  reloadableFocusId,
  requireKnownViewId,
  StalePageContextError,
  type PageContextSnapshot,
} from "./page-commands";

function snapshot(): PageContextSnapshot {
  return {
    mode: "philosophy",
    view_id: "chronology",
    graph_mode: "nodes",
    selected: null,
    path_start_node_id: null,
    active_layers: [],
    active_predicates: [],
    deep_link: "http://tos.local/?view=chronology",
  };
}

describe("page command registry", () => {
  it("advances revision and rejects stale deictic calls", async () => {
    const current = snapshot();
    const selected = vi.fn((input: Record<string, unknown>) => {
      current.selected = { id: String(input.item_id), kind: "node" };
      return current.selected;
    });
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": selected,
      "tos.page.show-neighborhood": noop,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });

    const result = await registry.invoke("tos.page.select", { item_id: "node:a", context_revision: 0 });
    expect(result.context_revision).toBe(1);
    expect(registry.context().selected?.id).toBe("node:a");
    await expect(
      registry.invoke("tos.page.show-neighborhood", { context_revision: 0 }),
    ).rejects.toBeInstanceOf(StalePageContextError);
  });

  it("propagates caller cancellation into the active handler", async () => {
    const current = snapshot();
    const observed = vi.fn();
    const pendingHandler = (_input: Record<string, unknown>, execution: { signal: AbortSignal }) =>
      new Promise((_resolve, reject) => {
        observed(execution.signal);
        execution.signal.addEventListener("abort", () => reject(execution.signal.reason), { once: true });
      });
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": pendingHandler,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const controller = new AbortController();
    const invocation = registry.invoke("tos.page.show-neighborhood", {}, { signal: controller.signal });
    controller.abort(new DOMException("stop", "AbortError"));

    await expect(invocation).rejects.toMatchObject({ name: "AbortError" });
    expect(observed.mock.calls[0][0].aborted).toBe(true);
    expect(registry.context().revision).toBe(0);
  });

  it("cancels a context-bound async command before a later selection can commit", async () => {
    const current = snapshot();
    let release: (() => void) | undefined;
    const pendingHandler = async (_input: Record<string, unknown>, execution: { signal: AbortSignal }) => {
      await new Promise<void>((resolve) => { release = resolve; });
      execution.signal.throwIfAborted();
      current.selected = { id: "node:stale", kind: "node" };
    };
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": (input) => {
        current.selected = { id: String(input.item_id), kind: "node" };
      },
      "tos.page.show-neighborhood": pendingHandler,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const invocation = registry.invoke("tos.page.show-neighborhood", { context_revision: 0 });

    await registry.invoke("tos.page.select", { item_id: "node:fresh", context_revision: 0 });
    release?.();

    await expect(invocation).rejects.toMatchObject({ name: "AbortError" });
    expect(registry.context().selected?.id).toBe("node:fresh");
    expect(registry.context().revision).toBe(1);
  });

  it("validates view identities and emits only reloadable focus IDs", () => {
    expect(() => requireKnownViewId("missing", ["chronology"])).toThrow("unknown Tree of Sophia view");
    expect(() => requireKnownViewId("chronology", ["chronology"])).not.toThrow();
    expect(
      reloadableFocusId({ id: "search-only", kind: "item" }, "search-only", ["node:a"]),
    ).toBe("");
    expect(
      reloadableFocusId({ id: "node:a", kind: "node" }, "node:a", ["node:a"]),
    ).toBe("node:a");
  });

  it("distinguishes expected command cancellation from genuine failures", () => {
    expect(isPageCommandCancellation(new DOMException("superseded", "AbortError"))).toBe(true);
    expect(isPageCommandCancellation(new Error("request failed"))).toBe(false);
    expect(isPageCommandCancellation("AbortError")).toBe(false);
  });

  it("cancels an active operation through the page cancel command", async () => {
    const current = snapshot();
    const pendingHandler = (_input: Record<string, unknown>, execution: { signal: AbortSignal }) =>
      new Promise((_resolve, reject) => {
        execution.signal.addEventListener("abort", () => reject(execution.signal.reason), { once: true });
      });
    const noop = vi.fn();
    const registry = createPageCommandRegistry(() => current, {
      "tos.page.open-view": noop,
      "tos.page.search": noop,
      "tos.page.select": noop,
      "tos.page.show-neighborhood": pendingHandler,
      "tos.page.start-path": noop,
      "tos.page.find-path": noop,
      "tos.page.reroute-without-selection": noop,
      "tos.page.inspect-epistemic": noop,
      "tos.page.clear-focus": noop,
    });
    const invocation = registry.invoke("tos.page.show-neighborhood");
    const cancellation = await registry.invoke("tos.page.cancel", {
      command_id: "tos.page.show-neighborhood",
    });

    expect(cancellation.cancelled_command_ids).toEqual(["tos.page.show-neighborhood"]);
    await expect(invocation).rejects.toMatchObject({ name: "AbortError" });
    expect(registry.context().pending_command_ids).toEqual([]);
    expect(registry.context().revision).toBe(0);
  });
});
