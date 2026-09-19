import {createFixtureProvider} from '../src/corpus-reader/fixture-provider.mjs';
import {createReference} from '../src/corpus-reader/model.mjs';
import {createCorpusNotebook,readingSlot} from '../src/corpus-reader/notebook.mjs';

const DEFAULT_TIMEOUT = 3000;

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function errorShape(error) {
  return {
    code: typeof error?.code === 'string' ? error.code : (typeof error?.name === 'string' ? error.name : 'error'),
    message: String(error?.message || error || 'Unknown error').slice(0, 240),
  };
}

function requireCheck(condition, message, details = {}) {
  if (!condition) {
    const error = new Error(message);
    Object.assign(error, details);
    throw error;
  }
}

function uniqueDbName() {
  const uuid = globalThis.crypto?.randomUUID?.();
  const suffix = uuid || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `tos-corpus-reader-smoke-${suffix}`;
}

function referenceFor(version, unit, end = 12) {
  const length = [...String(unit.text || '')].length;
  return createReference({
    source: version.source,
    versionId: version.id,
    unitId: unit.id,
    start: 0,
    end: Math.min(end, length),
  });
}

function stateShape(packet) {
  return JSON.stringify({
    revision: packet.revision,
    notes: packet.notes,
    readings: packet.readings,
    preferences: packet.preferences,
  });
}

function fakeOpenFailure(name, message) {
  return {
    open() {
      throw Object.assign(new Error(message), {name});
    },
  };
}

function deleteDatabase(indexedDB, dbName, timeoutMs = DEFAULT_TIMEOUT) {
  return new Promise(resolve => {
    let request;
    let timer;
    let settled = false;
    let blocked = false;
    const finish = result => {
      if (settled) return;
      settled = true;
      if (timer !== undefined) clearTimeout(timer);
      resolve(result);
    };
    try {
      request = indexedDB.deleteDatabase(dbName);
    } catch (error) {
      finish({ok: false, ...errorShape(error)});
      return;
    }
    timer = setTimeout(() => finish({ok: false, blocked, code: 'cleanup-timeout', message: 'IndexedDB deletion timed out.'}), timeoutMs);
    request.onblocked = () => { blocked = true; };
    request.onsuccess = () => finish({ok: true, blocked});
    request.onerror = () => finish({ok: false, blocked, ...errorShape(request.error || new Error('IndexedDB deletion failed'))});
  });
}

async function closeAll(handles) {
  const errors = [];
  for (const handle of [...handles].reverse()) {
    if (!handle || typeof handle.close !== 'function') continue;
    try {
      await handle.close();
    } catch (error) {
      errors.push(errorShape(error));
    }
  }
  return errors;
}

/**
 * Run a browser-only IndexedDB regression against synthetic corpus material.
 * The generated database name is unique and is removed in the finally path.
 */
