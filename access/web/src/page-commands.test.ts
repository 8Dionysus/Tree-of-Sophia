import {buildInterpretationComparison} from './interpretation-comparison.mjs';
import {pageCommandInteger, pageCommandOpaqueString, pageCommandDirection} from "./page-input";
import './observatory/human-forms-wasm-test-runtime.mjs';
import { describe, expect, it, vi } from "vitest";
import {
  createPageCommandRegistry,
  isPageCommandCancellation,
  reloadableFocusId,
  requireKnownViewId,
  StalePageContextError,
  type PageContextSnapshot,
} from "./page-commands";

const workspaceNoopHandlers = {
  "tos.page.prepare-word-analysis": () => ({}),
  "tos.page.inspect-selection": () => ({}),
  "tos.page.compare-readings": () => ({}),
  "tos.page.research-workspace": () => ({}),
  "tos.page.add-research-note": () => ({}),
  "tos.page.add-session-hypothesis": () => ({}),
  "tos.page.stage-proposal": () => ({}),
  "tos.page.exclude-selected-edge": () => ({}),
  "tos.page.save-route-comparison": () => ({}),
  "tos.page.workspace-undo": () => ({}),
  "tos.page.workspace-redo": () => ({}),
  "tos.page.workspace-export": () => ({}),
  "tos.page.workspace-import": () => ({}),
};

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
    research_workspace: {
      schema: "tos_research_workspace_summary_v1",
      session_id: "research:test",
      revision: 0,
      hypothesis_count: 0,
      proposal_count: 0,
      excluded_edge_count: 0,
      comparison_count: 0,
      note_count: 0,
      journal_count: 0,
      can_undo: false,
      can_redo: false,
    },
  };
}

describe("page command registry", () => {
  it("preserves iterable-before-getter focus order and skips falsey view iteration", () => {
    const events:string[]=[];
    const ids={*[Symbol.iterator](){events.push('iterate');yield 'node\ud800';}};
    expect(()=>requireKnownViewId('',ids)).toThrow('(empty)');expect(events).toEqual([]);
    const selected={kind:'node',get id(){events.push('selected');return 'node\ud800';}} as const;
    expect(reloadableFocusId(selected,'graph',ids)).toBe('node\ud800');
    expect(events).toEqual(['iterate','selected']);
    events.length=0;
    const missing={kind:'node',get id(){events.push('selected');return '';}} as const;
    expect(reloadableFocusId(missing,'node\ud800',ids)).toBe('node\ud800');
    expect(events).toEqual(['iterate','selected']);
  });

  it("reads and exports local research state without advancing the deictic revision", async () => {
    const current = snapshot();
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
      "tos.page.research-workspace": () => ({ local_only: true }),
      "tos.page.workspace-export": () => ({ schema_version: "tos_research_session_packet_v1" }),
    });

    const read = await registry.invoke("tos.page.research-workspace");
    const exported = await registry.invoke("tos.page.workspace-export");

    expect(read.context_revision).toBe(0);
    expect(read.value).toEqual({ local_only: true });
    expect(exported.context_revision).toBe(0);
    expect(registry.context().revision).toBe(0);
  });

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
      ...workspaceNoopHandlers,
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
      ...workspaceNoopHandlers,
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
      ...workspaceNoopHandlers,
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
      ...workspaceNoopHandlers,
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


describe("page input guards used by maintained command handlers", () => {
  it("preserves page input numeric profiles and native coercion", () => {
    expect(pageCommandInteger({limit: 99.9}, "limit", "page-knowledge-limit")).toBe(40);
    expect(pageCommandInteger({}, "limit", "gaps-limit")).toBe(20);
    expect(pageCommandInteger({}, "rank", "page-word-rank")).toBe(1);
    expect(pageCommandInteger({}, "depth", "neighborhood-depth")).toBe(1);
    expect(pageCommandInteger({}, "max_depth", "page-path-depth")).toBe(6);
    expect(pageCommandInteger({}, "alternative_limit", "page-path-alternatives")).toBe(1);
    expect(pageCommandInteger({}, "alternative_limit", "page-reroute-alternatives")).toBe(3);
    expect(pageCommandInteger({}, "limit", "epistemic-limit")).toBe(80);
    expect(pageCommandInteger({}, "limit", "page-compare-limit")).toBe(60);
    const valueOf = vi.fn(() => -1.9);
    const getter = vi.fn(() => ({valueOf}));
    expect(pageCommandInteger({get rank() { return getter(); }}, "rank", "page-word-rank")).toBe(1);
    expect(getter).toHaveBeenCalledTimes(1);
    expect(valueOf).toHaveBeenCalledTimes(1);
    expect(() => pageCommandInteger({rank: Symbol("rank")}, "rank", "page-word-rank")).toThrow(TypeError);
  });
  it("preserves page input opaque cursor identity and empty omission", () => {
    for (const cursor of [undefined, null, ""]) expect(pageCommandOpaqueString({cursor}, "cursor")).toBeUndefined();
    const cursor = "  \ud800opaque  ";
    expect(pageCommandOpaqueString({cursor}, "cursor")).toBe(cursor);
    expect(() => pageCommandOpaqueString({cursor: 1}, "cursor")).toThrow("cursor must be a string");
  });
  it("preserves page input case-sensitive direction and host exceptions", () => {
    expect(pageCommandDirection({})).toBe("outgoing");
    expect(pageCommandDirection({direction: " incoming "})).toBe("incoming");
    expect(() => pageCommandDirection({direction: "Outgoing"})).toThrow("direction must be outgoing, incoming, or either");
    const failure = {host: true};
    try { pageCommandDirection({get direction() { throw failure; }}); throw new Error("getter did not throw"); }
    catch (error) { expect(error).toBe(failure); }
  });
});


