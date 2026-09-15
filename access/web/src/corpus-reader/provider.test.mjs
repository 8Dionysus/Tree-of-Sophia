import {expect, test} from 'vitest';
import {
  CorpusProviderCursorError,
  CorpusProviderError,
  CorpusProviderUnavailableError,
  CorpusProviderUnsupportedScopeError,
  createCorpusProvider,
  createTransportProvider,
} from './provider.mjs';
import {createFixtureProvider} from './fixture-provider.mjs';
import {createReference, referenceKey, validateReference} from './model.mjs';

async function fixture(options = {}) {
  const provider = createFixtureProvider({documentCount: 3, unitCount: 18, ...options});
  const catalog = await provider.catalog({limit: 1});
  const document = await provider.document({documentId: catalog.items[0].id});
  const version = document.versions[0];
  return {provider, catalog, document, version};
}

const VALID_SOURCE = Object.freeze({
  workId: 'work-1',
  expressionId: 'expression-1',
  editionId: 'edition-1',
  itemId: 'item-1',
  fileId: 'file-1',
  fileSha256: 'a'.repeat(64),
  textLayerRef: 'fixture/source.txt',
  textLayerSha256: 'b'.repeat(64),
});

test('references retain source hierarchy, layer digest, unit and selector without wording', async () => {
  const {provider, version} = await fixture();
  const page = await provider.window({documentId: version.source.workId, versionId: version.id, limit: 1});
  const reference = createReference({source: version.source, versionId: version.id, unitId: page.units[0].id, start: 2, end: 8});
  const normalized = validateReference({...reference, text: 'must not be retained'});
  expect(normalized).toEqual(reference);
  expect(JSON.stringify(normalized)).not.toContain('must not be retained');
  expect(referenceKey(reference)).toBe(referenceKey(structuredClone(reference)));
  expect(referenceKey({...reference, target: {...reference.target, textLayerSha256: 'b'.repeat(64)}})).not.toBe(referenceKey(reference));
});

test('large logical fixture pages remain bounded and do not eagerly materialize the corpus', async () => {
  const provider = createFixtureProvider({documentCount: 10_000, unitCount: 100_000});
  const catalog = await provider.catalog({limit: 3});
  expect(catalog.total).toBe(10_000);
  expect(catalog.items).toHaveLength(3);
  const document = await provider.document({documentId: catalog.items[0].id});
  const page = await provider.window({documentId: document.id, versionId: document.versions[0].id, limit: 4});
  expect(page.total).toBe(10);
  expect(page.units).toHaveLength(4);
  expect(provider.metrics()).toMatchObject({
    logical: {documentCount: 10_000, unitCount: 100_000},
    maxPageItems: 3,
    maxPageUnits: 4,
  });
  expect(provider.metrics().materializedCatalogItems).toBe(3);
  expect(provider.metrics().materializedUnits).toBe(4);
});

test('longDocumentUnits exposes one long work while keeping the rest of the catalog cheap', async () => {
  const provider = createFixtureProvider({documentCount: 10_000, unitCount: 100_000, longDocumentUnits: 100_000});
  const catalog = await provider.catalog({limit: 1});
  expect(catalog.items[0].unitCount).toBe(100_000);
  expect(provider.metrics().logical).toMatchObject({
    documentCount: 10_000,
    unitCount: 100_000,
    longDocumentUnits: 100_000,
    totalUnits: 199_990,
  });
  const document = await provider.document({documentId: catalog.items[0].id});
  const version = document.versions[0];
  let cursor = null;
  let page = null;
  for (let index = 0; index < 100; index += 1) {
    page = await provider.window({documentId: document.id, versionId: version.id, cursor, limit: 100});
    cursor = page.nextCursor;
  }
  expect(page.units.at(-1).ordinal).toBe(10_000);
  expect(provider.metrics().maxPageUnits).toBe(100);
});

