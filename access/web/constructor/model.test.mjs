import { describe, expect, it, vi } from 'vitest';
import { createConstructorModel, CONSTRUCTOR_LIMITS } from './model.mjs';
import { createResearchWorkspace } from '../src/research-workspace.ts';

function library() {
  const material = (id, kind, parentId) => ({ id, kind, parentId, title: { ru: `Материал ${id}`, en: `Material ${id}` }, body: { ru: '', en: '' }, sourceRefs: [{ label: 'Source', ref: `ToS/source-witnesses/${id}.md` }] });
  return { schema: 'tos_constructor_library_v1', fingerprint: 'a'.repeat(64), rootId: 'work', nodes: [
    material('work', 'work', null), ...Array.from({ length: 4 }, (_, i) => material(`part-${i + 1}`, 'part', 'work')),
    material('chapter', 'chapter', 'part-1'), material('fragment', 'fragment', 'chapter'), material('dossier', 'dossier', null),
  ] };
}
function atlasLibrary() {
  const source = library(); source.fingerprint = 'b'.repeat(64);
  const kinds = ['concept', 'interpretation', 'question', 'note', 'excerpt', 'figure', 'symbol', 'tradition'];
  const demos = kinds.map((kind, index) => ({ id: `demo-${kind}`, kind, demo: true, parentId: null, title: { ru: `Макет ${kind}`, en: `Mockup ${kind}` }, body: { ru: 'Предложение для обсуждения.', en: 'A proposition for discussion.' }, position: [index * 110, index * 35, index * 20], sourceRefs: [{ label: 'Untrusted demo locator', ref: `not-a-source:${kind}` }] }));
  const edges = ['contrasts', 'echoes', 'develops', 'compares', 'translates'].map((kind, index) => ({ id: `relation-${index}`, from: demos[index].id, to: demos[index + 1].id, kind, label: { ru: 'Связь макета', en: 'Mockup relation' }, body: { ru: 'Одна из возможных связей.', en: 'One possible relation.' } }));
  source.nodes.push(...demos);
  source.atlas = { defaultIds: [...demos.map((node) => node.id), 'fragment', 'chapter'], edges, assemblies: [] };
  return source;
}
function memoryStorage() {
  const values = new Map();
  return { values, getItem: vi.fn((key) => values.get(key) ?? null), setItem: vi.fn((key, value) => { values.set(key, value); }) };
}
const model = (options = {}) => createConstructorModel(library(), { storage: null, ...options });
function withoutRevision(state) { const { revision, ...rest } = state; return rest; }

