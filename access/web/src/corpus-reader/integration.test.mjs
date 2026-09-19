import {expect, test} from 'vitest';
import {createReference, referenceKey} from './model.mjs';
import {createFixtureProvider} from './fixture-provider.mjs';
import {createCorpusNotebook, createMemoryCorpusNotebookState, readingSlot} from './notebook.mjs';
import {createCorpusReaderModel} from './view-model.mjs';

const codePointLength = value => Array.from(value).length;
const windowKey = (documentId, versionId) => `${documentId}\u0000${versionId}`;

async function makeNotebook(memoryStore = createMemoryCorpusNotebookState()) {
  const notebook = createCorpusNotebook({adapter: 'memory', memoryStore});
  await notebook.getPreference('integration-ready');
  return {memoryStore, notebook};
}

test('reader keeps each provider window bounded across three documents and versions', async () => {
  const provider = createFixtureProvider({
    documentCount: 3,
    unitCount: 300,
    languages: ['ru', 'en', 'de'],
    originalLanguage: 'ru',
  });
  const {notebook} = await makeNotebook();
  const model = createCorpusReaderModel({provider, notebook});
  await model.ready();
  const catalog = await model.catalog({limit: 24});

  for (const summary of catalog.items) {
    const manifest = await provider.document({documentId: summary.id});
    for (const version of manifest.versions) {
      const opened = await model.open({documentId: manifest.id, versionId: version.id});
      expect(opened.units.length).toBeLessThanOrEqual(model.limits.windowUnits);
      const snapshot = model.snapshot();
      expect(snapshot.document.id).toBe(manifest.id);
      expect(snapshot.activeVersionId).toBe(version.id);
      expect(Object.keys(snapshot.windows).length).toBeLessThanOrEqual(2);
      expect(snapshot.windows[windowKey(manifest.id, version.id)]?.units.length).toBeLessThanOrEqual(model.limits.windowUnits);
    }
  }
  expect(provider.metrics().maxPageUnits).toBeLessThanOrEqual(model.limits.windowUnits);
  expect(provider.metrics().logical.documentCount).toBe(3);
  await notebook.close();
});

test('strict note quotes survive targeted CAS and reading positions reload per version', async () => {
  const provider = createFixtureProvider({
    documentCount: 1,
    unitCount: 120,
    languages: ['ru', 'en', 'de'],
    originalLanguage: 'ru',
  });
  const {memoryStore, notebook: notebookA} = await makeNotebook();
  const notebookB = createCorpusNotebook({adapter: 'memory', memoryStore});
  const modelA = createCorpusReaderModel({provider, notebook: notebookA});
  await modelA.ready();

  const russianWindow = await modelA.open();
  const document = modelA.snapshot().document;
  const russian = document.versions.find(version => version.language === 'ru');
  const russianUnit = russianWindow.units[3];
  const russianQuote = russianUnit.text.slice(0, 16);
  const russianReference = modelA.anchorFor(russianUnit, russian, {
    start: 0,
    end: codePointLength(russianQuote),
  });
  expect(russianReference).toBeTruthy();
  const created = await modelA.saveNote(russianReference, 'Исследовательская заметка', russianQuote);
  expect(created.item).toMatchObject({kind: 'note', quote: russianQuote, text: 'Исследовательская заметка'});
  expect(created.item.reference).toEqual(russianReference);
  expect(created.item.referenceKey).toBe(referenceKey(russianReference));

  const storedNotes = await notebookA.listNotes({documentId: document.id});
  expect(storedNotes.items).toHaveLength(1);
  expect(storedNotes.items[0]).toMatchObject({quote: russianQuote, reference: russianReference});
  const external = await notebookB.putNote({
    id: created.item.id,
    reference: russianReference,
    kind: 'note',
    quote: russianQuote,
    text: 'Внешнее обновление',
    expectedRecordRevision: created.item.revision,
  });
  expect(external.item.revision).toBeGreaterThan(created.item.revision);
  await expect(modelA.saveNote(russianReference, 'Устаревшее обновление', russianQuote))
    .rejects.toMatchObject({code: 'conflict', expectedRecordRevision: created.item.revision});

  await modelA.savePosition(russianReference, 0.25, 'primary');
  const english = document.versions.find(version => version.language === 'en');
  const englishWindow = await modelA.loadWindow({versionId: english.id});
  const englishUnit = englishWindow.units[5];
  const englishReference = modelA.anchorFor(englishUnit, english, {start: 0, end: 8});
  await modelA.savePosition(englishReference, 0.5, 'secondary');
  const german = document.versions.find(version => version.language === 'de');
  const germanWindow = await modelA.loadWindow({versionId: german.id});
  const germanUnit = germanWindow.units[6];
  const germanReference = modelA.anchorFor(germanUnit, german, {start: 0, end: 8});
  await modelA.savePosition(germanReference, 0.75, 'primary');

  const notebookReload = createCorpusNotebook({adapter: 'memory', memoryStore});
  const modelReload = createCorpusReaderModel({provider, notebook: notebookReload});
  await modelReload.ready();
  const resumedRussian = await modelReload.open({documentId: document.id, versionId: russian.id});
  const resumedEnglish = await modelReload.open({documentId: document.id, versionId: english.id});
  const resumedGerman = await modelReload.open({documentId: document.id, versionId: german.id});
  expect(resumedRussian.units.some(unit => unit.id === russianUnit.id)).toBe(true);
  expect(resumedEnglish.units.some(unit => unit.id === englishUnit.id)).toBe(true);
  expect(resumedGerman.units.some(unit => unit.id === germanUnit.id)).toBe(true);
  expect(await notebookReload.loadReading(readingSlot(russianReference))).toMatchObject({reference: russianReference});
  expect(await notebookReload.loadReading(readingSlot(englishReference))).toMatchObject({reference: englishReference});
  expect(await notebookReload.loadReading(readingSlot(germanReference))).toMatchObject({reference: germanReference});
  await notebookA.close();
  await notebookB.close();
  await notebookReload.close();
});

