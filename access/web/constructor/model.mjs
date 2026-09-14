/** Local construction state only. Library witnesses stay with their source owner. */
import { createResearchWorkspace } from '../src/research-workspace.ts';

export const CONSTRUCTOR_SCHEMA = 'tos_constructor_workspace_v1';
export const CONSTRUCTOR_LIMITS = Object.freeze({ nodes: 200, edges: 600, history: 64, bytes: 1_000_000 });
const SOURCE_KINDS = new Set(['work', 'part', 'chapter', 'fragment', 'dossier']);
const MATERIAL_KINDS = new Set([...SOURCE_KINDS, 'concept', 'interpretation', 'question', 'note', 'excerpt', 'figure', 'symbol', 'tradition', 'character']);
const DRAFT_KINDS = new Set(['concept', 'interpretation', 'question', 'note', 'excerpt']);
const EDGE_KINDS = new Set(['contains', 'supports', 'questions', 'relates', 'interprets', 'contrasts', 'echoes', 'develops', 'compares', 'translates']);
const LOCAL = Object.freeze({ localOnly: true, source: false, reviewed: false, canon: false });
const MAX_REVISION = 1_000_000_000;
const clone = (value) => JSON.parse(JSON.stringify(value));
const record = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
const own = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
const localText = (value) => typeof value === 'string' ? value : value?.ru ?? value?.en ?? '';

function shape(value, required, optional = [], name = 'value') {
  if (!record(value) || required.some((key) => !own(value, key)) || Object.keys(value).some((key) => !required.includes(key) && !optional.includes(key))) {
    throw new Error(`${name} has unsupported or missing fields`);
  }
}
function text(value, name, maximum = 256, empty = false) {
  if (typeof value !== 'string' || value.length > maximum || (!empty && !value.trim())) throw new Error(`${name} must be ${empty ? '' : 'nonempty '}text of at most ${maximum} characters`);
  return value;
}
function position(value) {
  if (!Array.isArray(value) || value.length !== 3 || value.some((coordinate) => typeof coordinate !== 'number' || !Number.isFinite(coordinate) || Math.abs(coordinate) > 100_000)) {
    throw new Error('position must contain three finite coordinates between -100000 and 100000');
  }
  return [...value];
}
function boundedPacket(packet) {
  // Account for UTF-16 storage as well as UTF-8 export, including Cyrillic text.
  if (typeof packet !== 'string' || packet.length * 2 > CONSTRUCTOR_LIMITS.bytes || new TextEncoder().encode(packet).length > CONSTRUCTOR_LIMITS.bytes) {
    throw new Error('constructor packet exceeds the 1 MB storage limit');
  }
  return packet;
}
function initialState(libraryFingerprint) {
  return { schema: CONSTRUCTOR_SCHEMA, version: 1, libraryFingerprint, title: 'Tree of Sophia', nodes: [], edges: [], revision: 0 };
}
function radialPosition(index, count, center = [0, 0, 0], radius = 110) {
  const angle = -Math.PI / 2 + index * Math.PI * 2 / Math.max(count, 1);
  return [center[0] + Math.cos(angle) * radius, center[1] + Math.sin(angle) * radius, center[2] + (index % 3 - 1) * 22];
}