describe('local constructor state', () => {
  it('adds a bounded, undoable starter and expands only the selected material', () => {
    const tree = model();
    expect(tree.getState().nodes).toHaveLength(0);
    tree.seed();
    expect(tree.getState().nodes.map((node) => node.materialId)).toEqual(['work', 'part-1', 'part-2', 'part-3', 'part-4']);
    expect(tree.getState().edges).toHaveLength(4);
    const seeded = tree.exportPacket();
    expect(tree.seed()).toBe(false);
    expect(tree.exportPacket()).toBe(seeded);
    expect(tree.expand('material:part-1')).toEqual(['material:chapter']);
    expect(tree.getState().nodes.some((node) => node.materialId === 'fragment')).toBe(false);
    tree.undo(); expect(tree.getState().nodes).toHaveLength(5);
    tree.undo(); expect(tree.getState().nodes).toHaveLength(0);
    tree.redo(); expect(tree.getState().edges).toHaveLength(4);
  });

  it('lets independent trees arrange the same materials and form different local relations', () => {
    const first = model(); const second = model();
    for (const tree of [first, second]) { tree.addMaterial('work'); tree.addMaterial('dossier'); }
    const a = first.addMaterial('work'); const b = first.addMaterial('dossier');
    first.connect(a, b, 'questions', 'Что связывает эти материалы?');
    second.connect(a, b, 'supports');
    first.moveNode(b, [11, 22, 33]);
    expect(first.getState().nodes).toHaveLength(2);
    expect(first.getState().edges[0]).toMatchObject({ kind: 'questions', origin: 'draft' });
    expect(second.getState().edges[0]).toMatchObject({ kind: 'supports', origin: 'draft' });
    expect(second.getState().nodes.find((node) => node.id === b).position).not.toEqual([11, 22, 33]);
    const relation = first.connect(a, b, 'questions', 'Что связывает эти материалы?');
    expect(first.getState().edges).toHaveLength(1);
    expect(first.getState().edges[0].id).toBe(relation);
  });

  it('joins a material to its visible library parent without materializing absent ancestors', () => {
    const tree = model(); const chapter = tree.addMaterial('chapter');
    expect(tree.getState().nodes).toHaveLength(1); expect(tree.getState().edges).toEqual([]);
    const part = tree.addMaterial('part-1');
    expect(tree.getState().nodes).toHaveLength(2); expect(tree.getState().nodes.some((node) => node.materialId === 'work')).toBe(false);
    expect(tree.addMaterial('chapter')).toBe(chapter);
    expect(tree.getState().edges).toMatchObject([{ from: part, to: chapter, kind: 'contains', origin: 'structure' }]);
    const joined = tree.exportPacket(); tree.addMaterial('chapter'); expect(tree.exportPacket()).toBe(joined);
    const fragment = tree.addMaterial('fragment');
    expect(tree.getState().edges).toContainEqual(expect.objectContaining({ from: chapter, to: fragment, kind: 'contains', origin: 'structure' }));
    tree.undo(); expect(tree.getState().nodes).toHaveLength(2); expect(tree.getState().edges).toHaveLength(1);
  });

  it('preserves edits across undo, redo and reload, and branches history after a new edit', () => {
    const storage = memoryStorage(); const tree = model({ storage });
    const reference = tree.addMaterial('fragment');
    const id = tree.addDraft({ kind: 'interpretation', title: 'Становление', body: 'Первая мысль', sourceIds: [reference] });
    tree.editDraft(id, { title: 'Becoming', body: 'A second reading' });
    tree.rename('Моё дерево / My tree'); tree.moveNode(id, [1, -2, 3]);
    const edited = tree.getState();
    tree.undo(); tree.undo(); tree.undo();
    expect(tree.getState().nodes.find((node) => node.id === id)).toMatchObject({ title: 'Становление', body: 'Первая мысль' });
    tree.redo(); tree.redo(); tree.redo();
    expect(withoutRevision(tree.getState())).toEqual(withoutRevision(edited));
    const restored = model({ storage });
    expect(restored.getState()).toEqual(tree.getState());
    expect(restored.canUndo()).toBe(false);
    tree.undo(); tree.editDraft(id, { body: 'A different direction' });
    expect(tree.canRedo()).toBe(false);
    expect(tree.getState().revision).toBeGreaterThan(edited.revision);
  });

  it('removes incident edges and citations atomically, restores them on undo, and clears undoably', () => {
    const tree = model(); const source = tree.addMaterial('chapter');
    const id = tree.addDraft({ kind: 'question', title: 'Почему?', sourceIds: [source] });
    tree.connect(id, source, 'questions');
    const before = tree.getState(); tree.removeNode(source);
    expect(tree.getState().nodes[0].sourceIds).toEqual([]);
    expect(tree.getState().edges).toEqual([]);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
    tree.clear(); expect(tree.getState().nodes).toEqual([]);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
  });

  it('exports only local hypotheses and library-derived locators through the standard workspace', () => {
    const sourceLibrary = library(); const tree = createConstructorModel(sourceLibrary, { storage: null });
    const source = tree.addMaterial('fragment');
    const idea = tree.addDraft({ kind: 'concept', title: 'Возвращение', body: 'A local interpretation', sourceIds: [source] });
    tree.connect(idea, source, 'interprets', 'My reading');
    sourceLibrary.nodes.find((node) => node.id === 'fragment').sourceRefs[0].ref = 'forged-after-construction';
    const exported = tree.exportResearch();
    const research = createResearchWorkspace({ persistence: false });
    research.importPacket(exported);
    const state = research.getState();
    expect(state.proposals).toEqual([]);
    expect(state.hypotheses).toHaveLength(2);
    expect(state.hypotheses.every((hypothesis) => hypothesis.posture.session_hypothesis && !hypothesis.posture.source && !hypothesis.posture.reviewed && !hypothesis.posture.canon)).toBe(true);
    const citation = JSON.parse(state.notes.find((note) => note.targetId === idea).body);
    expect(citation.librarySourceRefs).toEqual(['ToS/source-witnesses/fragment.md']);
    expect(citation.sourceNodeIds).toEqual([source]);
  });

  it('bounds undo history while retaining the newest 64 transitions', () => {
    const tree = model();
    for (let i = 1; i <= 70; i++) tree.rename(`Tree ${i}`);
    let undone = 0; while (tree.undo()) undone++;
    expect(undone).toBe(CONSTRUCTOR_LIMITS.history);
    expect(tree.getState().title).toBe('Tree 6');
    let redone = 0; while (tree.redo()) redone++;
    expect(redone).toBe(CONSTRUCTOR_LIMITS.history);
    expect(tree.getState().title).toBe('Tree 70');
  });

  it('hands every observer an independent snapshot and supports unsubscription', () => {
    const tree = model(); const received = [];
    tree.subscribe((snapshot) => { snapshot.title = 'changed elsewhere'; });
    const unsubscribe = tree.subscribe((snapshot) => received.push(snapshot));
    tree.rename('My tree'); unsubscribe(); tree.rename('Next tree');
    expect(received).toHaveLength(1); expect(received[0].title).toBe('My tree');
    const snapshot = tree.getState(); snapshot.nodes.push({ id: 'outside' });
    expect(tree.getState().nodes).toEqual([]);
  });
});