test('stale addresses remain visible while the refreshed manifest exposes its current digest', async () => {
  const provider = createFixtureProvider({documentCount: 1, unitCount: 80, languages: ['ru'], originalLanguage: 'ru'});
  const {notebook} = await makeNotebook();
  const model = createCorpusReaderModel({provider, notebook});
  await model.ready();
  const initialWindow = await model.open();
  const initialDocument = model.snapshot().document;
  const initialVersion = initialDocument.versions[0];
  const selectedUnit = initialWindow.units[4];
  const oldQuote = selectedUnit.text.slice(0, 12);
  const oldReference = model.anchorFor(selectedUnit, initialVersion, {start: 0, end: codePointLength(oldQuote)});
  await model.savePosition(oldReference, 0.4, 'primary');
  const oldDigest = oldReference.target.textLayerSha256;

  provider.controls.bumpRevision({documentId: initialDocument.id, versionId: initialVersion.id});
  const currentManifest = await provider.document({documentId: initialDocument.id});
  const currentVersion = currentManifest.versions[0];
  expect(currentVersion.source.textLayerSha256).not.toBe(oldDigest);

  const stale = await model.open({documentId: initialDocument.id, versionId: initialVersion.id, reference: oldReference});
  expect(stale).toBeNull();
  const snapshot = model.snapshot();
  expect(snapshot.referenceStatus).toBe('changed');
  expect(snapshot.document.versions[0]).toMatchObject({
    revision: currentVersion.revision,
    source: {textLayerSha256: currentVersion.source.textLayerSha256},
  });
  expect(snapshot.document.versions[0].source.textLayerSha256).not.toBe(oldDigest);
  expect(snapshot.windows[windowKey(initialDocument.id, initialVersion.id)].units[4].id).toBe(selectedUnit.id);
  expect(await notebook.loadReading(readingSlot(oldReference))).toMatchObject({reference: oldReference});
  const retained=snapshot.windows[windowKey(initialDocument.id,initialVersion.id)].units[4];
  expect(model.anchorFor(retained).target.textLayerSha256).toBe(oldDigest);
  await notebook.close();
});

