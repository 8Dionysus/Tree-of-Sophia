import { describe, expect, it, vi } from "vitest";
import { createLocalStoragePersistence, createResearchWorkspace, RESEARCH_WORKSPACE_SCHEMA, RESEARCH_WORKSPACE_VERSION, type ResearchWorkspacePersistence } from "./research-workspace";

describe("portable research workspace", () => {
  it("keeps hypotheses explicitly session-local and outside ToS authority", () => {
    const workspace = createResearchWorkspace({ sessionId: "demo" });
    const hypothesis = workspace.addHypothesis({ id: "hyp:alternate-reading", title: "An alternate reading", body: "Treat this relation as a local possibility.", targetId: "edge:contested", fromId: "node:a", toId: "node:b", predicateLabel: "interprets" });
    expect(hypothesis.posture).toEqual({ session_hypothesis: true, source: false, reviewed: false, canon: false });
    expect(workspace.getState().hypotheses[0]).toEqual(hypothesis);
    expect(workspace.getState().journal.at(-1)?.action).toBe("hypothesis.add");
    expect(workspace.summary()).toMatchObject({ schema: "tos_research_workspace_summary_v1", hypothesis_count: 1, can_undo: true, can_redo: false });
    expect(JSON.parse(workspace.exportPacket()).hypotheses[0]).toMatchObject({ from_id: "node:a", to_id: "node:b", predicate_label: "interprets" });
    expect(() => workspace.addHypothesis({ id: "hyp:unpaired", title: "Bad", body: "Bad", fromId: "node:a" })).toThrow(/must be paired/);
  });

  it("tracks excluded edges and two comparable route snapshots", () => {
    const workspace = createResearchWorkspace({ sessionId: "routes" });
    workspace.excludeEdge("edge:contested");
    workspace.saveRouteSnapshot({ id: "route:canonical", label: "Canonical route", fromId: "node:a", toId: "node:c", nodeIds: ["node:a", "node:b", "node:c"], edgeIds: ["edge:ab", "edge:bc"] });
    workspace.saveRouteSnapshot({ id: "route:unrelated", label: "Unrelated route", fromId: "node:x", toId: "node:y", nodeIds: ["node:x", "node:y"], edgeIds: ["edge:xy"] });
    expect(workspace.comparableRoutesReady()).toBe(false);
    workspace.saveRouteSnapshot({ id: "route:rerouted", label: "Rerouted route", fromId: "node:a", toId: "node:c", nodeIds: ["node:a", "node:d", "node:c"], edgeIds: ["edge:ad", "edge:dc"] });
    expect(workspace.getState().excludedEdgeIds).toEqual(["edge:contested"]);
    expect(workspace.getState().routeSnapshots.map((route) => route.id)).toEqual(["route:canonical", "route:unrelated", "route:rerouted"]);
    expect(workspace.comparableRoutesReady()).toBe(true);
  });

  it("supports notes, journaled actions, and bounded undo/redo", () => {
    const workspace = createResearchWorkspace({ sessionId: "history", historyLimit: 2 });
    workspace.addNote({ id: "note:one", body: "First note" }); workspace.addNote({ id: "note:two", body: "Second note" }); workspace.addNote({ id: "note:three", body: "Third note" });
    expect(workspace.undo()).toBe(true); expect(workspace.getState().notes.map((note) => note.id)).toEqual(["note:one", "note:two"]);
    expect(workspace.undo()).toBe(true); expect(workspace.getState().notes.map((note) => note.id)).toEqual(["note:one"]);
    expect(workspace.undo()).toBe(false); expect(workspace.redo()).toBe(true); expect(workspace.getState().notes.map((note) => note.id)).toEqual(["note:one", "note:two"]);
    expect(workspace.getState().journal.map((entry) => entry.action)).toContain("workspace.undo"); expect(workspace.getState().journal.map((entry) => entry.action)).toContain("workspace.redo");
  });

  it("exports deterministic versioned JSON and imports a validated packet", () => {
    const workspace = createResearchWorkspace({ sessionId: "packet" }); workspace.selectLens({ id: "node:z", kind: "node", label: "Zarathustra" }); workspace.addNote({ id: "note:z", body: "Inspect this route." });
    const first = workspace.exportPacket(); expect(first).toBe(workspace.exportPacket()); expect(JSON.parse(first)).toMatchObject({ schema: RESEARCH_WORKSPACE_SCHEMA, version: RESEARCH_WORKSPACE_VERSION, session_id: "packet" });
    const restored = createResearchWorkspace({ sessionId: "other" }); expect(restored.importPacket(first)).toBe(true); expect(restored.getState()).toEqual(workspace.getState());
    expect(() => restored.importPacket('{"schema":"wrong"}')).toThrow(/invalid research workspace packet/);
    expect(() => restored.importPacket('{"schema":"tos_research_workspace_session_v1","version":1}')).toThrow(/invalid research workspace packet/);
  });

  it("persists through an adapter and can be explicitly disabled", () => {
    const saved: string[] = []; const adapter: ResearchWorkspacePersistence = { load: vi.fn(() => saved.at(-1) ?? null), save: vi.fn((packet) => { saved.push(packet); }) };
    const workspace = createResearchWorkspace({ sessionId: "persisted", persistence: adapter }); workspace.addNote({ id: "note:persisted", body: "Keep locally." }); expect(adapter.save).toHaveBeenCalledTimes(1);
    const restored = createResearchWorkspace({ sessionId: "ignored", persistence: adapter }); expect(restored.getState().sessionId).toBe("persisted"); expect(restored.getState().notes[0].id).toBe("note:persisted");
    const disabled = createResearchWorkspace({ sessionId: "disabled", persistence: false }); disabled.addNote({ id: "note:disabled", body: "No persistence." }); expect(adapter.save).toHaveBeenCalledTimes(1); expect(disabled.persistenceEnabled()).toBe(false);
  });

  it("offers a Storage-backed local adapter without network calls", () => {
    const values = new Map<string, string>(); const storage: Storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => { values.set(key, value); }, removeItem: (key) => { values.delete(key); }, clear: () => { values.clear(); }, key: (index) => [...values.keys()][index] ?? null, get length() { return values.size; } };
    const adapter = createLocalStoragePersistence(storage, "tos:test"); const workspace = createResearchWorkspace({ sessionId: "storage", persistence: adapter }); workspace.addNote({ id: "note:storage", body: "Storage only." }); expect(storage.getItem("tos:test")).toBe(workspace.exportPacket());
  });
});