test('optional structure pages expose explicit chapter anchors and stale cursors', async () => {
  const {provider, document, version} = await fixture({documentCount: 1, unitCount: 70});
  expect(provider.capabilities.structure).toBe(true);
  const first = await provider.structure({documentId: document.id, versionId: version.id, limit: 2});
  const window = await provider.window({documentId: document.id, versionId: version.id, limit: 100});
  expect(first).toMatchObject({documentId: document.id, versionId: version.id, total: 3});
  expect(first.items).toHaveLength(2);
  expect(first.items[0]).toMatchObject({unitId: window.units[0].id, level: 0, hasChildren: false});
  expect(first.items[1].unitId).toBe(window.units[33].id);
  expect(first.items[0]).not.toHaveProperty('parentId');
  const second = await provider.structure({documentId: document.id, versionId: version.id, cursor: first.nextCursor, limit: 2});
  expect(second.items[0].unitId).toBe(window.units[66].id);
  provider.controls.bumpRevision({documentId: document.id, versionId: version.id});
  await expect(provider.structure({documentId: document.id, versionId: version.id, cursor: first.nextCursor, limit: 2}))
    .rejects.toBeInstanceOf(CorpusProviderCursorError);
});

test('fixture language tags stay aligned with Arabic and Greek synthetic text', async () => {
  const provider = createFixtureProvider({documentCount: 1, unitCount: 4, languages: ['ar', 'el'], originalLanguage: 'ar'});
  const document = await provider.document({documentId: 'tos.work.fixture-000001'});
  const arabic = await provider.window({documentId: document.id, versionId: document.versions[0].id, limit: 4});
  const greek = await provider.window({documentId: document.id, versionId: document.versions[1].id, limit: 4});
  expect(arabic.units[0]).toMatchObject({language: 'ar'});
  expect(arabic.units[0].text).toMatch(/[\u0600-\u06ff]/u);
  expect(greek.units[0]).toMatchObject({language: 'el'});
  expect(greek.units[0].text).toMatch(/[\u0370-\u03ff]/u);
});

test('window supports exact compact deep links and rejects revision or version mismatches', async () => {
  const {provider, document, version} = await fixture();
  const first = await provider.window({documentId: document.id, versionId: version.id, limit: 1});
  const link = {unitId: first.units[0].id, revision: first.revision};
  const resumed = await provider.window({documentId: document.id, versionId: version.id, reference: link, limit: 1});
  expect(resumed.units[0].id).toBe(first.units[0].id);
  const resumedFromRoute = await provider.window({documentId: document.id, versionId: version.id, ...link, limit: 1});
  expect(resumedFromRoute.units[0].id).toBe(first.units[0].id);
  await expect(provider.window({documentId: document.id, versionId: version.id, reference: {...link, revision: 'f'.repeat(64)}, limit: 1}))
    .rejects.toMatchObject({code: 'stale_reference'});
  const otherVersion = document.versions[1];
  await expect(provider.window({documentId: document.id, versionId: otherVersion.id, reference: link, limit: 1}))
    .rejects.toMatchObject({code: 'invalid_request'});
});

test('cursor pagination is opaque and becomes stale after a fixture revision change', async () => {
  const {provider, document, version} = await fixture({unitCount: 20});
  const first = await provider.window({documentId: document.id, versionId: version.id, limit: 2});
  expect(first.nextCursor).toEqual(expect.any(String));
  provider.controls.bumpRevision();
  await expect(provider.window({documentId: document.id, versionId: version.id, cursor: first.nextCursor, limit: 2}))
    .rejects.toBeInstanceOf(CorpusProviderCursorError);
});

test('document search paginates independently from text windows and exposes unsupported scopes', async () => {
  const {provider, document, version} = await fixture({unitCount: 60});
  const first = await provider.search({documentId: document.id, versionId: version.id, query: 'synthetic', scope: 'version', limit: 2});
  expect(first.total).toBe(20);
  expect(first.items).toHaveLength(2);
  const second = await provider.search({documentId: document.id, versionId: version.id, query: 'synthetic', scope: 'version', cursor: first.nextCursor, limit: 2});
  expect(second.items[0].unitId).not.toBe(first.items[0].unitId);
  await expect(provider.search({documentId: document.id, versionId: version.id, query: 'synthetic', scope: 'corpus', limit: 2}))
    .rejects.toBeInstanceOf(CorpusProviderUnsupportedScopeError);
});