describe('atomic contextual creation', () => {
  it('materializes a preview, drafts its thought and connects them in one saved undo step', () => {
    const storage = memoryStorage(); const tree = model({ storage }); const listener = vi.fn();
    tree.addMaterial('chapter'); const before = tree.getState(); storage.setItem.mockClear();
    tree.subscribe(listener);
    const id = tree.addDraftWithContext({ kind: 'concept', title: 'A connected idea' }, { materialId: 'fragment', relationKind: 'supports' });
    const created = tree.getState();
    expect(created.revision).toBe(before.revision + 1); expect(created.nodes).toHaveLength(3);
    expect(created.nodes.find((node) => node.id === id)).toMatchObject({ sourceIds: ['material:fragment'], localOnly: true, canon: false });
    expect(created.edges).toMatchObject([{ from: 'material:chapter', to: 'material:fragment', kind: 'contains', origin: 'structure' }, { from: 'material:fragment', to: id, kind: 'supports', origin: 'draft' }]);
    expect(storage.setItem).toHaveBeenCalledTimes(1); expect(listener).toHaveBeenCalledTimes(1);
    expect(model({ storage }).getState()).toEqual(created);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
    tree.redo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(created));
  });

  it('reuses an existing source, preserves citations and directs a question toward that source', () => {
    const tree = model(); const sourceId = tree.addMaterial('chapter'); const otherSource = tree.addMaterial('dossier');
    const before = tree.getState();
    const id = tree.addDraftWithContext({ kind: 'question', title: 'Why?', sourceIds: [sourceId, otherSource] }, { sourceId, relationKind: 'questions', reverse: true });
    expect(tree.getState().nodes.find((node) => node.id === id).sourceIds).toEqual([sourceId, otherSource]);
    expect(tree.getState().edges).toMatchObject([{ from: id, to: sourceId, kind: 'questions', origin: 'draft' }]);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
  });

  it.each(['nodes', 'edges', 'parent edge'])('leaves no partial material, thought or relation when the %s limit is reached', (limit) => {
    const storage = memoryStorage(); const tree = model({ storage });
    const sourceId = tree.addMaterial('work'); const draftId = tree.addDraft({ kind: 'note', title: 'Existing' });
    tree.connect(sourceId, draftId, 'relates');
    const packet = JSON.parse(tree.exportPacket());
    if (limit === 'nodes') {
      const template = packet.nodes.find((node) => node.id === draftId);
      packet.nodes = Array.from({ length: 199 }, (_, index) => ({ ...template, id: `draft:existing-${index}` })); packet.edges = [];
    } else {
      if (limit === 'parent edge') packet.nodes.push({ id: 'material:chapter', materialId: 'chapter', kind: 'chapter', sourceIds: [], position: [0, 0, 0] });
      packet.edges = Array.from({ length: limit === 'parent edge' ? 599 : 600 }, (_, index) => ({ ...packet.edges[0], id: `edge:existing-${index}`, label: `Existing reading ${index}` }));
    }
    tree.importPacket(JSON.stringify(packet)); tree.rename('Temporary'); tree.undo();
    const before = tree.exportPacket(); const saved = storage.values.get('tos.constructor.v1');
    const listener = vi.fn(); tree.subscribe(listener); storage.setItem.mockClear();
    const options = limit !== 'edges' ? { materialId: 'fragment', relationKind: 'supports' } : { sourceId, relationKind: 'questions', reverse: true };
    for (let retry = 0; retry < 2; retry++) expect(() => tree.addDraftWithContext({ kind: 'question', title: 'Must stay unsaved' }, options)).toThrow();
    expect(tree.exportPacket()).toBe(before); expect(tree.canRedo()).toBe(true);
    expect(storage.values.get('tos.constructor.v1')).toBe(saved); expect(storage.setItem).not.toHaveBeenCalled(); expect(listener).not.toHaveBeenCalled();
  });
});