test('latest open wins when a slow fixture window for A resolves after fast B', async () => {
  let slowWindowStarted;
  const slowWindow = new Promise(resolve => { slowWindowStarted = resolve; });
  const documentA = 'tos.work.fixture-000001';
  const documentB = 'tos.work.fixture-000002';
  const provider = createFixtureProvider({
    documentCount: 2,
    unitCount: 80,
    languages: ['ru'],
    originalLanguage: 'ru',
    latencyMs: (operation, args) => {
      if (operation === 'window' && args.documentId === documentA) {
        slowWindowStarted();
        return 40;
      }
      return 0;
    },
  });
  const {notebook} = await makeNotebook();
  const model = createCorpusReaderModel({provider, notebook});
  await model.ready();
  const [manifestA, manifestB] = await Promise.all([
    provider.document({documentId: documentA}),
    provider.document({documentId: documentB}),
  ]);

  const openingA = model.open({documentId: manifestA.id, versionId: manifestA.versions[0].id});
  await slowWindow;
  const openingB = model.open({documentId: manifestB.id, versionId: manifestB.versions[0].id});
  const [resultA, resultB] = await Promise.all([openingA, openingB]);
  expect(resultA).toBeNull();
  expect(resultB.units[0].id).toContain('tos.text-unit.sid-');
  expect(model.snapshot().document.id).toBe(manifestB.id);
  expect(model.snapshot().windows[windowKey(manifestA.id, manifestA.versions[0].id)]).toBeUndefined();
  expect(model.snapshot().windows[windowKey(manifestB.id, manifestB.versions[0].id)].units[0].id).toBe(resultB.units[0].id);
  await notebook.close();
});


test('opening page pins the manifest even when the source changes between metadata and text',async()=>{
  const source=createFixtureProvider({documentCount:1,unitCount:20,languages:['de']});
  const manifest=await source.document({documentId:'tos.work.fixture-000001'}),version=manifest.versions[0];
  const first=await source.window({documentId:manifest.id,versionId:version.id,revision:version.revision,limit:1});
  const reference=createReference({source:version.source,versionId:version.id,unitId:first.units[0].id});
  expect((await source.window({documentId:manifest.id,versionId:version.id,reference,limit:1})).units[0].id).toBe(reference.unitId);
  let bumped=false;
  const provider={...source,window:args=>{if(!bumped){bumped=true;source.controls.bumpRevision();}return source.window(args);}};
  const {notebook}=await makeNotebook();const model=createCorpusReaderModel({provider,notebook});await model.ready();
  expect(await model.open({documentId:manifest.id,versionId:version.id})).toBeNull();
  expect(model.snapshot().referenceStatus).toBe('changed');
  expect(Object.keys(model.snapshot().windows)).toHaveLength(0);
  const page=await model.loadWindow({acceptCurrent:true});
  const active=model.snapshot().document.versions[0];
  expect(page.revision).toBe(active.source.textLayerSha256);
  expect(model.anchorFor(page.units[0],active).target.textLayerSha256).not.toBe(version.revision);
  await notebook.close();
});

test('parallel versions recover stale reading positions independently', async () => {
  const provider = createFixtureProvider({documentCount: 1, unitCount: 80, languages: ['ru', 'de'], originalLanguage: 'ru'});
  const {notebook} = await makeNotebook();
  const model = createCorpusReaderModel({provider, notebook});
  await model.ready();
  const first = await model.open();
  const document = model.snapshot().document;
  const primary = document.versions.find(version => version.language === 'ru');
  const secondary = document.versions.find(version => version.language === 'de');
  await model.savePosition(model.anchorFor(first.units[3], primary), 0, 'primary');
  const other = await model.loadWindow({versionId: secondary.id});
  const oldReference = model.anchorFor(other.units[4], secondary);
  await model.savePosition(oldReference, 0, 'secondary');
  provider.controls.bumpRevision();

  expect(await model.open({documentId: document.id, versionId: primary.id})).toBeNull();
  await model.loadWindow({versionId: primary.id, acceptCurrent: true});
  expect(await model.loadWindow({versionId: secondary.id})).toBeNull();
  expect(model.snapshot()).toMatchObject({
    referenceStatus: 'exact',
    referenceStates: {
      [windowKey(document.id, primary.id)]: 'exact',
      [windowKey(document.id, secondary.id)]: 'changed',
    },
  });
  const recovered = await model.loadWindow({versionId: secondary.id, acceptCurrent: true});
  expect(model.snapshot().referenceStates[windowKey(document.id, secondary.id)]).toBe('exact');
  expect(model.anchorFor(recovered.units[0]).target.textLayerSha256).not.toBe(oldReference.target.textLayerSha256);
  expect(await notebook.loadReading(readingSlot(oldReference))).toMatchObject({reference: oldReference});
  expect(Object.keys(model.snapshot().windows)).toHaveLength(2);
  await notebook.close();
});