export async function checkCorpusIndexedDB() {
  const indexedDB = globalThis.window?.indexedDB || globalThis.indexedDB;
  const dbName = uniqueDbName();
  const handles = [];
  const checks = {};
  let failure = null;
  let cleanup = {ok: true, skipped: true};
  let provider;

  try {
    requireCheck(indexedDB && typeof indexedDB.open === 'function' && typeof indexedDB.deleteDatabase === 'function', 'Browser IndexedDB is unavailable.', {code: 'indexeddb-unavailable'});
    checks.environment = {ok: true};

    provider = createFixtureProvider({documentCount: 3, unitCount: 12, languages: ['ru', 'en', 'grc']});
    const catalog = await provider.catalog({limit: 2});
    requireCheck(catalog.items.length === 2 && catalog.total === 3, 'Fixture catalog did not return the expected bounded page.');
    const documentId = catalog.items[0].id;
    const document = await provider.document({documentId});
    const versions = document.versions;
    requireCheck(versions.length >= 2, 'Fixture document did not expose two versions.');
    const [primaryVersion, secondaryVersion, tertiaryVersion] = versions;
    const primaryWindow = await provider.window({documentId, versionId: primaryVersion.id, limit: 4});
    const secondaryWindow = await provider.window({documentId, versionId: secondaryVersion.id, limit: 4});
    const tertiaryWindow = await provider.window({documentId, versionId: tertiaryVersion.id, limit: 4});
    requireCheck(primaryWindow.units.length >= 3 && secondaryWindow.units.length >= 1 && tertiaryWindow.units.length >= 1, 'Fixture windows did not return enough units.');
    const primaryReference = referenceFor(primaryVersion, primaryWindow.units[0]);
    const secondReference = referenceFor(primaryVersion, primaryWindow.units[1]);
    const thirdReference = referenceFor(primaryVersion, primaryWindow.units[2]);
    const secondaryReference = referenceFor(secondaryVersion, secondaryWindow.units[0]);
    const tertiaryReference = referenceFor(tertiaryVersion, tertiaryWindow.units[0]);
    const tertiarySlot = readingSlot(tertiaryReference);
    checks.fixture = {ok: true, documentId, versions: versions.slice(0, 3).map(version => version.id), tertiarySlot};

    const open = () => {
      const notebook = createCorpusNotebook({indexedDB, dbName});
      handles.push(notebook);
      return notebook;
    };
    const first = open();
    await first.putNote({reference: primaryReference, kind: 'note', quote: 'Fixture quote', text: 'IndexedDB smoke note'});
    await first.putNote({reference: secondReference, kind: 'bookmark'});
    await first.putNote({reference: thirdReference, kind: 'bookmark'});
    await first.saveReading({slot: 'primary', documentId, versionId: primaryVersion.id, reference: primaryReference, offset: 7});
    await first.saveReading({slot: 'secondary', documentId, versionId: secondaryVersion.id, reference: secondaryReference, offset: 11});
    await first.saveReading({slot: tertiarySlot, documentId, versionId: tertiaryVersion.id, reference: tertiaryReference, offset: 13});
    const persistedPacket = await first.exportPacket();
    const persistedNote = persistedPacket.notes.find(item => item.kind === 'note');
    requireCheck(persistedNote && persistedPacket.notes.length === 3, 'Initial IndexedDB records were not written.');
    const initialRevision = persistedPacket.revision;
    await first.close();

    const reopened = open();
    const restoredNotes = await reopened.listNotes({documentId, limit: 20});
    const restoredPrimary = await reopened.loadReading('primary');
    const restoredSecondary = await reopened.loadReading('secondary');
    const restoredTertiary = await reopened.loadReading(tertiarySlot);
    requireCheck(restoredNotes.items.length === 3, 'IndexedDB records did not survive reopening.');
    requireCheck(restoredPrimary?.versionId === primaryVersion.id && restoredSecondary?.versionId === secondaryVersion.id &&
      restoredTertiary?.versionId === tertiaryVersion.id && restoredTertiary?.offset === 13,
      'Reading positions did not survive reopening per version.');
    checks.persistence = {ok: true, revision: initialRevision, notes: restoredNotes.items.length};
    const basePacket = await reopened.exportPacket();
    await reopened.close();

    const left = open();
    const right = open();
    const casRevision = basePacket.revision;
    const casResults = await Promise.allSettled([
      left.putNote({id: persistedNote.id, reference: primaryReference, kind: 'note', text: 'CAS left', expectedRevision: casRevision}),
      right.putNote({id: persistedNote.id, reference: primaryReference, kind: 'note', text: 'CAS right', expectedRevision: casRevision}),
    ]);
    const winners = casResults.filter(result => result.status === 'fulfilled');
    const conflicts = casResults.filter(result => result.status === 'rejected' && result.reason?.code === 'conflict');
    requireCheck(winners.length === 1 && conflicts.length === 1, 'Concurrent stale revision did not produce exactly one CAS conflict.');
    checks.cas = {ok: true, revision: winners[0].value.revision};
    const oldNote=winners[0].value.item;
    await left.saveReading({slot:tertiarySlot,documentId,versionId:tertiaryVersion.id,reference:tertiaryReference,offset:14});
    const edited=await left.putNote({id:oldNote.id,reference:primaryReference,kind:'note',text:'Record CAS after reading',expectedRecordRevision:oldNote.revision});
    let recordConflict=false,deleteConflict=false;
    try{await right.putNote({id:oldNote.id,reference:primaryReference,kind:'note',text:'Stale record overwrite',expectedRecordRevision:oldNote.revision});}catch(error){recordConflict=error.code==='conflict';}
    try{await right.deleteNote(oldNote.id,undefined,oldNote.revision);}catch(error){deleteConflict=error.code==='conflict';}
    requireCheck(recordConflict&&deleteConflict&&edited.item.text==='Record CAS after reading','Targeted note CAS failed across browser database connections.');
    checks.recordCas={ok:true,revision:edited.revision,unrelatedReadingAllowed:true,staleEditRejected:recordConflict,staleDeleteRejected:deleteConflict};

    const paged = [];
    let cursor = null;
    do {
      const page = await left.listNotes({documentId, limit: 1, cursor});
      paged.push(...page.items);
      cursor = page.nextCursor;
    } while (cursor);
    requireCheck(paged.length === 3 && new Set(paged.map(item => item.id)).size === 3, 'IndexedDB note pagination lost or duplicated records.');
    checks.pagination = {ok: true, pages: paged.length};

    const otherDocument=await provider.document({documentId:catalog.items[1].id});
    const otherVersion=otherDocument.versions[0];
    const otherPage=await provider.window({documentId:otherDocument.id,versionId:otherVersion.id,limit:1});
    await left.putNote({reference:referenceFor(otherVersion,otherPage.units[0]),kind:'bookmark'});
    const globalPaged = [];
    let globalCursor = null;
    let globalPages = 0;
    do {
      const page = await left.listNotes({documentId: null, limit: 2, cursor: globalCursor});
      globalPaged.push(...page.items);
      globalCursor = page.nextCursor;
      globalPages += 1;
    } while (globalCursor);
    requireCheck(globalPaged.length === 4 && new Set(globalPaged.map(item => item.id)).size === 4 &&
      new Set(globalPaged.map(item => item.documentId)).size === 2,
      'IndexedDB global note pagination lost, duplicated, or rejected records from another document.');
    checks.globalPagination = {ok: true, pages: globalPages, notes: globalPaged.length};

    const beforeImport = await left.exportPacket();
    const importedNote = JSON.parse(JSON.stringify(beforeImport.notes[0]));
    importedNote.id = 'smoke-import-note';
    importedNote.text = 'Imported via merge';
    const additivePacket = {
      schema: beforeImport.schema,
      version: beforeImport.version,
      revision: 0,
      notes: [importedNote],
      readings: [],
      preferences: [{key: 'smoke-import', value: true}],
    };
    const additive = await left.importPacket(additivePacket, {expectedRevision: beforeImport.revision});
    const afterAdditive = await left.exportPacket();
    requireCheck(additive.revision === beforeImport.revision + 1 && afterAdditive.notes.length === beforeImport.notes.length + 1 &&
      await left.getPreference('smoke-import') === true, 'Valid import did not merge without clearing existing notes.');
    const noOp = await left.importPacket(additivePacket);
    requireCheck(noOp.revision === additive.revision, 'An exact import no-op advanced the local revision.');

    const conflictPacket = JSON.parse(JSON.stringify(afterAdditive));
    conflictPacket.notes[0].text = 'conflicting imported content';
    let conflictCode = null;
    try {
      await left.importPacket(conflictPacket);
    } catch (error) {
      conflictCode = error?.code || error?.name || null;
    }
    const afterConflict = await left.exportPacket();
    requireCheck(conflictCode === 'conflict' && stateShape(afterConflict) === stateShape(afterAdditive),
      'Conflicting note import was not rejected atomically.');

    await left.setPreference('smoke-bump', true);
    let staleCode = null;
    try {
      await left.importPacket(afterConflict, {expectedRevision: afterConflict.revision});
    } catch (error) {
      staleCode = error?.code || error?.name || null;
    }
    requireCheck(staleCode === 'conflict' && await left.getPreference('smoke-bump') === true,
      'Stale import expectedRevision did not stop the merge.');

    const beforeInvalidImport = await left.exportPacket();
    const invalidPacket = JSON.parse(JSON.stringify(beforeInvalidImport));
    invalidPacket.notes[0].reference.target.fileSha256 = 'invalid-digest';
    let invalidCode = null;
    try {
      await left.importPacket(invalidPacket);
    } catch (error) {
      invalidCode = error?.code || error?.name || null;
    }
    const afterInvalidImport = await left.exportPacket();
    requireCheck(invalidCode === 'invalid-packet' && stateShape(afterInvalidImport) === stateShape(beforeInvalidImport),
      'Invalid import was not rejected without changing the notebook.');
    checks.import = {ok: true, additiveRevision: additive.revision, noOpRevision: noOp.revision, conflictCode, staleCode, invalidCode};

    const unavailable = createCorpusNotebook({indexedDB: fakeOpenFailure('InvalidStateError', 'synthetic unavailable')});
    handles.push(unavailable);
    await unavailable.getPreference('smoke');
    const unavailableStatus = unavailable.status();
    const quota = createCorpusNotebook({indexedDB: fakeOpenFailure('QuotaExceededError', 'synthetic quota')});
    handles.push(quota);
    await quota.getPreference('smoke');
    const quotaStatus = quota.status();
    requireCheck(unavailableStatus.adapter === 'memory' && unavailableStatus.persistent === false && unavailableStatus.warning === 'storage-unavailable',
      'Unavailable IndexedDB did not expose an explicit non-persistent warning.');
    requireCheck(quotaStatus.adapter === 'memory' && quotaStatus.persistent === false && quotaStatus.warning === 'quota',
      'Quota failure did not expose an explicit non-persistent warning.');
    checks.fallback = {ok: true, unavailable: unavailableStatus.warning, quota: quotaStatus.warning};
  } catch (error) {
    failure = errorShape(error);
    checks.failure = {ok: false, ...failure};
  } finally {
    const closeErrors = await closeAll(handles);
    if (closeErrors.length) checks.close = {ok: false, errors: closeErrors};
    if (indexedDB && typeof indexedDB.deleteDatabase === 'function') {
      cleanup = await deleteDatabase(indexedDB, dbName);
    }
    if (closeErrors.length || !cleanup.ok) {
      checks.cleanup = {ok: false, closeErrors, deletion: cleanup};
    } else {
      checks.cleanup = {ok: true, deletion: cleanup};
    }
  }

  const ok = !failure && Object.values(checks).every(check => check?.ok !== false) && cleanup.ok;
  return {
    ok,
    dbName,
    checks,
    ...(failure ? {error: failure} : {}),
    cleanup,
  };
}
