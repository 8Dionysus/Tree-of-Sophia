import '../observatory/human-forms-wasm-test-runtime.mjs';
import {readFile} from 'node:fs/promises';
import {describe, expect, it} from 'vitest';
import {createPublicCorpusProvider} from '../corpus-reader/public-provider.mjs';
import {unitReference} from '../corpus-reader/view-model.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {createReference} from '../corpus-reader/model.mjs';
import {ensureSourceRecord, resolveSourceLocator} from './source-address.mjs';
import {createCompareAndSwapPersistence} from './workspace-persistence.mjs';
import {savePublicThought} from './workspace-view.mjs';

const corpusRoot = new URL('../../public/public-corpus/', import.meta.url);

async function localCorpusFetch(path) {
  const prefix = '/static/public-corpus/';
  if (!path.startsWith(prefix)) return new Response('{}', {status: 404});
  try {
    const bytes = await readFile(new URL(path.slice(prefix.length), corpusRoot));
    return new Response(bytes, {status: 200, headers: {'Content-Type': 'application/json'}});
  } catch {
    return new Response('{}', {status: 404, headers: {'Content-Type': 'application/json'}});
  }
}

describe('public research shelf source addresses', () => {
  it('stages and reopens a real long German canonical reference through its shelf owner', async () => {
    const provider = createPublicCorpusProvider({fetch: localCorpusFetch});
    const manifest = await provider.describe();
    const documentId = manifest.profiles.zarathustra.documentIds[0];
    const documentEntry = manifest.documents.find(item => item.id === documentId);
    const documentResponse = await localCorpusFetch(`/static/public-corpus/${documentEntry.manifest.path}`);
    const document = await documentResponse.json();
    const version = document.versions.find(item => item.id === 'gutenberg-7205-de');
    const unitId = manifest.profiles.zarathustra.journeys[0].unitId;
    const sourceWindow = await provider.window({documentId, versionId: version.id, unitId});
    const reference = unitReference({unitId: sourceWindow.units[0].id}, version, document, {start: 0, end: 20});
    expect(JSON.stringify(reference).length).toBeGreaterThan(1_024);
    expect((await provider.resolve({reference})).status).toBe('exact');

    const shelf = createResearchShelfStore({adapter: 'memory'});
    const workspace = createBrowserResearchWorkspace({persistence: false});
    try {
      const locator = await ensureSourceRecord(shelf, reference, 'Zarathustra · German source');
      expect(locator.length).toBeLessThanOrEqual(1_024);
      workspace.addHypothesis({id: 'long-source-thesis', title: 'A source-linked thesis', body: 'Keep the full reference with its owner.'});
      workspace.stageProposal({id: 'long-source-proposal', kind: 'interpretation', parentHypothesisId: 'long-source-thesis',
        targetId: 'long-source-thesis', statement: 'An exact source supports this proposal.', sourceRefs: [locator], evidenceRefs: [locator],
        confidencePosture: {value: 'unknown', meaning: 'maker_declared_uncertainty_not_truth_probability'}, actorOrigin: 'human',
        basePageRevision: 0, baseWorkspaceRevision: workspace.summary().revision, dataFingerprint: `sha256:${'a'.repeat(64)}`});

      const stored = workspace.getState().proposals[0];
      expect(stored.sourceRefs).toEqual([locator]);
      const reopened = await resolveSourceLocator(stored.sourceRefs[0], shelf, provider);
      expect(reopened.status).toBe('exact');
      expect(JSON.stringify(reopened.reference)).toBe(JSON.stringify(reference));
    } finally {
      workspace.dispose();
      await shelf.close();
    }
  });

  it('refuses to overwrite a workspace value written by another tab', () => {
    const values = new Map();
    const storage = {getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value)};
    const first = createCompareAndSwapPersistence(storage, 'workspace');
    const second = createCompareAndSwapPersistence(storage, 'workspace');
    expect(first.load()).toBeNull();
    expect(second.load()).toBeNull();
    first.save('first-tab');
    expect(() => second.save('stale-tab')).toThrow(/Another tab changed/);
    expect(storage.getItem('workspace')).toBe('first-tab');
  });

  it.each(['objection', 'revision'])('rejects an oversized generated %s statement before any owner mutation', async field => {
    const calls = {resolve: 0, shelf: 0, workspace: 0, notebook: 0};
    const workspace = {getState: () => { calls.workspace++; return {hypotheses: [], proposals: [], notes: []}; },
      addHypothesis: () => { calls.workspace++; }, stageProposal: () => { calls.workspace++; }};
    const shelf = {get: async () => null, list: async () => { calls.shelf++; return {items: [], nextCursor: null}; },
      save: async () => { calls.shelf++; }};
    const notebook = {exportPacket: async () => { calls.notebook++; return {notes: []}; }, putNote: async () => { calls.notebook++; }};
    const draft = {thesis: 'A thesis', grounds: [{}, {}], objection: '', objectionSources: [], revision: ''};
    draft[field] = 'x'.repeat(field === 'objection' ? 4_000 - 'Objection to thesis: '.length + 1 : 4_000 - 'Revises earlier thesis: '.length + 1);
    await expect(savePublicThought({ ...draft, api: {provider: {resolve: async () => { calls.resolve++; return {status: 'exact'}; }}, getQuestion: () => ''},
      workspace, shelf, notebook, labels: () => ({pick: 'pick', open: 'open', thesis: 'Thesis', objection: 'Objection', revision: 'Revision'}) }))
      .rejects.toThrow(/at most/);
    expect(calls).toEqual({resolve: 0, shelf: 0, workspace: 0, notebook: 0});
  });

  it('stages objection and revision statements at their exact Rust workspace boundary', async () => {
    const makeReference = suffix => createReference({source: {workId: `work-${suffix}`, expressionId: `expression-${suffix}`,
      editionId: `edition-${suffix}`, itemId: `item-${suffix}`, fileId: `file-${suffix}`, fileSha256: 'a'.repeat(64),
      textLayerRef: `${suffix}/text.txt`, textLayerSha256: 'b'.repeat(64)}, versionId: `version-${suffix}`, unitId: `unit-${suffix}`, start: 0, end: 1});
    const sources = ['one', 'two'].map(suffix => ({reference: makeReference(suffix), title: `Source ${suffix}`, quote: 'x'}));
    const shelf = createResearchShelfStore({adapter: 'memory'});
    const workspace = createBrowserResearchWorkspace({persistence: false});
    const notebook = createCorpusNotebook({indexedDB: null, dbName: `tos-boundary-${Date.now()}`});
    try {
      await savePublicThought({thesis: 'A thesis', grounds: sources, objection: 'o'.repeat(4_000 - 'Objection to thesis: '.length),
        objectionSources: [sources[0]], revision: 'r'.repeat(4_000 - 'Revises earlier thesis: '.length),
        api: {provider: {resolve: async ({reference}) => ({status: 'exact', reference})}, getQuestion: () => 'x'.repeat(511) + '😀'},
        workspace, shelf, notebook, labels: () => ({pick: 'pick', open: 'open', thesis: 'Thesis', objection: 'Objection', revision: 'Revision'})});
      const statements = workspace.getState().proposals.map(item => item.statement);
      expect(workspace.getState().hypotheses[0].title).toBe('x'.repeat(511));
      expect(statements.map(value => value.length)).toEqual([8, 4_000, 4_000]);
    } finally {
      workspace.dispose();
      await shelf.close();
      await notebook.close();
    }
  });
});