test('resolve distinguishes exact, changed and unavailable source identities', async () => {
  const {provider, document, version} = await fixture();
  const page = await provider.window({documentId: document.id, versionId: version.id, limit: 1});
  const reference = createReference({source: version.source, versionId: version.id, unitId: page.units[0].id, start: 0, end: 4});
  await expect(provider.resolve({reference})).resolves.toMatchObject({status: 'exact', reference});
  const retargeted = createCorpusProvider({provider: {
    ...provider,
    resolve: async () => ({status: 'exact', reference: {...reference, unitId: 'different-unit'}}),
  }});
  await expect(retargeted.resolve({reference})).rejects.toMatchObject({code: 'invalid_response'});
  provider.controls.markChanged(reference);
  await expect(provider.resolve({reference})).resolves.toMatchObject({status: 'changed'});
  provider.controls.clearChanged(reference);
  const changed = {...reference, target: {...reference.target, textLayerSha256: 'c'.repeat(64)}};
  await expect(provider.resolve({reference: changed})).resolves.toMatchObject({status: 'changed'});
  const unavailable = {...reference, target: {...reference.target, workId: 'tos.work.fixture-999999'}};
  await expect(provider.resolve({reference: unavailable})).resolves.toMatchObject({status: 'unavailable'});
});

test('fixture controls cover latency cancellation, failures and bounded metrics', async () => {
  const {provider, document, version} = await fixture({latencyMs: 30});
  const controller = new AbortController();
  const pending = provider.window({documentId: document.id, versionId: version.id, signal: controller.signal, limit: 1});
  controller.abort();
  await expect(pending).rejects.toMatchObject({name: 'AbortError'});
  provider.controls.setFailure('document', true);
  await expect(provider.document({documentId: document.id})).rejects.toMatchObject({code: 'fixture_failure'});
  provider.controls.clearFailure('document');
  expect(provider.metrics().aborted).toBe(1);
  expect(provider.metrics().failed).toBe(1);
});

test('long searches yield between bounded scan batches and honor mid-scan cancellation', async () => {
  const provider = createFixtureProvider({documentCount: 1, unitCount: 1, longDocumentUnits: 100_000});
  const documentId = 'tos.work.fixture-000001';
  const versionId = 'tos.expression.fixture-000001-de';
  const controller = new AbortController();
  const pending = provider.search({documentId, versionId, query: 'synthetic', scope: 'version', limit: 1, signal: controller.signal});
  setTimeout(() => controller.abort(), 0);
  await expect(pending).rejects.toMatchObject({name: 'AbortError'});
  expect(provider.metrics().aborted).toBe(1);
  expect(provider.metrics().materializedUnits).toBeGreaterThan(0);
  expect(provider.metrics().materializedUnits).toBeLessThan(100_000);
});

test('real host stays explicitly unavailable until a provider or caller transport is supplied', async () => {
  const unavailable = createCorpusProvider();
  await expect(unavailable.catalog({limit: 1})).rejects.toBeInstanceOf(CorpusProviderUnavailableError);
  expect(unavailable.capabilities).toMatchObject({search: false, catalog: false});

  const calls = [];
  const transport = createTransportProvider({
    capabilities: {search: true, searchScopes: ['corpus']},
    request(operation, args) {
      calls.push([operation, args]);
      if (operation === 'catalog') return {items: [{id: 'work-1', title: 'Work'}], nextCursor: null, total: 1};
      throw new CorpusProviderError('fixture_failure', 'test');
    },
  });
  await expect(transport.catalog({query: '', limit: 1})).resolves.toMatchObject({total: 1});
  expect(calls[0][0]).toBe('catalog');
  expect(calls[0][1].limit).toBe(1);
  expect(transport.capabilities).toMatchObject({search: true, searchScopes: ['corpus']});
});