describe('populated atlas presets', () => {
  it('merges a preset once without replacing personal edits, positions or parallel relations, then undoes it together', () => {
    const source = atlasLibrary(); const storage = memoryStorage(); const tree = createConstructorModel(source, { storage });
    const from = tree.addMaterial('demo-concept'); const to = tree.addMaterial('demo-interpretation');
    tree.moveNode(from, [910, -120, 75]);
    const personal = tree.addDraft({ kind: 'note', title: 'My note', body: 'Keep this thought' });
    tree.editDraft(personal, { body: 'My revised thought' }); tree.rename('My existing space');
    const personalRelation = tree.connect(from, to, 'contrasts');
    const before = tree.getState(); const listener = vi.fn(); tree.subscribe(listener); storage.setItem.mockClear();
    expect(tree.seedAtlas()).toBe(true);
    const merged = tree.getState();
    expect(merged.title).toBe(before.title);
    expect(merged.nodes.find((node) => node.id === personal).body).toBe('My revised thought');
    expect(merged.nodes.find((node) => node.id === from).position).toEqual([910, -120, 75]);
    expect(merged.nodes.find((node) => node.materialId === 'demo-symbol').position).toEqual(source.nodes.find((node) => node.id === 'demo-symbol').position);
    expect(merged.edges.some((edge) => edge.id === personalRelation)).toBe(true);
    expect(merged.edges.some((edge) => edge.id === 'atlas:relation-0')).toBe(true);
    expect(merged.edges.some((edge) => edge.from === 'material:chapter' && edge.to === 'material:fragment' && edge.origin === 'structure')).toBe(true);
    expect(storage.setItem).toHaveBeenCalledTimes(1); expect(listener).toHaveBeenCalledTimes(1);
    const exported = tree.exportPacket(); expect(tree.seedAtlas()).toBe(false); expect(tree.exportPacket()).toBe(exported);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
    tree.redo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(merged));
    expect(createConstructorModel(source, { storage }).getState()).toEqual(tree.getState());
  });

  it('adds only eligible implicit edges and rejects an explicit dangling relation atomically', () => {
    const storage = memoryStorage(); const tree = createConstructorModel(atlasLibrary(), { storage });
    tree.addDraft({ kind: 'note', title: 'Existing thought' }); tree.rename('Temporary'); tree.undo();
    const before = tree.exportPacket(); const saved = storage.values.get('tos.constructor.v1'); const listener = vi.fn(); tree.subscribe(listener); storage.setItem.mockClear();
    expect(() => tree.growAtlas({ nodeIds: ['demo-concept'], edgeIds: ['relation-0'] })).toThrow();
    expect(tree.exportPacket()).toBe(before); expect(tree.canRedo()).toBe(true);
    expect(storage.values.get('tos.constructor.v1')).toBe(saved); expect(storage.setItem).not.toHaveBeenCalled(); expect(listener).not.toHaveBeenCalled();
    tree.growAtlas({ nodeIds: ['demo-concept'] }); expect(tree.getState().edges).toEqual([]);
    tree.growAtlas({ nodeIds: ['demo-interpretation'] });
    expect(tree.getState().edges).toMatchObject([{ id: 'atlas:relation-0', from: 'material:demo-concept', to: 'material:demo-interpretation' }]);
    const ids = new Set(tree.getState().nodes.map((node) => node.id));
    expect(tree.getState().edges.every((edge) => ids.has(edge.from) && ids.has(edge.to))).toBe(true);
  });

  it.each(['nodes', 'edges'])('rejects an overflowing preset at the %s limit without leaving any partial additions', (limit) => {
    const storage = memoryStorage(); const tree = createConstructorModel(atlasLibrary(), { storage });
    const from = tree.addDraft({ kind: 'note', title: 'First existing thought' }); const to = tree.addDraft({ kind: 'note', title: 'Second existing thought' });
    tree.connect(from, to, 'relates'); const packet = JSON.parse(tree.exportPacket());
    if (limit === 'nodes') {
      packet.nodes = Array.from({ length: 199 }, (_, index) => ({ ...packet.nodes[0], id: `draft:existing-${index}` })); packet.edges = [];
    } else packet.edges = Array.from({ length: 600 }, (_, index) => ({ ...packet.edges[0], id: `edge:existing-${index}`, label: `Reading ${index}` }));
    tree.importPacket(JSON.stringify(packet)); tree.rename('Temporary'); tree.undo();
    const before = tree.exportPacket(); const saved = storage.values.get('tos.constructor.v1'); const listener = vi.fn(); tree.subscribe(listener); storage.setItem.mockClear();
    expect(() => tree.seedAtlas()).toThrow();
    expect(tree.exportPacket()).toBe(before); expect(tree.canRedo()).toBe(true);
    expect(storage.values.get('tos.constructor.v1')).toBe(saved); expect(storage.setItem).not.toHaveBeenCalled(); expect(listener).not.toHaveBeenCalled();
  });

  it('round-trips richer relation kinds and keeps personal relations distinct from bound atlas identities', () => {
    const source = atlasLibrary(); const tree = createConstructorModel(source, { storage: null }); tree.seedAtlas();
    for (const edge of source.atlas.edges) {
      const personal = tree.connect(`material:${edge.from}`, `material:${edge.to}`, edge.kind);
      expect(personal.startsWith('atlas:')).toBe(false);
    }
    const packet = tree.exportPacket(); const restored = createConstructorModel(source, { storage: null }); restored.importPacket(packet);
    expect(withoutRevision(restored.getState())).toEqual(withoutRevision(tree.getState()));
    expect(new Set(restored.getState().edges.filter((edge) => edge.id.startsWith('atlas:')).map((edge) => edge.kind))).toEqual(new Set(['contrasts', 'echoes', 'develops', 'compares', 'translates']));
    const altered = JSON.parse(packet); altered.edges.find((edge) => edge.id === 'atlas:relation-0').kind = 'supports';
    const before = restored.exportPacket(); expect(() => restored.importPacket(JSON.stringify(altered))).toThrow(); expect(restored.exportPacket()).toBe(before);
  });

  it('exports mockup material as unreviewed hypotheses and excludes all demo locators from source references', () => {
    const source = atlasLibrary(); const tree = createConstructorModel(source, { storage: null }); tree.seedAtlas();
    const idea = tree.addDraft({ kind: 'interpretation', title: 'My interpretation', sourceIds: ['material:demo-concept', 'material:fragment'] });
    const research = createResearchWorkspace({ persistence: false }); const packet = tree.exportResearch(); research.importPacket(packet);
    const exported = research.getState();
    for (const material of source.nodes.filter((node) => node.demo)) {
      expect(exported.hypotheses.find((hypothesis) => hypothesis.id === `material:${material.id}`)?.posture).toEqual({ session_hypothesis: true, source: false, reviewed: false, canon: false });
    }
    expect(exported.proposals).toEqual([]); expect(packet).not.toContain('not-a-source:');
    expect(JSON.parse(exported.notes.find((note) => note.targetId === 'material:demo-concept').body)).toMatchObject({ mockupMaterial: true, source: false, reviewed: false, canon: false, librarySourceRefs: [] });
    expect(JSON.parse(exported.notes.find((note) => note.targetId === idea).body).librarySourceRefs).toEqual(['ToS/source-witnesses/fragment.md']);
    source.nodes.find((node) => node.id === 'demo-concept').demo = false;
    expect(() => createConstructorModel(source, { storage: null })).toThrow();
  });
});

