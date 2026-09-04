/** Session-local research state. This module has no DOM, network, or ToS writeback. */
export const RESEARCH_WORKSPACE_SCHEMA = "tos_research_workspace_session_v1" as const;
export const RESEARCH_WORKSPACE_VERSION = 1 as const;
const MAX_ID = 256; const MAX_LABEL = 512; const MAX_TEXT = 4_000; const MAX_REF = 1_024; const MAX_ITEMS = 256;
const MAX_ROUTE_ITEMS = 512; const MAX_JOURNAL = 512; const MAX_REVISION = 1_000_000_000; const MAX_PACKET = 1_000_000; const MAX_HISTORY = 100;

export type WorkspaceSelectionKind = "node" | "edge" | "cluster" | "item";
export type WorkspaceSelection = { id: string; kind: WorkspaceSelectionKind; label?: string };
export type HypothesisPosture = { session_hypothesis: true; source: false; reviewed: false; canon: false };
export type ResearchHypothesis = { id: string; title: string; body: string; targetId?: string; fromId?: string; toId?: string; predicateLabel?: string; posture: HypothesisPosture };
export type ResearchProposalKind = "relation" | "interpretation" | "metadata_correction" | "source_route" | "concept_enrichment";
export type ProposalConfidenceValue = "unknown" | "low" | "medium" | "high";
export type ProposalConfidencePosture = { value: ProposalConfidenceValue; meaning: "maker_declared_uncertainty_not_truth_probability" };
export type ResearchProposal = {
  id: string; kind: ResearchProposalKind; parentHypothesisId: string; targetId?: string; fromId?: string; toId?: string;
  statement: string; sourceRefs: string[]; evidenceRefs: string[]; confidencePosture: ProposalConfidencePosture;
  actorOrigin: "human" | "agent"; basePageRevision: number; baseWorkspaceRevision: number; dataFingerprint: string;
  createdAt: string; digest: string; localOnly: true; reviewStatus: "pending_human_review"; canon: false;
};
export type ResearchProposalInput = {
  id: string; kind: ResearchProposalKind; parentHypothesisId: string; targetId?: string; fromId?: string; toId?: string;
  statement: string; sourceRefs: string[]; evidenceRefs: string[]; confidencePosture: ProposalConfidencePosture;
  actorOrigin: "human" | "agent"; basePageRevision: number; baseWorkspaceRevision: number; dataFingerprint: string; createdAt?: string;
};
export type RouteSnapshot = { id: string; label: string; fromId: string; toId: string; nodeIds: string[]; edgeIds: string[] };
export type WorkspaceNote = { id: string; body: string; targetId?: string };
export type JsonScalar = string | number | boolean | null;
export type JournalDetail = Record<string, JsonScalar>;
export type WorkspaceJournalEntry = { sequence: number; action: string; targetId?: string; detail?: JournalDetail };
export type ResearchWorkspaceState = {
  schema: typeof RESEARCH_WORKSPACE_SCHEMA; version: typeof RESEARCH_WORKSPACE_VERSION; sessionId: string; revision: number;
  selectedLens: WorkspaceSelection | null; excludedEdgeIds: string[]; routeSnapshots: RouteSnapshot[];
  hypotheses: ResearchHypothesis[]; proposals: ResearchProposal[]; notes: WorkspaceNote[]; journal: WorkspaceJournalEntry[];
};
export type ResearchWorkspacePersistence = { load: () => string | null; save: (packet: string) => void };
export type ResearchWorkspaceOptions = { sessionId?: string; historyLimit?: number; persistence?: ResearchWorkspacePersistence | false };
export type WorkspaceListener = (state: ResearchWorkspaceState) => void;
export type ResearchWorkspaceSummary = { schema: "tos_research_workspace_summary_v1"; session_id: string; revision: number; hypothesis_count: number; proposal_count: number; excluded_edge_count: number; comparison_count: number; note_count: number; journal_count: number; can_undo: boolean; can_redo: boolean };

