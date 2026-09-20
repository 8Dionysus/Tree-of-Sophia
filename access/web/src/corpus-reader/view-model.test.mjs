import {describe, expect, it, vi} from 'vitest';
import {
  READER_LIMITS,
  createCorpusReaderModel,
  exactReference,
  normalizeCatalog,
  normalizeDocument,
  normalizeWindow,
  searchUnits,
  unitReference,
} from './view-model.mjs';
import {createReference, isReference} from './model.mjs';
import {createFixtureProvider} from './fixture-provider.mjs';
import {createCorpusNotebook, createMemoryCorpusNotebookState} from './notebook.mjs';

function sourceFor(workId, suffix = 'a') {
  return {
    workId,
    expressionId: `${workId}-expression-${suffix}`,
    editionId: `${workId}-edition-${suffix}`,
    itemId: `${workId}-item-${suffix}`,
    fileId: `${workId}-file-${suffix}`,
    fileSha256: suffix.repeat(64),
    textLayerRef: `${workId}/text/${suffix}.txt`,
    textLayerSha256: suffix.repeat(64),
  };
}

const source = sourceFor('work-1');
const version = {
  id: 'ru-1', language: 'ru', status: 'available', source,
  revision: source.textLayerSha256, sourceRevision: source.fileSha256,
  contentRevision: source.textLayerSha256, edition: 'Первое издание',
};
const document = {id: 'work-1', title: {ru: 'Работа'}, versions: [version]};
const units = Array.from({length: READER_LIMITS.windowUnits + 4}, (_, index) => ({
  id: `unit-${index}`, text: `Строка ${index} — ζωή`, ordinal: index + 1,
}));