test('bound providers reject response identity drift and revision drift before the UI sees it', async () => {
  const validVersion = {
    id: 'version-1', language: 'de', role: 'original', revision: 'a'.repeat(64), unitCount: 1,
    source: VALID_SOURCE,
  };
  const base = {
    catalog: async () => ({items: [], nextCursor: null, total: 0}),
    document: async () => ({id: 'wrong-work', title: 'Wrong', versions: [validVersion]}),
    window: async () => ({documentId: 'wrong-work', versionId: 'wrong-version', revision: 'b'.repeat(64), units: [], previousCursor: null, nextCursor: null, total: 0}),
    search: async () => ({items: [], nextCursor: null, total: 0}),
    resolve: async () => ({status: 'unavailable'}),
  };
  const provider = createCorpusProvider({provider: base});
  await expect(provider.document({documentId: 'work-1'})).rejects.toMatchObject({code: 'invalid_response'});
  await expect(provider.window({documentId: 'work-1', versionId: 'version-1', limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});

  const revisionProvider = createCorpusProvider({provider: {
    ...base,
    document: async () => ({id: 'work-1', title: 'Work', versions: [validVersion]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'b'.repeat(64), units: [], previousCursor: null, nextCursor: null, total: 0}),
  }});
  await expect(revisionProvider.window({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});
});

test('bound document manifests provide createReference-ready source metadata and reject duplicates', async () => {
  const version = {id: 'version-1', language: 'de', role: 'original', revision: 'a'.repeat(64), unitCount: 1, source: VALID_SOURCE};
  const core = {
    catalog: async () => ({items: [], nextCursor: null, total: 0}),
    document: async () => ({id: 'work-1', title: 'Work', versions: [version]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [], previousCursor: null, nextCursor: null, total: 0}),
    search: async () => ({items: [], nextCursor: null, total: 0}),
    resolve: async () => ({status: 'unavailable'}),
  };
  const validProvider = createCorpusProvider({provider: core});
  await expect(validProvider.document({documentId: 'work-1'})).resolves.toMatchObject({id: 'work-1'});
  expect(createReference({source: VALID_SOURCE, versionId: 'version-1', unitId: 'unit-1'}).target.textLayerSha256)
    .toBe(VALID_SOURCE.textLayerSha256);
  const incompleteSource = createCorpusProvider({provider: {
    ...core,
    document: async () => ({id: 'work-1', title: 'Work', versions: [{...version, source: {workId: 'work-1'}}]}),
  }});
  await expect(incompleteSource.document({documentId: 'work-1'})).rejects.toMatchObject({code: 'invalid_response'});
  const wrongSourceWork = createCorpusProvider({provider: {
    ...core,
    document: async () => ({id: 'work-1', title: 'Work', versions: [{...version, source: {...VALID_SOURCE, workId: 'work-2'}}]}),
  }});
  await expect(wrongSourceWork.document({documentId: 'work-1'})).rejects.toMatchObject({code: 'invalid_response'});
  const invalidDigest = createCorpusProvider({provider: {
    ...core,
    document: async () => ({id: 'work-1', title: 'Work', versions: [{...version, source: {...VALID_SOURCE, textLayerSha256: 'not-a-digest'}}]}),
  }});
  await expect(invalidDigest.document({documentId: 'work-1'})).rejects.toMatchObject({code: 'invalid_response'});
  const duplicateVersions = createCorpusProvider({provider: {
    ...core,
    document: async () => ({id: 'work-1', title: 'Work', versions: [version, {...version}]}),
  }});
  await expect(duplicateVersions.document({documentId: 'work-1'})).rejects.toMatchObject({code: 'invalid_response'});
  const unit = {id: 'unit-1', ordinal: 1, kind: 'paragraph', text: 'one', language: 'de', reference: 'chapter-001/unit-001'};
  const duplicateUnits = createCorpusProvider({provider: {
    ...core,
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [unit, {...unit}], previousCursor: null, nextCursor: null, total: 2}),
  }});
  await expect(duplicateUnits.window({documentId: 'work-1', versionId: 'version-1', limit: 2})).rejects.toMatchObject({code: 'invalid_response'});
});