describe('constructor import and authority boundary', () => {
  it('refuses to bind identical material IDs to another library revision', () => {
    const tree = model(); tree.addMaterial('work');
    const differentLibrary = library(); differentLibrary.fingerprint = 'b'.repeat(64);
    differentLibrary.nodes[0].sourceRefs[0].ref = 'another/source.md';
    const other = createConstructorModel(differentLibrary, { storage: null });
    const before = other.exportPacket();
    expect(() => other.importPacket(tree.exportPacket())).toThrow(/fingerprint/);
    expect(other.exportPacket()).toBe(before); expect(other.canUndo()).toBe(false);
    expect(() => createConstructorModel({ ...library(), fingerprint: 'not-a-digest' }, { storage: null })).toThrow(/fingerprint/);
  });

  const alterations = [
    ['canon claim', (packet) => { packet.nodes[1].canon = true; }],
    ['review claim', (packet) => { packet.nodes[1].reviewed = true; }],
    ['source claim', (packet) => { packet.nodes[1].source = true; }],
    ['nonlocal draft', (packet) => { packet.nodes[1].localOnly = false; }],
    ['copied source metadata', (packet) => { packet.nodes[0].sourceRefs = ['forged']; }],
    ['edited material text', (packet) => { packet.nodes[0].title = 'Source now claims something else'; }],
    ['unknown material', (packet) => { packet.nodes[0].materialId = 'missing'; }],
    ['wrong material kind', (packet) => { packet.nodes[0].kind = 'work'; }],
    ['missing citation node', (packet) => { packet.nodes[1].sourceIds = ['missing']; }],
    ['missing edge endpoint', (packet) => { packet.edges[0].to = 'missing'; }],
    ['forged structural relation', (packet) => { packet.edges[0].origin = 'structure'; packet.edges[0].kind = 'contains'; }],
    ['invalid position', (packet) => { packet.nodes[1].position = [0, null, 0]; }],
    ['duplicate node', (packet) => { packet.nodes.push(packet.nodes[1]); }],
    ['too many edges', (packet) => { packet.edges = Array(601).fill(packet.edges[0]); }],
    ['too many nodes', (packet) => { packet.nodes = Array(201).fill(packet.nodes[0]); }],
  ];
  it.each(alterations)('rejects %s without changing state, storage, history or observers', (_, alter) => {
    const storage = memoryStorage(); const tree = model({ storage });
    const source = tree.addMaterial('fragment');
    const draft = tree.addDraft({ kind: 'note', title: 'My note', sourceIds: [source] });
    tree.connect(draft, source, 'relates');
    tree.rename('Temporary'); tree.undo();
    const before = tree.exportPacket(); const saved = storage.getItem('tos.constructor.v1');
    const listener = vi.fn(); tree.subscribe(listener); storage.setItem.mockClear();
    const forged = JSON.parse(before); alter(forged);
    expect(() => tree.importPacket(JSON.stringify(forged))).toThrow();
    expect(tree.exportPacket()).toBe(before); expect(tree.canRedo()).toBe(true);
    expect(storage.getItem('tos.constructor.v1')).toBe(saved);
    expect(storage.setItem).not.toHaveBeenCalled(); expect(listener).not.toHaveBeenCalled();
  });

  it('imports a valid tree as one undoable replacement with a local revision', () => {
    const tree = model(); tree.seed(); const before = tree.getState();
    const other = model(); other.addDraft({ kind: 'note', title: 'Another tree' });
    const packet = JSON.parse(other.exportPacket()); packet.revision = 1_000_000_000;
    expect(tree.importPacket(JSON.stringify(packet))).toBe(true);
    expect(tree.getState().nodes).toHaveLength(1); expect(tree.getState().revision).toBe(before.revision + 1);
    tree.undo(); expect(withoutRevision(tree.getState())).toEqual(withoutRevision(before));
    tree.redo(); expect(tree.getState().nodes[0].title).toBe('Another tree');
  });

  it('rejects malformed JSON, oversized imports, invalid commands and edits to material bytes', () => {
    const tree = model(); const source = tree.addMaterial('work'); const before = tree.exportPacket();
    for (const command of [
      () => tree.importPacket('{'), () => tree.importPacket(' '.repeat(500_001)),
      () => tree.editDraft(source, { title: 'Alter witness' }),
      () => tree.moveNode(source, [1, Infinity, 2]),
      () => tree.addDraft({ kind: 'note', title: 'Invalid reference', sourceIds: ['absent'] }),
      () => tree.connect(source, source, 'relates'), () => tree.addMaterial('absent'),
    ]) { expect(command).toThrow(); expect(tree.exportPacket()).toBe(before); }
  });
});