/** All failed mutations are atomic. Persistence failure never discards in-memory work. */
export function createConstructorModel(library, { storage, key = 'tos.constructor.v1' } = {}) {
  if (!record(library) || library.schema !== 'tos_constructor_library_v1' || !Array.isArray(library.nodes)) throw new Error('constructor library schema is invalid');
  const libraryFingerprint = library.fingerprint;
  if (typeof libraryFingerprint !== 'string' || !/^[a-f0-9]{64}$/.test(libraryFingerprint)) throw new Error('library fingerprint must be a SHA-256 content digest');
  const materials = new Map();
  for (const material of library.nodes) {
    if (!record(material) || !MATERIAL_KINDS.has(material.kind)) throw new Error('library material kind is invalid');
    text(material.id, 'library material id', 220);
    if (materials.has(material.id)) throw new Error(`duplicate library material: ${material.id}`);
    if ((!SOURCE_KINDS.has(material.kind) && material.demo !== true) || (material.demo !== undefined && typeof material.demo !== 'boolean')) throw new Error('mockup material kinds must declare demo: true');
    if (material.demo === true) {
      text(material.title?.ru, 'mockup title.ru', 512); text(material.title?.en, 'mockup title.en', 512);
      text(material.body?.ru, 'mockup body.ru', 4_000, true); text(material.body?.en, 'mockup body.en', 4_000, true);
      position(material.position);
    }
    materials.set(material.id, clone(material));
  }
  const rootId = library.rootId;
  if (!materials.has(rootId)) throw new Error('library root does not exist');
  const children = new Map();
  for (const material of materials.values()) {
    if (material.parentId !== null && !materials.has(material.parentId)) throw new Error(`library parent does not exist: ${material.id}`);
    if (material.parentId === material.id) throw new Error(`library material cannot parent itself: ${material.id}`);
    const siblings = children.get(material.parentId) ?? [];
    siblings.push(material); children.set(material.parentId, siblings);
  }
  for (const material of materials.values()) {
    const seen = new Set([material.id]); let parent = material.parentId;
    while (parent !== null) {
      if (seen.has(parent)) throw new Error('library hierarchy contains a cycle');
      seen.add(parent); parent = materials.get(parent).parentId;
    }
  }
  const atlasEdges = new Map();
  const atlas = library.atlas === undefined ? null : clone(library.atlas);
  function materialIds(value, name) {
    if (!Array.isArray(value) || value.length > CONSTRUCTOR_LIMITS.nodes || new Set(value).size !== value.length) throw new Error(`${name} must contain at most 200 unique material IDs`);
    for (const id of value) if (typeof id !== 'string' || !materials.has(id)) throw new Error(`${name} contains an unknown material: ${String(id)}`);
    return value;
  }
  if (atlas !== null) {
    if (!record(atlas) || !Array.isArray(atlas.edges)) throw new Error('atlas must contain an edges array');
    materialIds(atlas.defaultIds, 'atlas defaultIds');
    for (const edge of atlas.edges) {
      if (!record(edge)) throw new Error('atlas edge is invalid');
      text(edge.id, 'atlas edge id', 220);
      if (atlasEdges.has(edge.id)) throw new Error(`duplicate atlas edge: ${edge.id}`);
      if (!materials.has(edge.from) || !materials.has(edge.to) || edge.from === edge.to) throw new Error('atlas edge endpoints must be two known, distinct materials');
      if (!EDGE_KINDS.has(edge.kind)) throw new Error('atlas edge kind is invalid');
      atlasEdges.set(edge.id, edge);
    }
  }
  text(key, 'storage key');
  let state = initialState(libraryFingerprint); let error = null; let blockedStorage = false; let serial = 0;
  const undoStack = []; const redoStack = []; const listeners = new Set();
  let persistence = storage;
  if (storage === undefined) {
    try { persistence = typeof globalThis.localStorage === 'undefined' ? null : globalThis.localStorage; }
    catch (cause) { error = `Local storage unavailable: ${cause instanceof Error ? cause.message : String(cause)}`; persistence = null; }
  }
  const materialNodeId = (id) => `material:${id}`;
  const nodeOf = (draft, id) => {
    const node = draft.nodes.find((candidate) => candidate.id === id);
    if (!node) throw new Error(`node does not exist: ${id}`);
    return node;
  };
  const materialOf = (id) => {
    const material = materials.get(id);
    if (!material) throw new Error(`material does not exist in this library: ${id}`);
    return material;
  };
  function validate(candidate) {
    shape(candidate, ['schema', 'version', 'libraryFingerprint', 'title', 'nodes', 'edges', 'revision'], [], 'constructor packet');
    if (candidate.schema !== CONSTRUCTOR_SCHEMA || candidate.version !== 1) throw new Error('constructor schema or version is unsupported');
    if (candidate.libraryFingerprint !== libraryFingerprint) throw new Error('constructor library fingerprint does not match; material references cannot be rebound to another library');
    text(candidate.title, 'tree title', 512);
    if (!Number.isSafeInteger(candidate.revision) || candidate.revision < 0 || candidate.revision > MAX_REVISION) throw new Error('revision is invalid');
    if (!Array.isArray(candidate.nodes) || candidate.nodes.length > CONSTRUCTOR_LIMITS.nodes) throw new Error('tree supports at most 200 nodes');
    if (!Array.isArray(candidate.edges) || candidate.edges.length > CONSTRUCTOR_LIMITS.edges) throw new Error('tree supports at most 600 edges');
    const nodes = new Map(); const seenMaterials = new Set();
    for (const node of candidate.nodes) {
      if (!record(node)) throw new Error('node is invalid');
      if (own(node, 'materialId')) {
        shape(node, ['id', 'materialId', 'kind', 'sourceIds', 'position'], [], 'material node');
        const material = materialOf(node.materialId);
        if (node.id !== materialNodeId(material.id) || node.kind !== material.kind) throw new Error('material node must match its library identity and kind');
        if (seenMaterials.has(material.id)) throw new Error('a material can occur only once in one tree');
        seenMaterials.add(material.id);
      } else {
        shape(node, ['id', 'kind', 'title', 'body', 'sourceIds', 'position', ...Object.keys(LOCAL)], [], 'draft node');
        if (!DRAFT_KINDS.has(node.kind)) throw new Error('draft kind is invalid');
        for (const [field, value] of Object.entries(LOCAL)) if (node[field] !== value) throw new Error('draft authority must remain local-only, unreviewed and non-canonical');
        text(node.title, 'draft title', 512); text(node.body, 'draft body', 4_000, true);
        if (!node.id.startsWith('draft:')) throw new Error('draft id must use the draft namespace');
      }
      text(node.id, 'node id'); position(node.position);
      if (nodes.has(node.id)) throw new Error(`duplicate node: ${node.id}`);
      if (!Array.isArray(node.sourceIds) || node.sourceIds.length > CONSTRUCTOR_LIMITS.nodes || new Set(node.sourceIds).size !== node.sourceIds.length) throw new Error('sourceIds must be a bounded array of unique node IDs');
      if (own(node, 'materialId') && node.sourceIds.length) throw new Error('material source references come from the library');
      nodes.set(node.id, node);
    }
    for (const node of candidate.nodes) for (const id of node.sourceIds) {
      if (typeof id !== 'string' || !nodes.has(id) || id === node.id) throw new Error(`source node does not exist or references itself: ${String(id)}`);
    }
    const edgeIds = new Set(); const relations = new Set();
    for (const edge of candidate.edges) {
      shape(edge, ['id', 'from', 'to', 'kind', 'origin'], ['label'], 'edge');
      text(edge.id, 'edge id');
      if (!nodes.has(edge.from) || !nodes.has(edge.to) || edge.from === edge.to) throw new Error('edge endpoints must be two existing, distinct nodes');
      if (!EDGE_KINDS.has(edge.kind)) throw new Error('edge kind is invalid');
      if (edge.label !== undefined) text(edge.label, 'edge label', 512, true);
      if (!['structure', 'draft'].includes(edge.origin)) throw new Error('edge origin is invalid');
      if (edge.id.startsWith('atlas:')) {
        const owned = atlasEdges.get(edge.id.slice(6));
        if (!owned || edge.from !== materialNodeId(owned.from) || edge.to !== materialNodeId(owned.to) || edge.kind !== owned.kind || edge.origin !== 'draft' || own(edge, 'label')) throw new Error('atlas edge must match its exact library relation');
      }
      if (edge.origin === 'structure') {
        const from = nodes.get(edge.from); const to = nodes.get(edge.to);
        if (edge.kind !== 'contains' || !from.materialId || !to.materialId || materials.get(to.materialId).parentId !== from.materialId || own(edge, 'label')) {
          throw new Error('structural edge must match the exact library hierarchy');
        }
      }
      // Atlas relations own bilingual wording by identity; a personal relation between
      // the same endpoints remains an independent assertion and must survive a merge.
      const relation = JSON.stringify([edge.from, edge.to, edge.kind, edge.origin, edge.id.startsWith('atlas:') ? ['atlas', edge.id] : ['local', edge.label ?? '']]);
      if (edgeIds.has(edge.id) || relations.has(relation)) throw new Error('duplicate edge identity or relation');
      edgeIds.add(edge.id); relations.add(relation);
    }
    boundedPacket(JSON.stringify(candidate));
    return clone(candidate);
  }
  function parse(packet) {
    boundedPacket(packet);
    let candidate;
    try { candidate = JSON.parse(packet); } catch { throw new Error('constructor packet is not valid JSON'); }
    return validate(candidate);
  }
  if (persistence) {
    try { const packet = persistence.getItem(key); if (packet !== null) state = parse(packet); }
    catch (cause) {
      blockedStorage = true;
      error = `Saved tree could not be loaded; original storage is protected: ${cause instanceof Error ? cause.message : String(cause)}`;
    }
  }
  function persist() {
    if (!persistence || blockedStorage) return;
    try { persistence.setItem(key, boundedPacket(JSON.stringify(state))); error = null; }
    catch (cause) { error = `Tree remains in memory; saving failed: ${cause instanceof Error ? cause.message : String(cause)}`; }
  }
  function notify() {
    // A listener receives its own snapshot and cannot corrupt another listener's view.
    for (const listener of listeners) listener(clone(state));
  }
  function push(stack, item) {
    stack.push(clone(item));
    if (stack.length > CONSTRUCTOR_LIMITS.history) stack.shift();
  }
  function nextRevision() {
    if (state.revision >= MAX_REVISION) throw new Error('constructor revision limit reached; export this tree before starting another');
    return state.revision + 1;
  }
  function commit(mutate) {
    const candidate = clone(state); mutate(candidate);
    if (JSON.stringify(candidate) === JSON.stringify(state)) return false;
    candidate.revision = nextRevision();
    const next = validate(candidate);
    push(undoStack, state); redoStack.length = 0; state = next;
    persist(); notify(); return true;
  }
  function transition(from, to) {
    if (!from.length) return false;
    const candidate = clone(from.at(-1)); candidate.revision = nextRevision();
    const next = validate(candidate);
    from.pop(); push(to, state); state = next; persist(); notify(); return true;
  }
  function freshId(prefix, draft) {
    let id;
    do { id = `${prefix}:${++serial}`; } while (draft.nodes.some((node) => node.id === id) || draft.edges.some((edge) => edge.id === id));
    return id;
  }
  function addEdge(draft, from, to, kind, origin, label) {
    nodeOf(draft, from); nodeOf(draft, to);
    const existing = draft.edges.find((edge) => !edge.id.startsWith('atlas:') && edge.from === from && edge.to === to && edge.kind === kind && edge.origin === origin && (edge.label ?? '') === (label ?? ''));
    if (existing) return existing.id;
    const id = freshId('edge', draft);
    draft.edges.push({ id, from, to, kind, origin, ...(label ? { label } : {}) });
    return id;
  }
  function addMaterialTo(draft, id, options = {}) {
    const material = materialOf(id); const nodeId = materialNodeId(id);
    if (options.position !== undefined) position(options.position);
    if (options.parentId !== undefined) nodeOf(draft, options.parentId);
    if (!draft.nodes.some((node) => node.id === nodeId)) {
      draft.nodes.push({ id: nodeId, materialId: id, kind: material.kind, sourceIds: [], position: options.position === undefined ? radialPosition(draft.nodes.length, 7) : position(options.position) });
    }
    const parentId = options.parentId !== undefined ? options.parentId : draft.nodes.find((node) => node.materialId === material.parentId)?.id;
    if (parentId !== undefined) {
      const parent = nodeOf(draft, parentId);
      addEdge(draft, parent.id, nodeId, 'contains', parent.materialId === material.parentId ? 'structure' : 'draft');
    }
    return nodeId;
  }
  function growAtlas(input) {
    if (!atlas) throw new Error('this library has no atlas presets');
    shape(input, ['nodeIds'], ['edgeIds'], 'atlas selection');
    const nodeIds = materialIds(input.nodeIds, 'atlas nodeIds');
    if (input.edgeIds !== undefined) {
      if (!Array.isArray(input.edgeIds) || input.edgeIds.length > CONSTRUCTOR_LIMITS.edges || new Set(input.edgeIds).size !== input.edgeIds.length) throw new Error('atlas edgeIds must contain at most 600 unique relation IDs');
      for (const id of input.edgeIds) if (typeof id !== 'string' || !atlasEdges.has(id)) throw new Error(`unknown atlas edge: ${String(id)}`);
    }
    return commit((draft) => {
      for (const id of nodeIds) {
        const material = materialOf(id);
        addMaterialTo(draft, id, material.position === undefined ? {} : { position: material.position });
      }
      // A preset may list a child before its parent. Complete exact structural
      // joins only after all requested nodes are present; never fetch ancestors.
      for (const id of nodeIds) addMaterialTo(draft, id);
      const visible = new Set(draft.nodes.map((node) => node.id));
      const selectedEdges = input.edgeIds === undefined
        ? [...atlasEdges.values()].filter((edge) => visible.has(materialNodeId(edge.from)) && visible.has(materialNodeId(edge.to)))
        : input.edgeIds.map((id) => atlasEdges.get(id));
      for (const edge of selectedEdges) {
        const from = materialNodeId(edge.from); const to = materialNodeId(edge.to);
        if (!visible.has(from) || !visible.has(to)) throw new Error(`atlas edge requires both materials in the workspace: ${edge.id}`);
        const id = `atlas:${edge.id}`;
        if (!draft.edges.some((candidate) => candidate.id === id)) draft.edges.push({ id, from, to, kind: edge.kind, origin: 'draft' });
      }
    });
  }
  function exportResearch() {
    const drafts = state.nodes.filter((node) => !node.materialId || materials.get(node.materialId).demo === true);
    const relations = state.edges.filter((edge) => edge.origin === 'draft');
    if (drafts.length + relations.length > 256) throw new Error('research export supports at most 256 draft nodes and relations combined; use the constructor packet for this larger tree');
    const workspace = createResearchWorkspace({ sessionId: 'constructor-local', persistence: false });
    const sourceRefs = (id, visited = new Set()) => {
      if (visited.has(id)) return [];
      visited.add(id); const node = nodeOf(state, id);
      if (node.materialId) {
        const material = materials.get(node.materialId);
        return material.demo === true ? [] : (material.sourceRefs ?? []).map((source) => source.ref);
      }
      return [...new Set(node.sourceIds.flatMap((sourceId) => sourceRefs(sourceId, visited)))];
    };
    for (const node of state.nodes) {
      const material = node.materialId ? materials.get(node.materialId) : null;
      const mockup = material?.demo === true;
      if (!material || mockup) {
        const title = mockup ? localText(material.title) : node.title;
        const body = mockup ? localText(material.body) : node.body;
        workspace.addHypothesis({ id: node.id, title, body: body || title, targetId: node.id });
      }
      const references = sourceRefs(node.id);
      // Notes carry only locators and local citation choices, never imported source claims.
      if (node.materialId || node.sourceIds.length) {
        const body = JSON.stringify({ constructorNodeId: node.id, ...(node.materialId ? { libraryMaterialId: node.materialId, ...(mockup ? { mockupMaterial: true, source: false, reviewed: false, canon: false } : {}) } : {}), sourceNodeIds: node.sourceIds, librarySourceRefs: references });
        if (body.length > 4_000) throw new Error(`research source note exceeds 4000 characters for ${node.id}; use the constructor packet to retain all references`);
        workspace.addNote({ id: `ref:${node.id}`, targetId: node.id, body });
      }
    }
    for (const edge of relations) {
      const atlasEdge = edge.id.startsWith('atlas:') ? atlasEdges.get(edge.id.slice(6)) : null;
      const title = edge.label || localText(atlasEdge?.label) || edge.kind;
      const body = localText(atlasEdge?.body) || title;
      workspace.addHypothesis({ id: `relation:${edge.id}`, title, body, fromId: edge.from, toId: edge.to, predicateLabel: edge.kind });
    }
    const packet = workspace.exportPacket();
    if (packet.length > 1_000_000) throw new Error('research packet exceeds its 1 MB limit; use the constructor packet to retain this tree');
    return packet;
  }
  return {
    getState: () => clone(state),
    persistenceError: () => error,
    subscribe(listener) { if (typeof listener !== 'function') throw new Error('listener must be a function'); listeners.add(listener); return () => listeners.delete(listener); },
    addMaterial(id, options = {}) { let result; commit((draft) => { result = addMaterialTo(draft, id, options); }); return result; },
    expand(id) {
      const node = nodeOf(state, id);
      if (!node.materialId) throw new Error('only library material nodes have structural children');
      const descendants = children.get(node.materialId) ?? []; const added = [];
      commit((draft) => {
        descendants.forEach((child, index) => {
          const childId = materialNodeId(child.id);
          if (!draft.nodes.some((item) => item.id === childId)) added.push(childId);
          addMaterialTo(draft, child.id, { parentId: id, position: radialPosition(index, descendants.length, node.position) });
        });
      });
      return added;
    },
    addDraft(input) {
      shape(input, ['kind', 'title'], ['body', 'sourceIds', 'position'], 'draft input');
      let id;
      commit((draft) => {
        id = freshId('draft', draft);
        draft.nodes.push({ id, kind: input.kind, title: input.title, body: input.body ?? '', sourceIds: input.sourceIds ?? [], position: input.position ?? radialPosition(draft.nodes.length, 7), ...LOCAL });
      });
      return id;
    },
    addDraftWithContext(input, options = {}) {
      shape(input, ['kind', 'title'], ['body', 'sourceIds', 'position'], 'draft input');
      shape(options, [], ['sourceId', 'materialId', 'relationKind', 'reverse'], 'draft context');
      if (options.sourceId !== undefined && options.materialId !== undefined) throw new Error('draft context must specify either sourceId or materialId, not both');
      if (options.reverse !== undefined && typeof options.reverse !== 'boolean') throw new Error('draft context reverse must be boolean');
      const relationKind = options.relationKind ?? 'relates';
      if (!EDGE_KINDS.has(relationKind)) throw new Error('draft context relation kind is invalid');
      if (input.sourceIds !== undefined && !Array.isArray(input.sourceIds)) throw new Error('sourceIds must be an array of node IDs');
      let id;
      commit((draft) => {
        const sourceId = options.sourceId !== undefined ? nodeOf(draft, options.sourceId).id
          : options.materialId !== undefined ? addMaterialTo(draft, options.materialId) : null;
        const source = sourceId ? nodeOf(draft, sourceId) : null;
        const sourceIds = [...(input.sourceIds ?? [])];
        if (sourceId && !sourceIds.includes(sourceId)) sourceIds.push(sourceId);
        id = freshId('draft', draft);
        const defaultPosition = source ? [source.position[0] + 150, source.position[1] + 105, source.position[2] + 20] : radialPosition(draft.nodes.length, 7);
        draft.nodes.push({ id, kind: input.kind, title: input.title, body: input.body ?? '', sourceIds, position: input.position ?? defaultPosition, ...LOCAL });
        if (sourceId) addEdge(draft, options.reverse ? id : sourceId, options.reverse ? sourceId : id, relationKind, 'draft');
      });
      return id;
    },
    editDraft(id, patch) {
      shape(patch, [], ['title', 'body'], 'draft edit');
      return commit((draft) => { const node = nodeOf(draft, id); if (node.materialId) throw new Error('library material cannot be edited as a draft'); Object.assign(node, patch); });
    },
    connect(from, to, kind, label) {
      if (label !== undefined) text(label, 'edge label', 512, true);
      let id; commit((draft) => { id = addEdge(draft, from, to, kind, 'draft', label); }); return id;
    },
    removeNode(id) {
      return commit((draft) => {
        nodeOf(draft, id); draft.nodes = draft.nodes.filter((node) => node.id !== id);
        draft.edges = draft.edges.filter((edge) => edge.from !== id && edge.to !== id);
        for (const node of draft.nodes) node.sourceIds = node.sourceIds.filter((sourceId) => sourceId !== id);
      });
    },
    removeEdge(id) { return commit((draft) => { if (!draft.edges.some((edge) => edge.id === id)) throw new Error(`edge does not exist: ${id}`); draft.edges = draft.edges.filter((edge) => edge.id !== id); }); },
    moveNode(id, value) { const next = position(value); return commit((draft) => { nodeOf(draft, id).position = next; }); },
    rename(title) { text(title, 'tree title', 512); return commit((draft) => { draft.title = title; }); },
    canUndo: () => undoStack.length > 0, canRedo: () => redoStack.length > 0,
    undo: () => transition(undoStack, redoStack), redo: () => transition(redoStack, undoStack),
    clear() { return commit((draft) => { draft.nodes = []; draft.edges = []; }); },
    growAtlas,
    seedAtlas() { if (!atlas) throw new Error('this library has no atlas presets'); return growAtlas({ nodeIds: atlas.defaultIds }); },
    seed() {
      return commit((draft) => {
        const root = addMaterialTo(draft, rootId, { position: [0, 0, 0] });
        const parts = (children.get(rootId) ?? []).filter((node) => node.kind === 'part').slice(0, 4);
        parts.forEach((part, index) => addMaterialTo(draft, part.id, { parentId: root, position: radialPosition(index, parts.length, [0, 0, 0], 140) }));
      });
    },
    exportPacket: () => boundedPacket(JSON.stringify(state)),
    importPacket(packet) { const imported = parse(packet); return commit((draft) => { Object.assign(draft, imported, { revision: state.revision }); }); },
    exportResearch,
  };
}