test('structure is optional and bound providers validate its identity', async () => {
  const base = {
    catalog: async () => ({items: [], nextCursor: null, total: 0}),
    document: async () => ({id: 'work-1', title: 'Work', versions: [{id: 'version-1', language: 'de', role: 'original', revision: 'a'.repeat(64), unitCount: 1, source: VALID_SOURCE}]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [], previousCursor: null, nextCursor: null, total: 0}),
    search: async () => ({items: [], nextCursor: null, total: 0}),
    resolve: async () => ({status: 'unavailable'}),
  };
  const withoutStructure = createCorpusProvider({provider: base});
  expect(withoutStructure.structure).toBeUndefined();
  expect(withoutStructure.capabilities?.structure).not.toBe(true);
  const withStructure = createCorpusProvider({provider: {
    ...base,
    structure: async () => ({documentId: 'wrong-work', versionId: 'version-1', revision: 'a'.repeat(64), items: [], nextCursor: null, total: 0}),
  }});
  expect(withStructure.capabilities.structure).toBe(true);
  await expect(withStructure.structure({documentId: 'work-1', versionId: 'version-1', limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});
  let transportStructureCalls = 0;
  const transport = createTransportProvider({
    request: async () => ({items: [], nextCursor: null, total: 0}),
    structure: async args => {
      transportStructureCalls += 1;
      return {documentId: args.documentId, versionId: args.versionId, revision: 'a'.repeat(64), items: [], nextCursor: null, total: 0};
    },
  });
  expect(transport.capabilities.structure).toBe(true);
  await expect(transport.structure({documentId: 'work-1', versionId: 'version-1', limit: 1})).resolves.toMatchObject({total: 0});
  expect(transportStructureCalls).toBe(1);
});

test('bound providers reject oversized pages, text payloads and metadata', async () => {
  const base = {
    catalog: async () => ({items: [{id: 'w1', title: 'one'}, {id: 'w2', title: 'two'}], nextCursor: null, total: 2}),
    document: async () => ({id: 'work-1', title: 'Work', versions: [{id: 'version-1', language: 'de', role: 'original', revision: 'a'.repeat(64), unitCount: 1, source: VALID_SOURCE}]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [{id: 'u', ordinal: 1, kind: 'paragraph', text: 'x', language: 'de', reference: 'u'}], previousCursor: null, nextCursor: null, total: 1}),
    search: async () => ({items: [{unitId: 'u', reference: 'u', excerpt: 'x'}, {unitId: 'v', reference: 'v', excerpt: 'y'}], nextCursor: null, total: 2}),
    resolve: async () => ({status: 'unavailable'}),
  };
  await expect(createCorpusProvider({provider: base}).catalog({limit: 1})).rejects.toMatchObject({code: 'invalid_response'});
  await expect(createCorpusProvider({provider: base}).search({documentId: 'work-1', versionId: 'version-1', query: 'x', limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});

  const oversizedText = createCorpusProvider({provider: {...base, window: async () => ({
    documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64),
    units: [{id: 'u', ordinal: 1, kind: 'paragraph', text: 'x'.repeat(65_537), language: 'de', reference: 'u'}],
    previousCursor: null, nextCursor: null, total: 1,
  })}});
  await expect(oversizedText.window({documentId: 'work-1', versionId: 'version-1', limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});
});

test('unknown counts and metadata-only versions remain explicit at the provider boundary', async () => {
  const fixtureProvider = createFixtureProvider({
    documentCount: 2,
    unitCount: 18,
    unknownTotals: true,
    unknownUnitCounts: true,
    metadataOnlyDocuments: ['tos.work.fixture-000002'],
    versionAvailability: {'1:en': 'restricted'},
  });
  const provider = createCorpusProvider({provider: fixtureProvider});
  const catalog = await provider.catalog({limit: 2});
  expect(catalog.total).toBeNull();
  expect(catalog.items[0].unitCount).toBeNull();
  expect(catalog.items[1]).toMatchObject({status: 'metadata_only', unitCount: null, textAvailable: false});
  expect(catalog.items[0].versions.find(item => item.language === 'en')).toMatchObject({status: 'restricted', unitCount: null, available: false});

  const document = await provider.document({documentId: catalog.items[0].id});
  expect(document.unitCount).toBeNull();
  expect(document.versions.find(item => item.language === 'en')).toMatchObject({status: 'restricted', unitCount: null, source: null, revision: null});
  const window = await provider.window({documentId: document.id, versionId: document.versions[0].id, limit: 1});
  expect(window.total).toBeNull();
});

test('fixture can expose empty continuation pages without forcing a full scan', async () => {
  const fixtureProvider = createFixtureProvider({
    documentCount: 3,
    unitCount: 198,
    emptyPages: {catalog: true, window: true, search: true, structure: true},
  });
  const catalog = await fixtureProvider.catalog({limit: 1});
  expect(catalog.items).toHaveLength(0);
  expect(catalog.nextCursor).toEqual(expect.any(String));
  const catalogContinuation = await fixtureProvider.catalog({limit: 1, cursor: catalog.nextCursor});
  expect(catalogContinuation.items).toHaveLength(1);

  const document = await fixtureProvider.document({documentId: catalogContinuation.items[0].id});
  const version = document.versions[0];
  const emptyWindow = await fixtureProvider.window({documentId: document.id, versionId: version.id, limit: 2});
  expect(emptyWindow.units).toHaveLength(0);
  expect(emptyWindow.nextCursor).toEqual(expect.any(String));
  const windowContinuation = await fixtureProvider.window({documentId: document.id, versionId: version.id, cursor: emptyWindow.nextCursor, limit: 2});
  expect(windowContinuation.units).toHaveLength(2);

  const emptySearch = await fixtureProvider.search({documentId: document.id, versionId: version.id, query: 'synthetic', scope: 'version', limit: 1});
  expect(emptySearch.items).toHaveLength(0);
  expect(emptySearch.nextCursor).toEqual(expect.any(String));
  const searchContinuation = await fixtureProvider.search({documentId: document.id, versionId: version.id, query: 'synthetic', scope: 'version', cursor: emptySearch.nextCursor, limit: 1});
  expect(searchContinuation.items).toHaveLength(1);

  const emptyStructure = await fixtureProvider.structure({documentId: document.id, versionId: version.id, limit: 1});
  expect(emptyStructure.items).toHaveLength(0);
  expect(emptyStructure.nextCursor).toEqual(expect.any(String));
  const structureContinuation = await fixtureProvider.structure({documentId: document.id, versionId: version.id, cursor: emptyStructure.nextCursor, limit: 1});
  expect(structureContinuation.items).toHaveLength(1);
});

test('UTF-8 byte bounds reject multibyte units that fit the character bound', async () => {
  const version = {id: 'version-1', language: 'ru', role: 'original', revision: 'a'.repeat(64), unitCount: 1, source: VALID_SOURCE};
  const provider = createCorpusProvider({provider: {
    catalog: async () => ({items: [], nextCursor: null, total: null}),
    document: async () => ({id: 'work-1', title: 'Work', versions: [version]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [{
      id: 'unit-1', ordinal: 1, kind: 'paragraph', text: 'Ж'.repeat(32_769), language: 'ru', reference: 'unit-1',
    }], previousCursor: null, nextCursor: null, total: null}),
    search: async () => ({items: [], nextCursor: null, total: null}),
    resolve: async () => ({status: 'unavailable'}),
  }});
  await expect(provider.window({documentId: 'work-1', versionId: 'version-1', limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});
});

test('bound providers normalize omitted counts and fixture variants exercise incomplete packets', async () => {
  const core = {
    catalog: async () => ({items: [{id: 'work-1', title: 'Work'}], nextCursor: null}),
    document: async () => ({id: 'work-1', title: 'Work', versions: [{
      id: 'version-1', language: 'de', role: 'original', status: 'metadata-only',
    }]}),
    window: async () => ({documentId: 'work-1', versionId: 'version-1', revision: 'a'.repeat(64), units: [], nextCursor: null}),
    search: async () => ({items: [], nextCursor: null}),
    resolve: async () => ({status: 'unavailable'}),
  };
  const bound = createCorpusProvider({provider: core});
  await expect(bound.catalog({limit: 1})).resolves.toMatchObject({total: null, items: [{unitCount: null}]});
  await expect(bound.document({documentId: 'work-1'})).resolves.toMatchObject({unitCount: null, versions: [{revision: null, source: null, unitCount: null}]});

  const fixtureProvider = createFixtureProvider({documentCount: 1, unitCount: 4});
  const fixtureBound = createCorpusProvider({provider: fixtureProvider});
  const document = await fixtureBound.document({documentId: 'tos.work.fixture-000001'});
  fixtureProvider.controls.setResponseVariant('catalog', 'omit-total');
  await expect(fixtureBound.catalog({limit: 1})).resolves.toMatchObject({total: null});
  fixtureProvider.controls.setResponseVariant('catalog', 'missing-next-cursor');
  await expect(fixtureBound.catalog({limit: 1})).rejects.toMatchObject({code: 'invalid_response'});
  fixtureProvider.controls.setResponseVariant('window', 'wrong-identity');
  await expect(fixtureBound.window({documentId: document.id, versionId: document.versions[0].id, limit: 1}))
    .rejects.toMatchObject({code: 'invalid_response'});
});