describe('constructor persistence failures', () => {
  it('protects saved bytes when the available library revision has changed', () => {
    const storage = memoryStorage(); const tree = model({ storage }); tree.seed();
    const saved = storage.values.get('tos.constructor.v1'); storage.setItem.mockClear();
    const updatedLibrary = library(); updatedLibrary.fingerprint = 'c'.repeat(64);
    const restored = createConstructorModel(updatedLibrary, { storage });
    expect(restored.getState().nodes).toEqual([]); expect(restored.persistenceError()).toContain('fingerprint');
    restored.seed();
    expect(storage.setItem).not.toHaveBeenCalled(); expect(storage.values.get('tos.constructor.v1')).toBe(saved);
  });

  it('keeps corrupt restored storage protected even after edits, undo, clear and import', () => {
    const storage = memoryStorage(); const corrupt = '{"unreadable":';
    storage.values.set('tos.constructor.v1', corrupt);
    const tree = model({ storage }); const other = model(); other.seed();
    expect(tree.persistenceError()).toBeTruthy();
    tree.addDraft({ kind: 'note', title: 'Unsaved local work' }); tree.undo(); tree.clear(); tree.importPacket(other.exportPacket());
    expect(storage.values.get('tos.constructor.v1')).toBe(corrupt);
    expect(storage.setItem).not.toHaveBeenCalled(); expect(tree.persistenceError()).toBeTruthy();
    expect(JSON.parse(tree.exportPacket()).nodes).toHaveLength(5);
  });

  it('retains working state and history after quota failure and retries a later save', () => {
    const storage = memoryStorage(); const tree = model({ storage });
    storage.setItem.mockImplementationOnce(() => { throw new Error('quota exceeded'); });
    const id = tree.addDraft({ kind: 'note', title: 'Recoverable work' });
    expect(tree.getState().nodes[0].id).toBe(id); expect(tree.canUndo()).toBe(true);
    expect(tree.persistenceError()).toContain('quota exceeded');
    tree.editDraft(id, { body: 'Now saved' }); expect(tree.persistenceError()).toBeNull();
    expect(model({ storage }).getState()).toEqual(tree.getState());
  });

  it('keeps named storage keys independent', () => {
    const storage = memoryStorage(); const a = model({ storage, key: 'a' }); const b = model({ storage, key: 'b' });
    a.rename('First'); b.rename('Second');
    expect(model({ storage, key: 'a' }).getState().title).toBe('First');
    expect(model({ storage, key: 'b' }).getState().title).toBe('Second');
  });
});