const POSTURE: HypothesisPosture = { session_hypothesis: true, source: false, reviewed: false, canon: false };
const isRecord = (value: unknown): value is Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value); return prototype === Object.prototype || prototype === null;
};
const keys = (value: Record<string, unknown>): string[] => Object.keys(value).sort();
const only = (value: Record<string, unknown>, required: string[], optional: string[] = []): boolean => {
  const allowed = new Set([...required, ...optional]);
  return required.every((key) => Object.prototype.hasOwnProperty.call(value, key)) && keys(value).every((key) => allowed.has(key));
};
function stringValue(value: unknown, name: string, maximum: number, required = true): string | undefined {
  if (typeof value !== "string") { if (!required && value === undefined) return undefined; throw new Error(`${name} must be a string`); }
  const result = value.trim(); if (required && !result) throw new Error(`${name} is required`);
  if (result.length > maximum) throw new Error(`${name} exceeds ${maximum} characters`); return result || undefined;
}
const idValue = (value: unknown, name: string): string => stringValue(value, name, MAX_ID) as string;
function listValue(value: unknown, name: string, maximum: number): string[] {
  if (!Array.isArray(value)) throw new Error(`${name} must be an array`); if (value.length > maximum) throw new Error(`${name} exceeds ${maximum} items`);
  const result = value.map((item, index) => idValue(item, `${name}[${index}]`)); if (new Set(result).size !== result.length) throw new Error(`${name} must not contain duplicates`); return result;
}
function refListValue(value: unknown, name: string): string[] {
  if (!Array.isArray(value)) throw new Error(`${name} must be an array`); if (value.length < 1) throw new Error(`${name} must contain at least one reference`); if (value.length > MAX_ITEMS) throw new Error(`${name} exceeds ${MAX_ITEMS} items`);
  const result = value.map((item, index) => stringValue(item, `${name}[${index}]`, MAX_REF) as string); if (new Set(result).size !== result.length) throw new Error(`${name} must not contain duplicates`); return result;
}
function integerValue(value: unknown, name: string, maximum: number, minimum = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) throw new Error(`${name} must be an integer between ${minimum} and ${maximum}`); return value;
}
function cloneState(state: ResearchWorkspaceState): ResearchWorkspaceState {
  return { ...state, selectedLens: state.selectedLens ? { ...state.selectedLens } : null, excludedEdgeIds: [...state.excludedEdgeIds],
    routeSnapshots: state.routeSnapshots.map((route) => ({ ...route, nodeIds: [...route.nodeIds], edgeIds: [...route.edgeIds] })),
    hypotheses: state.hypotheses.map((item) => ({ ...item, posture: { ...item.posture } })),
    proposals: state.proposals.map((item) => ({ ...item, sourceRefs: [...item.sourceRefs], evidenceRefs: [...item.evidenceRefs], confidencePosture: { ...item.confidencePosture } })),
    notes: state.notes.map((item) => ({ ...item })),
    journal: state.journal.map((item) => ({ ...item, detail: item.detail ? { ...item.detail } : undefined })) };
}
function stableStringify(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  if (isRecord(value)) return `{${keys(value).map((key) => `${JSON.stringify(key)}:${stableStringify(value[key])}`).join(",")}}`;
  return JSON.stringify(value);
}
/** Synchronous, runtime-portable digest for local traceability; not a cryptographic signature. */
function deterministicDigest(value: string): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  for (const byte of new TextEncoder().encode(value)) { hash ^= BigInt(byte); hash = BigInt.asUintN(64, hash * prime); }
  return `fnv1a64:${hash.toString(16).padStart(16, "0")}`;
}
function packetOf(state: ResearchWorkspaceState): Record<string, unknown> {
  return { schema: RESEARCH_WORKSPACE_SCHEMA, version: RESEARCH_WORKSPACE_VERSION, session_id: state.sessionId, revision: state.revision,
    selected_lens: state.selectedLens ? { ...state.selectedLens } : null, excluded_edge_ids: [...state.excludedEdgeIds],
    route_snapshots: state.routeSnapshots.map((route) => ({ id: route.id, label: route.label, from_id: route.fromId, to_id: route.toId, node_ids: [...route.nodeIds], edge_ids: [...route.edgeIds] })),
    hypotheses: state.hypotheses.map((item) => ({ id: item.id, title: item.title, body: item.body, ...(item.targetId ? { target_id: item.targetId } : {}), ...(item.fromId ? { from_id: item.fromId } : {}), ...(item.toId ? { to_id: item.toId } : {}), ...(item.predicateLabel ? { predicate_label: item.predicateLabel } : {}), posture: { ...POSTURE } })),
    proposals: state.proposals.map((item) => proposalPacketOf(item)),
    notes: state.notes.map((item) => ({ id: item.id, body: item.body, ...(item.targetId ? { target_id: item.targetId } : {}) })),
    journal: state.journal.map((item) => ({ sequence: item.sequence, action: item.action, ...(item.targetId ? { target_id: item.targetId } : {}), ...(item.detail ? { detail: { ...item.detail } } : {}) })) };
}
function selectionOf(value: unknown): WorkspaceSelection | null {
  if (value === null) return null; if (!isRecord(value) || !only(value, ["id", "kind"], ["label"])) throw new Error("selected_lens is invalid");
  const kind = stringValue(value.kind, "selected_lens.kind", MAX_ID) as string; if (!["node", "edge", "cluster", "item"].includes(kind)) throw new Error("selected_lens.kind is invalid");
  const label = stringValue(value.label, "selected_lens.label", MAX_LABEL, false); return { id: idValue(value.id, "selected_lens.id"), kind: kind as WorkspaceSelectionKind, ...(label ? { label } : {}) };
}
function hypothesisOf(value: unknown, index: number): ResearchHypothesis {
  if (!isRecord(value) || !only(value, ["id", "title", "body", "posture"], ["target_id", "from_id", "to_id", "predicate_label"])) throw new Error(`hypotheses[${index}] is invalid`);
  if (!isRecord(value.posture) || !only(value.posture, ["session_hypothesis", "source", "reviewed", "canon"]) || value.posture.session_hypothesis !== true || value.posture.source !== false || value.posture.reviewed !== false || value.posture.canon !== false) throw new Error(`hypotheses[${index}] posture must remain session-local`);
  const targetId = stringValue(value.target_id, `hypotheses[${index}].target_id`, MAX_ID, false); const fromId = stringValue(value.from_id, `hypotheses[${index}].from_id`, MAX_ID, false); const toId = stringValue(value.to_id, `hypotheses[${index}].to_id`, MAX_ID, false); if ((fromId === undefined) !== (toId === undefined)) throw new Error(`hypotheses[${index}] from_id and to_id must be paired`); const predicateLabel = stringValue(value.predicate_label, `hypotheses[${index}].predicate_label`, MAX_LABEL, false); return { id: idValue(value.id, `hypotheses[${index}].id`), title: stringValue(value.title, `hypotheses[${index}].title`, MAX_LABEL) as string, body: stringValue(value.body, `hypotheses[${index}].body`, MAX_TEXT) as string, ...(targetId ? { targetId } : {}), ...(fromId ? { fromId } : {}), ...(toId ? { toId } : {}), ...(predicateLabel ? { predicateLabel } : {}), posture: { ...POSTURE } };
}
function proposalPacketOf(proposal: ResearchProposal, includeDigest = true): Record<string, unknown> {
  return {
    id: proposal.id, kind: proposal.kind, parent_hypothesis_id: proposal.parentHypothesisId,
    ...(proposal.targetId ? { target_id: proposal.targetId } : {}), ...(proposal.fromId ? { from_id: proposal.fromId } : {}),
    ...(proposal.toId ? { to_id: proposal.toId } : {}), statement: proposal.statement, source_refs: [...proposal.sourceRefs],
    evidence_refs: [...proposal.evidenceRefs], confidence_posture: { ...proposal.confidencePosture }, actor_origin: proposal.actorOrigin,
    base_page_revision: proposal.basePageRevision, base_workspace_revision: proposal.baseWorkspaceRevision,
    data_fingerprint: proposal.dataFingerprint, created_at: proposal.createdAt, local_only: true,
    review_status: "pending_human_review", canon: false, ...(includeDigest ? { digest: proposal.digest } : {}),
  };
}
function proposalDigestOf(proposal: ResearchProposal): string { return deterministicDigest(stableStringify(proposalPacketOf(proposal, false))); }
function createdAtValue(value: unknown, name: string): string {
  const result = stringValue(value, name, 64) as string;
  const date = new Date(result);
  if (!Number.isFinite(date.getTime()) || date.toISOString() !== result) throw new Error(`${name} must be a canonical UTC ISO timestamp`);
  return result;
}
function confidencePostureOf(value: unknown, name: string): ProposalConfidencePosture {
  if (!isRecord(value) || !only(value, ["value", "meaning"])) throw new Error(`${name} is invalid`);
  const postureValue = stringValue(value.value, `${name}.value`, MAX_ID) as ProposalConfidenceValue;
  if (!["unknown", "low", "medium", "high"].includes(postureValue)) throw new Error(`${name}.value is invalid`);
  if (value.meaning !== "maker_declared_uncertainty_not_truth_probability") throw new Error(`${name}.meaning is invalid`);
  return { value: postureValue, meaning: "maker_declared_uncertainty_not_truth_probability" };
}
function validateProposalTargets(kind: ResearchProposalKind, targetId: string | undefined, fromId: string | undefined, toId: string | undefined, name: string): void {
  if ((kind === "relation" || kind === "source_route") && (fromId === undefined || toId === undefined)) throw new Error(`${name} requires fromId and toId`);
  if ((kind === "interpretation" || kind === "metadata_correction" || kind === "concept_enrichment") && targetId === undefined) throw new Error(`${name} requires targetId`);
}
function proposalOf(value: unknown, index: number): ResearchProposal {
  const prefix = `proposals[${index}]`;
  const required = ["id", "kind", "parent_hypothesis_id", "statement", "source_refs", "evidence_refs", "confidence_posture", "actor_origin", "base_page_revision", "base_workspace_revision", "data_fingerprint", "created_at", "local_only", "review_status", "canon", "digest"];
  const optional = ["target_id", "from_id", "to_id"];
  if (!isRecord(value) || !only(value, required, optional)) throw new Error(`${prefix} is invalid`);
  const kind = stringValue(value.kind, `${prefix}.kind`, MAX_ID) as ResearchProposalKind;
  if (!["relation", "interpretation", "metadata_correction", "source_route", "concept_enrichment"].includes(kind)) throw new Error(`${prefix}.kind is invalid`);
  const targetId = stringValue(value.target_id, `${prefix}.target_id`, MAX_ID, false); const fromId = stringValue(value.from_id, `${prefix}.from_id`, MAX_ID, false); const toId = stringValue(value.to_id, `${prefix}.to_id`, MAX_ID, false);
  if ((fromId === undefined) !== (toId === undefined)) throw new Error(`${prefix} from_id and to_id must be paired`);
  validateProposalTargets(kind, targetId, fromId, toId, prefix);
  if (value.local_only !== true || value.review_status !== "pending_human_review" || value.canon !== false) throw new Error(`${prefix} posture must remain local and pending human review`);
  const actorOrigin = stringValue(value.actor_origin, `${prefix}.actor_origin`, MAX_ID) as "human" | "agent";
  if (actorOrigin !== "human" && actorOrigin !== "agent") throw new Error(`${prefix}.actor_origin is invalid`);
  const proposal: ResearchProposal = {
    id: idValue(value.id, `${prefix}.id`), kind, parentHypothesisId: idValue(value.parent_hypothesis_id, `${prefix}.parent_hypothesis_id`),
    ...(targetId ? { targetId } : {}), ...(fromId ? { fromId } : {}), ...(toId ? { toId } : {}), statement: stringValue(value.statement, `${prefix}.statement`, MAX_TEXT) as string,
    sourceRefs: refListValue(value.source_refs, `${prefix}.source_refs`), evidenceRefs: refListValue(value.evidence_refs, `${prefix}.evidence_refs`),
    confidencePosture: confidencePostureOf(value.confidence_posture, `${prefix}.confidence_posture`), actorOrigin,
    basePageRevision: integerValue(value.base_page_revision, `${prefix}.base_page_revision`, MAX_REVISION), baseWorkspaceRevision: integerValue(value.base_workspace_revision, `${prefix}.base_workspace_revision`, MAX_REVISION),
    dataFingerprint: stringValue(value.data_fingerprint, `${prefix}.data_fingerprint`, MAX_REF) as string, createdAt: createdAtValue(value.created_at, `${prefix}.created_at`),
    digest: idValue(value.digest, `${prefix}.digest`), localOnly: true, reviewStatus: "pending_human_review", canon: false,
  };
  if (proposalDigestOf(proposal) !== proposal.digest) throw new Error(`${prefix} digest mismatch`);
  return proposal;
}
function routeOf(value: unknown, index: number): RouteSnapshot {
  if (!isRecord(value) || !only(value, ["id", "label", "from_id", "to_id", "node_ids", "edge_ids"])) throw new Error(`route_snapshots[${index}] is invalid`);
  return { id: idValue(value.id, `route_snapshots[${index}].id`), label: stringValue(value.label, `route_snapshots[${index}].label`, MAX_LABEL) as string, fromId: idValue(value.from_id, `route_snapshots[${index}].from_id`), toId: idValue(value.to_id, `route_snapshots[${index}].to_id`), nodeIds: listValue(value.node_ids, `route_snapshots[${index}].node_ids`, MAX_ROUTE_ITEMS), edgeIds: listValue(value.edge_ids, `route_snapshots[${index}].edge_ids`, MAX_ROUTE_ITEMS) };
}
function noteOf(value: unknown, index: number): WorkspaceNote {
  if (!isRecord(value) || !only(value, ["id", "body"], ["target_id"])) throw new Error(`notes[${index}] is invalid`);
  const targetId = stringValue(value.target_id, `notes[${index}].target_id`, MAX_ID, false); return { id: idValue(value.id, `notes[${index}].id`), body: stringValue(value.body, `notes[${index}].body`, MAX_TEXT) as string, ...(targetId ? { targetId } : {}) };
}
function journalOf(value: unknown): WorkspaceJournalEntry[] {
  if (!Array.isArray(value) || value.length > MAX_JOURNAL) throw new Error(`journal must be an array of at most ${MAX_JOURNAL} items`); let previous = 0;
  return value.map((raw, index) => { if (!isRecord(raw) || !only(raw, ["sequence", "action"], ["target_id", "detail"])) throw new Error(`journal[${index}] is invalid`);
    const sequence = integerValue(raw.sequence, `journal[${index}].sequence`, MAX_REVISION, 1); if (sequence <= previous) throw new Error("journal sequence must be increasing"); previous = sequence;
    const targetId = stringValue(raw.target_id, `journal[${index}].target_id`, MAX_ID, false); let detail: JournalDetail | undefined;
    if (raw.detail !== undefined) { if (!isRecord(raw.detail) || keys(raw.detail).length > 32) throw new Error(`journal[${index}].detail is invalid`); detail = {};
      for (const key of keys(raw.detail)) { idValue(key, `journal[${index}].detail key`); const item = raw.detail[key]; if (item !== null && typeof item !== "string" && typeof item !== "number" && typeof item !== "boolean") throw new Error(`journal[${index}].detail values must be scalar`); if (typeof item === "number" && !Number.isFinite(item)) throw new Error(`journal[${index}].detail value is invalid`); detail[key] = item as JsonScalar; }
    }
    return { sequence, action: idValue(raw.action, `journal[${index}].action`), ...(targetId ? { targetId } : {}), ...(detail ? { detail } : {}) };
  });
}
function stateOf(packet: unknown): ResearchWorkspaceState {
  const required = ["schema", "version", "session_id", "revision", "selected_lens", "excluded_edge_ids", "route_snapshots", "hypotheses", "notes", "journal"];
  if (!isRecord(packet) || !only(packet, required, ["proposals"]) || packet.schema !== RESEARCH_WORKSPACE_SCHEMA || packet.version !== RESEARCH_WORKSPACE_VERSION) throw new Error("packet shape or schema is invalid");
  const routes = Array.isArray(packet.route_snapshots) ? packet.route_snapshots.map(routeOf) : (() => { throw new Error("route_snapshots must be an array"); })();
  const hypotheses = Array.isArray(packet.hypotheses) ? packet.hypotheses.map(hypothesisOf) : (() => { throw new Error("hypotheses must be an array"); })();
  const proposals = packet.proposals === undefined ? [] : Array.isArray(packet.proposals) ? packet.proposals.map(proposalOf) : (() => { throw new Error("proposals must be an array"); })();
  const notes = Array.isArray(packet.notes) ? packet.notes.map(noteOf) : (() => { throw new Error("notes must be an array"); })();
  if (routes.length > MAX_ITEMS || hypotheses.length > MAX_ITEMS || proposals.length > MAX_ITEMS || notes.length > MAX_ITEMS) throw new Error(`collections exceed ${MAX_ITEMS} items`);
  if (new Set(routes.map((item) => item.id)).size !== routes.length || new Set(hypotheses.map((item) => item.id)).size !== hypotheses.length || new Set(proposals.map((item) => item.id)).size !== proposals.length || new Set(notes.map((item) => item.id)).size !== notes.length) throw new Error("collection IDs must be unique");
  const hypothesisIds = new Set(hypotheses.map((item) => item.id));
  if (proposals.some((item) => !hypothesisIds.has(item.parentHypothesisId))) throw new Error("proposal parent hypothesis does not exist");
  return { schema: RESEARCH_WORKSPACE_SCHEMA, version: RESEARCH_WORKSPACE_VERSION, sessionId: idValue(packet.session_id, "session_id"), revision: integerValue(packet.revision, "revision", MAX_REVISION), selectedLens: selectionOf(packet.selected_lens), excludedEdgeIds: listValue(packet.excluded_edge_ids, "excluded_edge_ids", MAX_ITEMS), routeSnapshots: routes, hypotheses, proposals, notes, journal: journalOf(packet.journal) };
}
function parsePacket(packet: string): ResearchWorkspaceState { if (typeof packet !== "string" || packet.length > MAX_PACKET) throw new Error("invalid research workspace packet: packet is too large"); try { return stateOf(JSON.parse(packet)); } catch (error) { throw new Error(`invalid research workspace packet: ${error instanceof Error ? error.message : "invalid JSON"}`); } }
function emptyState(sessionId: string): ResearchWorkspaceState { return { schema: RESEARCH_WORKSPACE_SCHEMA, version: RESEARCH_WORKSPACE_VERSION, sessionId, revision: 0, selectedLens: null, excludedEdgeIds: [], routeSnapshots: [], hypotheses: [], proposals: [], notes: [], journal: [] }; }
function historyLimitOf(value: number | undefined): number { if (value === undefined) return 50; if (!Number.isSafeInteger(value) || value < 1 || value > MAX_HISTORY) throw new Error(`historyLimit must be an integer between 1 and ${MAX_HISTORY}`); return value; }