async function waitForLength(value, expected, attempts = 40) {
  for (let index = 0; index < attempts && value.length < expected; index += 1) {
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  expect(value).toHaveLength(expected);
}

describe('corpus reader view model', () => {
  it('normalizes a catalog without losing bilingual identity or edition metadata', () => {
    const page = normalizeCatalog({items: [document], next_cursor: 'next', total: 1});
    expect(page.items[0].id).toBe('work-1');
    expect(page.items[0].versions[0]).toMatchObject({id: 'ru-1', language: 'ru', contentRevision: source.textLayerSha256});
    expect(page.nextCursor).toBe('next');
  });

  it('bounds a returned window and derives a canonical reference only from source metadata', () => {
    const result = normalizeWindow({units, previousCursor: null, nextCursor: 'next'}, {document, version});
    expect(result.units).toHaveLength(READER_LIMITS.windowUnits);
    expect(result.truncated).toBe(true);
    expect(result.previous).toBeNull();
    expect(result.hasPrevious).toBe(false);
    expect(result.next).toBe('next');
    expect(result.units[0].reference.target.textLayerSha256).toBe(version.source.textLayerSha256);
    const reference = unitReference(result.units[0], version, document);
    expect(reference).toMatchObject({
      schemaVersion: 'tos.corpus.reader.reference.v1',
      versionId: 'ru-1',
      unitId: 'unit-0',
      target: {workId: 'work-1', textLayerSha256: source.textLayerSha256},
    });
    expect(reference).not.toHaveProperty('text');
    expect(result.units[0].text).toContain('ζωή');
  });

  it('keeps unknown counts and caps legacy manifest payloads', () => {
    const page = normalizeCatalog({items: [{...document, unitCount: undefined}], nextCursor: 'more'});
    expect(page.total).toBeNull();
    expect(page.items[0].unitCount).toBeNull();
    expect(page.items[0].versions[0].units).toHaveLength(0);
    const manifest = normalizeDocument({
      ...document,
      unitCount: undefined,
      versions: [{...version, units: Array.from({length: 100}, (_, index) => ({id: `u-${index}`, text: 'x'}))}],
    });
    expect(manifest.unitCount).toBeNull();
    expect(manifest.versions[0].units).toHaveLength(READER_LIMITS.windowUnits);
    expect(manifest.versions[0].unitsTruncated).toBe(true);
  });

  it('searches literal Unicode text and returns exact code-point offsets', () => {
    const reference = createReference({source, versionId: 'ru-1', unitId: 'u'});
    const result = searchUnits([{id: 'u', label: 'Абзац о слове', ordinal: 3, text: 'слово ζωή слово', reference}], 'ζωή');
    expect(result.items[0]).toMatchObject({unitId: 'u', label: 'Абзац о слове', ordinal: 3, start: 6, end: 9, scope: 'loaded'});
    expect(result.items[0].reference.selector).toMatchObject({start: 6, end: 9, positionUnit: 'unicode_code_point'});
    expect(result.items[0].reference).not.toHaveProperty('quote');
  });

  it('keeps a bounded loaded search total unknown when the result cap is reached', () => {
    const result = searchUnits(Array.from({length: 3}, (_, index) => ({id: `u-${index}`, text: 'repeat'})), 'repeat', {limit: 2});
    expect(result.items).toHaveLength(2);
    expect(result.total).toBeNull();
    expect(result.truncated).toBe(true);
  });

  it('accepts only the canonical reference shape and strips delivered wording', () => {
    expect(exactReference({documentId: 'work-1', versionId: 'ru-1', unitId: 'unit-2', start: 4, end: 12})).toBeNull();
    const canonical = createReference({source, versionId: 'ru-1', unitId: 'unit-2', start: 4, end: 12});
    const reference = exactReference({...canonical, text: 'quoted', paragraph: 'derived'});
    expect(reference).toEqual(canonical);
    expect(reference).not.toHaveProperty('text');
    expect(reference).not.toHaveProperty('paragraph');
    expect(isReference(reference)).toBe(true);
  });

  it('keeps the newest document window when an older open resolves late', async () => {
    const documentA = {id: 'work-a', title: 'A', versions: [{...version, id: 'ru-a', source: sourceFor('work-a', 'a'), revision: 'a'.repeat(64)}]};
    const documentB = {id: 'work-b', title: 'B', versions: [{...version, id: 'ru-b', source: sourceFor('work-b', 'b'), revision: 'b'.repeat(64)}]};
    const pending = [];
    const provider = {
      catalog: vi.fn(async () => ({items: [documentA, documentB]})),
      document: vi.fn(async ({documentId}) => documentId === 'work-a' ? documentA : documentB),
      window: vi.fn(({documentId}) => new Promise(resolve => pending.push({documentId, resolve}))),
    };
    const model = createCorpusReaderModel({provider});
    await model.catalog();
    const first = model.open({documentId: 'work-a', versionId: 'ru-a'});
    await waitForLength(pending, 1);
    const second = model.open({documentId: 'work-b', versionId: 'ru-b'});
    await waitForLength(pending, 2);
    pending[1].resolve({units: [{id: 'new', text: 'новое'}]});
    pending[0].resolve({units: [{id: 'old', text: 'старое'}]});
    await Promise.all([first, second]);
    const snapshot = model.snapshot();
    expect(snapshot.document.id).toBe('work-b');
    expect(snapshot.windows['work-b\u0000ru-b'].units[0].id).toBe('new');
    expect(snapshot.windows['work-a\u0000ru-a']).toBeUndefined();
  });

  it('does not fall back from an explicitly requested unavailable version', async () => {
    const provider = {
      catalog: async () => ({items: [document]}),
      document: async () => document,
      window: async () => ({units: [{id: 'unit-1', text: 'text'}]}),
    };
    const model = createCorpusReaderModel({provider});
    await model.catalog();
    await expect(model.open({documentId: 'work-1', versionId: 'missing-version'})).rejects.toThrow('version-unavailable');
    expect(model.snapshot().activeVersionId).toBeNull();
  });

  it('marks a compact deep link changed without retargeting the current window', async () => {
    const provider = {
      catalog: async () => ({items: [document]}),
      document: async () => document,
      window: async ({unitId}) => ({units: [{id: unitId || 'unit-1', text: 'новая версия'}]}),
    };
    const model = createCorpusReaderModel({provider});
    await model.catalog();
    const stale = await model.open({documentId: 'work-1', versionId: 'ru-1', unitId: 'unit-2', revision: '0'.repeat(64)});
    expect(stale).toBeNull();
    expect(model.snapshot().referenceStatus).toBe('changed');
    expect(model.snapshot().windows['work-1\u0000ru-1']).toBeUndefined();
  });

  it('hydrates a catalog summary before opening the first document with no arguments', async () => {
    const provider = createFixtureProvider({documentCount: 1, unitCount: 80, languages: ['ru', 'en'], originalLanguage: 'ru'});
    const model = createCorpusReaderModel({provider});
    await model.catalog();
    expect(model.snapshot().catalog.items[0].versions).toHaveLength(0);
    const opened = await model.open();
    expect(opened.units).toHaveLength(READER_LIMITS.windowUnits);
    expect(model.snapshot().document.versions.length).toBe(2);
  });

  it('pages provider search at the corpus limit and retains only a bounded result cache', async () => {
    const base = createFixtureProvider({documentCount: 1, unitCount: 240, languages: ['ru'], originalLanguage: 'ru'});
    const calls = [];
    const provider = {...base, search: async args => { calls.push(args); return base.search(args); }};
    const model = createCorpusReaderModel({provider});
    await model.open();
    const firstPage = await model.search({query: 'synthetic', scope: 'version'});
    expect(firstPage.items).toHaveLength(READER_LIMITS.searchResults);
    expect(firstPage.nextCursor).toBeTruthy();
    expect(calls[0].limit).toBe(100);
    const secondPage = await model.search({query: 'synthetic', scope: 'version', cursor: firstPage.nextCursor});
    expect(secondPage.items).toHaveLength(200);
    expect(calls[1].limit).toBe(100);
    expect(secondPage.nextCursor).toBeTruthy();
    expect(secondPage.items.length).toBeLessThanOrEqual(300);
  });

  it('retains a readable window when the next provider page is unavailable', async () => {
    const provider = createFixtureProvider({documentCount: 1, unitCount: 120, languages: ['ru'], originalLanguage: 'ru'});
    const model = createCorpusReaderModel({provider});
    const first = await model.open();
    const document = model.snapshot().document;
    const version = document.versions[0];
    provider.controls.setFailure('window', {code: 'unavailable', message: 'continuation unavailable'});
    expect(await model.loadWindow({versionId: version.id, direction: 'next'})).toBeNull();
    const retained = model.snapshot().windows[`${document.id}\u0000${version.id}`];
    expect(retained.units[0].id).toBe(first.units[0].id);
    expect(model.snapshot().windowErrors[`${document.id}\u0000${version.id}`]).toMatchObject({message: 'continuation unavailable'});
    expect(model.snapshot().error).toBeNull();
  });

  it('exposes an empty intermediate page as an explicit continuation', async () => {
    const provider = createFixtureProvider({
      documentCount: 2,
      unitCount: 240,
      languages: ['ru'],
      originalLanguage: 'ru',
      emptyPages: {catalog: true, search: true},
    });
    const model = createCorpusReaderModel({provider});
    const firstCatalog = await model.catalog({limit: 1});
    expect(firstCatalog.items).toHaveLength(0);
    expect(firstCatalog.nextCursor).toEqual(expect.any(String));
    const secondCatalog = await model.catalog({limit: 1, cursor: firstCatalog.nextCursor});
    expect(secondCatalog.items).toHaveLength(1);
    await model.open({documentId: secondCatalog.items[0].id, versionId: secondCatalog.items[0].versions[0]?.id});
    const firstSearch = await model.search({query: 'synthetic', scope: 'version'});
    expect(firstSearch.items).toHaveLength(0);
    expect(firstSearch.nextCursor).toEqual(expect.any(String));
    const secondSearch = await model.search({query: 'synthetic', scope: 'version', cursor: firstSearch.nextCursor});
    expect(secondSearch.items.length).toBeGreaterThan(0);
  });

  it('gates version and corpus search by advertised provider capabilities', async () => {
    const calls = [];
    const base = createFixtureProvider({documentCount: 1, unitCount: 8, languages: ['ru'], originalLanguage: 'ru'});
    const provider = {
      ...base,
      capabilities: {...base.capabilities, search: true, searchScopes: ['version']},
      search: async args => { calls.push(args); return base.search(args); },
    };
    const model = createCorpusReaderModel({provider});
    await model.open();
    expect(model.searchCapabilities()).toMatchObject({loaded: true, version: true, corpus: false});
    await expect(model.search({query: 'synthetic', scope: 'corpus'})).rejects.toThrow('corpus-search-unavailable');
    expect(calls).toHaveLength(0);
    const disabled = {...provider, capabilities: {...provider.capabilities, search: false}};
    const disabledModel = createCorpusReaderModel({provider: disabled});
    await disabledModel.open();
    expect(disabledModel.searchCapabilities()).toMatchObject({version: false, corpus: false});
    await expect(disabledModel.search({query: 'synthetic', scope: 'version'})).rejects.toThrow('version-search-unavailable');
  });

  it('keeps the catalog cache bounded while preserving page order', async () => {
    const provider = createFixtureProvider({documentCount: 120, unitCount: 1, languages: ['ru'], originalLanguage: 'ru'});
    const model = createCorpusReaderModel({provider});
    let page = await model.catalog();
    while (page.nextCursor) page = await model.catalog({cursor: page.nextCursor});
    const items = model.snapshot().catalog.items;
    expect(items).toHaveLength(READER_LIMITS.catalogPage * 4);
    expect(items[0].id).toBe('tos.work.fixture-000025');
    expect(items.at(-1).id).toBe('tos.work.fixture-000120');
  });

  it('stores strict notes with targeted CAS and keeps reading positions per version', async () => {
    const provider = createFixtureProvider({documentCount: 1, unitCount: 120, languages: ['ru', 'en', 'de'], originalLanguage: 'ru'});
    const memoryStore = createMemoryCorpusNotebookState();
    const notebookA = createCorpusNotebook({adapter: 'memory', memoryStore});
    const notebookB = createCorpusNotebook({adapter: 'memory', memoryStore});
    const modelA = createCorpusReaderModel({provider, notebook: notebookA});
    await modelA.ready();
    const opened = await modelA.open();
    const documentA = modelA.snapshot().document;
    const ru = documentA.versions.find(item => item.language === 'ru');
    const ruReference = unitReference(opened.units[3], ru, documentA, {start: 0, end: 8});
    await modelA.saveNote(ruReference, 'Первое состояние', 'точная цитата');
    await modelA.savePosition(ruReference, 0, 'primary');
    // Reading writes advance the notebook globally, but should not invalidate
    // a note update guarded by the note record's own revision.
    await modelA.saveNote(ruReference, 'Обновлённое состояние', 'точная цитата');
    expect(modelA.notebookState().notes[0]).toMatchObject({text: 'Обновлённое состояние', quote: 'точная цитата'});

    await notebookB.close();
    const notebookC = createCorpusNotebook({adapter: 'memory', memoryStore});
    const modelC = createCorpusReaderModel({provider, notebook: notebookC});
    await modelC.ready();
    const beforeConflict = modelC.notebookState().notes[0];
    await modelA.saveNote(ruReference, 'Третье состояние', 'точная цитата');
    await expect(modelC.saveNote(ruReference, 'Устаревшее состояние')).rejects.toMatchObject({code: 'conflict'});
    expect(modelC.notebookState().notes[0].revision).toBe(beforeConflict.revision);

    const en = documentA.versions.find(item => item.language === 'en');
    const enWindow = await modelA.loadWindow({versionId: en.id});
    const enReference = unitReference(enWindow.units[4], en, documentA, {start: 0, end: 5});
    await modelA.savePosition(enReference, 0, 'secondary');
    const de = documentA.versions.find(item => item.language === 'de');
    const deWindow = await modelA.loadWindow({versionId: de.id});
    const deReference = unitReference(deWindow.units[5], de, documentA, {start: 0, end: 5});
    await modelA.savePosition(deReference, 0, 'primary');
    expect(modelA.notebookState().positions.filter(item => isReference(item.reference))).toHaveLength(3);

    const resumed = createCorpusReaderModel({provider, notebook: notebookC});
    await resumed.ready();
    const resumedRu = await resumed.open({documentId: documentA.id, versionId: ru.id});
    expect(resumedRu.units[0].id).toBe(opened.units[3].id);
    await notebookA.close();
    await notebookC.close();
  });

  it('loads older notebook records through a bounded page seam', async () => {
    const notebook = createCorpusNotebook({adapter: 'memory'});
    const model = createCorpusReaderModel({notebook});
    const records = Array.from({length: 105}, (_, index) => createReference({
      source, versionId: 'ru-1', unitId: `unit-${index + 1}`,
    }));
    for (const reference of records) await notebook.putNote({reference, kind: 'note', text: 'record'});
    await model.ready();
    const firstPage = await model.notebookPage({documentId: 'work-1'});
    expect(firstPage.items).toHaveLength(100);
    expect(firstPage.nextCursor).toBeTruthy();
    expect(model.notebookState().nextCursor).toBe(firstPage.nextCursor);
    const secondPage = await model.notebookPage({cursor: firstPage.nextCursor, documentId: 'work-1'});
    expect(secondPage.items).toHaveLength(5);
    expect(model.notebookState().notes).toHaveLength(105);
    expect(model.notebookState().nextCursor).toBeNull();
    await notebook.close();
  });

  it('reads off-page document metadata without navigating or evicting the catalog', async () => {
    const offPage = {
      id: 'work-off-page',
      title: {ru: 'Работа вне страницы'},
      versions: [{id: 'ru-off-page', language: 'ru', status: 'available'}],
    };
    const provider = {
      catalog: vi.fn(async () => ({items: [{id: 'work-on-page', title: 'На странице', versions: []}]})),
      document: vi.fn(async ({documentId}) => documentId === offPage.id ? offPage : null),
    };
    const model = createCorpusReaderModel({provider});
    await model.catalog();

    const result = await model.readDocumentMetadata(offPage.id);

    expect(result).toMatchObject({id: offPage.id, title: offPage.title, versions: [{id: 'ru-off-page'}]});
    expect(provider.document).toHaveBeenCalledWith(expect.objectContaining({documentId: offPage.id, signal: expect.any(AbortSignal)}));
    expect(model.snapshot()).toMatchObject({document: null, activeVersionId: null, catalog: {items: [{id: 'work-on-page'}]}});
  });
});