describe("maintained interpretation comparison projection", () => {
  it("keeps source reading identities and existing bounded contested review posture", () => {
    const challenges = Array.from({length: 10}, (_, index) => ({id: `reading:${index}`}));
    const context = Array.from({length: 11}, (_, index) => ({id: `context:${index}`}));
    const gaps = Array.from({length: 14}, (_, index) => `gap:${index}`);
    const selection = {id: "selected", kind: "node"};
    const result = buildInterpretationComparison({challenge_relations: challenges, context_relations: context,
      gaps, conclusion: {can_conclude: true}}, selection, () => "en", (value: unknown) => value);
    expect(result.selection).toBe(selection);
    expect(result.schema).toBe("tos_interpretation_comparison_v1");
    expect(result.posture).toBe("contested_review_required");
    expect(result.can_conclude).toBe(true);
    expect(result.competing_reading_count).toBe(10);
    expect(result.competing_readings).toEqual(challenges.slice(0, 8));
    expect(result.competing_readings[0]).toBe(challenges[0]);
    expect(result.contextual_readings).toEqual(context.slice(0, 8));
    expect(result.gaps).toEqual(gaps.slice(0, 12));
    expect(result.authority_note).toBe("Projected challenge relations are review leads, not adjudicated counterevidence or canon decisions.");
  });
  it("preserves lazy posture and localized gap observations and native host exceptions", () => {
    const reads: string[] = [], ruGaps: string[] = [], authority = {opaque: true};
    const packet = {
      get challenge_relations() {reads.push("challenge"); return [];},
      get context_relations() {reads.push("context"); return [];},
      get posture() {reads.push("posture"); return "";},
      get selection_posture() {reads.push("selection"); return {review_posture: "source-review"};},
      get gaps_ru() {reads.push("gaps_ru"); return ruGaps;},
      get gaps() {throw new Error("truthy empty localized array must not fall back");},
      get conclusion() {reads.push("conclusion"); return {can_conclude: 1};},
      get authority_note() {reads.push("authority"); return authority;},
    };
    const result = buildInterpretationComparison(packet, {}, () => {reads.push("language"); return "ru";}, (value: unknown) => value);
    expect(reads).toEqual(["challenge", "context", "posture", "selection", "language", "gaps_ru", "conclusion", "authority"]);
    expect(result.posture).toBe("source-review");
    expect(result.can_conclude).toBe(false);
    expect(result.gaps).toEqual([]);
    expect(result.authority_note).toBe(authority);
    const refusal = {host: "original"};
    let observed;
    try {buildInterpretationComparison({get challenge_relations() {throw refusal;}}, {}, () => "en", () => null);}
    catch (error) {observed = error;}
    expect(observed).toBe(refusal);
  });
  it("keeps retained native map callbacks independent of the freed rule session", () => {
    let retained: ((value: unknown) => unknown) | undefined;
    const observations: string[] = [], sliceResult = {opaque: "mapped result"};
    let lengthReads = 0;
    const mapped = {
      get length() {observations.push("length"); return lengthReads++ === 0 ? 0 : 7;},
      slice(start: number, end: number) {observations.push(`slice:${start}:${end}`); return sliceResult;},
    };
    const source = {map(callback: (value: unknown) => unknown) {retained = callback; return mapped;}};
    const summarize = (value: unknown) => value;
    const result = buildInterpretationComparison({challenge_relations: source, context_relations: [], gaps: []}, {}, () => "en", summarize);
    expect(result.posture).toBe("review_status_unresolved");
    expect(result.competing_reading_count).toBe(7);
    expect(result.competing_readings).toBe(sliceResult);
    expect(observations).toEqual(["length", "length", "slice:0:8"]);
    const original = {id: "later"};
    expect(retained?.(original)).toBe(original);
  });
});