export function createLocalStoragePersistence(storage: Storage, key: string): ResearchWorkspacePersistence { const safeKey = idValue(key, "persistence key"); return { load: () => storage.getItem(safeKey), save: (packet) => storage.setItem(safeKey, packet) }; }
export function createResearchWorkspace(options: ResearchWorkspaceOptions = {}) {
  const sessionId = idValue(options.sessionId ?? "local", "sessionId"); const limit = historyLimitOf(options.historyLimit); const persistence = options.persistence === false ? null : options.persistence;
  let state = emptyState(sessionId); let persistenceError: string | null = null; const undoStack: ResearchWorkspaceState[] = []; const redoStack: ResearchWorkspaceState[] = []; const listeners = new Set<WorkspaceListener>();
  const persist = (): void => { if (!persistence) return; try { persistence.save(stableStringify(packetOf(state))); persistenceError = null; } catch (error) { persistenceError = error instanceof Error ? error.message : String(error); } };
  const notify = (): void => { const current = cloneState(state); listeners.forEach((listener) => listener(current)); };
  if (persistence) { try { const stored = persistence.load(); if (stored) state = parsePacket(stored); } catch (error) { persistenceError = error instanceof Error ? error.message : String(error); } }
  const journal = (draft: ResearchWorkspaceState, action: string, targetId?: string, detail?: JournalDetail): void => { const sequence = (draft.journal.at(-1)?.sequence ?? 0) + 1; draft.journal.push({ sequence, action, ...(targetId ? { targetId } : {}), ...(detail ? { detail: { ...detail } } : {}) }); if (draft.journal.length > MAX_JOURNAL) draft.journal.splice(0, draft.journal.length - MAX_JOURNAL); };
  const push = (stack: ResearchWorkspaceState[], item: ResearchWorkspaceState): void => { stack.push(cloneState(item)); if (stack.length > limit) stack.splice(0, stack.length - limit); };
  const commit = (action: string, mutate: (draft: ResearchWorkspaceState) => void, targetId?: string): ResearchWorkspaceState => { push(undoStack, state); const next = cloneState(state); mutate(next); next.revision = Math.min(MAX_REVISION, state.revision + 1); journal(next, action, targetId); state = next; redoStack.splice(0); persist(); notify(); return cloneState(state); };
  const transition = (from: ResearchWorkspaceState[], to: ResearchWorkspaceState[], action: string): boolean => { const target = from.pop(); if (!target) return false; push(to, state); const next = cloneState(target); next.revision = Math.min(MAX_REVISION, state.revision + 1); journal(next, action); state = next; persist(); notify(); return true; };
  return {
    getState: (): ResearchWorkspaceState => cloneState(state), subscribe: (listener: WorkspaceListener): (() => void) => { listeners.add(listener); return () => listeners.delete(listener); },
    persistenceEnabled: (): boolean => persistence !== null, persistenceError: (): string | null => persistenceError,
    summary: (): ResearchWorkspaceSummary => ({ schema: "tos_research_workspace_summary_v1", session_id: state.sessionId, revision: state.revision, hypothesis_count: state.hypotheses.length, proposal_count: state.proposals.length, excluded_edge_count: state.excludedEdgeIds.length, comparison_count: state.routeSnapshots.length, note_count: state.notes.length, journal_count: state.journal.length, can_undo: undoStack.length > 0, can_redo: redoStack.length > 0 }),
    exportPacket: (): string => stableStringify(packetOf(state)),
    importPacket: (packet: string): boolean => { state = parsePacket(packet); undoStack.splice(0); redoStack.splice(0); persist(); notify(); return true; },
    selectLens: (selection: WorkspaceSelection | null): ResearchWorkspaceState => commit("lens.select", (draft) => { draft.selectedLens = selection === null ? null : selectionOf(selection); }, selection?.id),
    excludeEdge: (edgeId: string): ResearchWorkspaceState => { const id = idValue(edgeId, "edgeId"); return state.excludedEdgeIds.includes(id) ? cloneState(state) : commit("edge.exclude", (draft) => { draft.excludedEdgeIds.push(id); }, id); },
    includeEdge: (edgeId: string): ResearchWorkspaceState => { const id = idValue(edgeId, "edgeId"); return !state.excludedEdgeIds.includes(id) ? cloneState(state) : commit("edge.include", (draft) => { draft.excludedEdgeIds = draft.excludedEdgeIds.filter((candidate) => candidate !== id); }, id); },
    addHypothesis: (input: { id: string; title: string; body: string; targetId?: string; fromId?: string; toId?: string; predicateLabel?: string }): ResearchHypothesis => { const id = idValue(input.id, "hypothesis.id"); if (state.hypotheses.some((item) => item.id === id)) throw new Error(`hypothesis already exists: ${id}`); const targetId = stringValue(input.targetId, "hypothesis.targetId", MAX_ID, false); const fromId = stringValue(input.fromId, "hypothesis.fromId", MAX_ID, false); const toId = stringValue(input.toId, "hypothesis.toId", MAX_ID, false); if ((fromId === undefined) !== (toId === undefined)) throw new Error("hypothesis.fromId and hypothesis.toId must be paired"); const predicateLabel = stringValue(input.predicateLabel, "hypothesis.predicateLabel", MAX_LABEL, false); const result: ResearchHypothesis = { id, title: stringValue(input.title, "hypothesis.title", MAX_LABEL) as string, body: stringValue(input.body, "hypothesis.body", MAX_TEXT) as string, ...(targetId ? { targetId } : {}), ...(fromId ? { fromId } : {}), ...(toId ? { toId } : {}), ...(predicateLabel ? { predicateLabel } : {}), posture: { ...POSTURE } }; commit("hypothesis.add", (draft) => { draft.hypotheses.push(result); }, id); return { ...result, posture: { ...result.posture } }; },
    stageProposal: (input: ResearchProposalInput): ResearchProposal => {
      const id = idValue(input.id, "proposal.id");
      if (state.proposals.some((item) => item.id === id)) throw new Error(`proposal already exists: ${id}`);
      if (!state.hypotheses.some((item) => item.id === input.parentHypothesisId)) throw new Error(`proposal parent hypothesis does not exist: ${input.parentHypothesisId}`);
      if (input.baseWorkspaceRevision !== state.revision) throw new Error(`proposal workspace revision is stale: expected ${state.revision}`);
      const targetId = stringValue(input.targetId, "proposal.targetId", MAX_ID, false); const fromId = stringValue(input.fromId, "proposal.fromId", MAX_ID, false); const toId = stringValue(input.toId, "proposal.toId", MAX_ID, false);
      if ((fromId === undefined) !== (toId === undefined)) throw new Error("proposal.fromId and proposal.toId must be paired");
      if (!["relation", "interpretation", "metadata_correction", "source_route", "concept_enrichment"].includes(input.kind)) throw new Error("proposal.kind is invalid");
      validateProposalTargets(input.kind, targetId, fromId, toId, "proposal");
      const draft: ResearchProposal = {
        id, kind: input.kind, parentHypothesisId: idValue(input.parentHypothesisId, "proposal.parentHypothesisId"), ...(targetId ? { targetId } : {}), ...(fromId ? { fromId } : {}), ...(toId ? { toId } : {}),
        statement: stringValue(input.statement, "proposal.statement", MAX_TEXT) as string, sourceRefs: refListValue(input.sourceRefs, "proposal.sourceRefs"), evidenceRefs: refListValue(input.evidenceRefs, "proposal.evidenceRefs"),
        confidencePosture: confidencePostureOf(input.confidencePosture, "proposal.confidencePosture"), actorOrigin: input.actorOrigin, basePageRevision: integerValue(input.basePageRevision, "proposal.basePageRevision", MAX_REVISION), baseWorkspaceRevision: integerValue(input.baseWorkspaceRevision, "proposal.baseWorkspaceRevision", MAX_REVISION),
        dataFingerprint: stringValue(input.dataFingerprint, "proposal.dataFingerprint", MAX_REF) as string, createdAt: createdAtValue(input.createdAt ?? new Date().toISOString(), "proposal.createdAt"), digest: "", localOnly: true, reviewStatus: "pending_human_review", canon: false,
      };
      if (draft.actorOrigin !== "human" && draft.actorOrigin !== "agent") throw new Error("proposal.actorOrigin is invalid");
      draft.digest = proposalDigestOf(draft);
      commit("proposal.stage", (next) => { next.proposals.push(draft); }, id);
      return { ...draft, sourceRefs: [...draft.sourceRefs], evidenceRefs: [...draft.evidenceRefs], confidencePosture: { ...draft.confidencePosture } };
    },
    removeHypothesis: (id: string): ResearchWorkspaceState => { const targetId = idValue(id, "hypothesis.id"); if (state.proposals.some((item) => item.parentHypothesisId === targetId)) throw new Error(`hypothesis has staged proposals: ${targetId}`); return !state.hypotheses.some((item) => item.id === targetId) ? cloneState(state) : commit("hypothesis.remove", (draft) => { draft.hypotheses = draft.hypotheses.filter((item) => item.id !== targetId); }, targetId); },
    saveRouteSnapshot: (input: RouteSnapshot): ResearchWorkspaceState => { const route: RouteSnapshot = { id: idValue(input.id, "route.id"), label: stringValue(input.label, "route.label", MAX_LABEL) as string, fromId: idValue(input.fromId, "route.fromId"), toId: idValue(input.toId, "route.toId"), nodeIds: listValue(input.nodeIds, "route.nodeIds", MAX_ROUTE_ITEMS), edgeIds: listValue(input.edgeIds, "route.edgeIds", MAX_ROUTE_ITEMS) }; return commit("route.snapshot", (draft) => { const index = draft.routeSnapshots.findIndex((item) => item.id === route.id); if (index < 0) draft.routeSnapshots.push(route); else draft.routeSnapshots[index] = route; }, route.id); },
    removeRouteSnapshot: (id: string): ResearchWorkspaceState => { const targetId = idValue(id, "route.id"); return !state.routeSnapshots.some((item) => item.id === targetId) ? cloneState(state) : commit("route.remove", (draft) => { draft.routeSnapshots = draft.routeSnapshots.filter((item) => item.id !== targetId); }, targetId); },
    addNote: (input: { id: string; body: string; targetId?: string }): ResearchWorkspaceState => { const id = idValue(input.id, "note.id"); if (state.notes.some((item) => item.id === id)) throw new Error(`note already exists: ${id}`); const targetId = stringValue(input.targetId, "note.targetId", MAX_ID, false); return commit("note.add", (draft) => { draft.notes.push({ id, body: stringValue(input.body, "note.body", MAX_TEXT) as string, ...(targetId ? { targetId } : {}) }); }, id); },
    updateNote: (input: { id: string; body: string; targetId?: string }): ResearchWorkspaceState => { const id = idValue(input.id, "note.id"); if (!state.notes.some((item) => item.id === id)) throw new Error(`note does not exist: ${id}`); const body = stringValue(input.body, "note.body", MAX_TEXT) as string; const targetId = stringValue(input.targetId, "note.targetId", MAX_ID, false); return commit("note.update", (draft) => { const note = draft.notes.find((item) => item.id === id); if (note) { note.body = body; if (targetId) note.targetId = targetId; else delete note.targetId; } }, id); },
    removeNote: (id: string): ResearchWorkspaceState => { const targetId = idValue(id, "note.id"); return !state.notes.some((item) => item.id === targetId) ? cloneState(state) : commit("note.remove", (draft) => { draft.notes = draft.notes.filter((item) => item.id !== targetId); }, targetId); },
    canUndo: (): boolean => undoStack.length > 0, canRedo: (): boolean => redoStack.length > 0, undo: (): boolean => transition(undoStack, redoStack, "workspace.undo"), redo: (): boolean => transition(redoStack, undoStack, "workspace.redo"),
    clearHistory: (): void => { undoStack.splice(0); redoStack.splice(0); }, comparableRoutesReady: (): boolean => {
      const counts = new Map<string, number>();
      state.routeSnapshots.forEach((route) => { const key = `${route.fromId}\u0000${route.toId}`; counts.set(key, (counts.get(key) ?? 0) + 1); });
      return [...counts.values()].some((count) => count >= 2);
    },
  };
}
export type ResearchWorkspace = ReturnType<typeof createResearchWorkspace>;
