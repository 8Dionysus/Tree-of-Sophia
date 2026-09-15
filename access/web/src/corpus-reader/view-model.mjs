import {createReference, isReference, referenceKey as canonicalReferenceKey, validateReference} from './model.mjs';
import {readingSlot} from './notebook.mjs';
export {isReference};

/*
 * The corpus reader deliberately keeps its data seam small.  The web view
 * owns presentation and transient reading state; the provider owns source
 * identity, text preparation, rights, and search semantics.
 */

export const READER_LIMITS = Object.freeze({
  catalogPage: 24,
  windowUnits: 40,
  query: 160,
  searchResults: 100,
  quote: 1200,
  note: 4000,
  notebookPage: 100,
});

// Keep the view model useful for a large corpus without turning a long
// browsing session into an unbounded in-memory cache.  The provider page
// limits remain the request boundary; these are client-side retention caps.
const CATALOG_CACHE_LIMIT = READER_LIMITS.catalogPage * 4;
const SEARCH_CACHE_LIMIT = READER_LIMITS.searchResults * 3;
const NOTEBOOK_CACHE_LIMIT = 400;

const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const list = value => Array.isArray(value) ? value : [];
const first = (...values) => values.find(value => value !== undefined && value !== null);
const text = (value, fallback = '') => typeof value === 'string' ? value : fallback;
const boundedText = (value, max, fallback = '') => text(value, fallback).slice(0, max);
const count = value => Number.isSafeInteger(value) && value >= 0 ? value : null;

const UNAVAILABLE_STATUSES = new Set([
  'unavailable', 'metadata_only', 'metadata-only', 'metadata', 'restricted',
  'public_metadata_only', 'public-metadata-only', 'local_only', 'local-only',
  'link_only', 'link-only', 'missing', 'expired', 'text_unavailable',
  'text-unavailable', 'withheld', 'unknown', 'pending', 'not_available',
  'not-available', 'not_indexed', 'not-indexed', 'rights_unknown', 'rights-unknown',
]);

function availabilityStatus(value) {
  if (!object(value)) return String(value ?? '').trim().toLowerCase();
  return String(first(value.status, value.availability, value.textAvailability, value.text_availability) ?? '')
    .trim().toLowerCase();
}

export function isTextAvailable(value = {}) {
  if (!object(value)) return false;
  if (value.available === false || value.textAvailable === false || value.text_available === false) return false;
  const status = availabilityStatus(value);
  const visibility = String(value.visibility ?? '').trim().toLowerCase();
  return !UNAVAILABLE_STATUSES.has(status)
    && !['restricted', 'public_metadata_only', 'local_only', 'unknown'].includes(visibility);
}

export function availabilityLabel(value = {}) {
  const status = availabilityStatus(value);
  const normalizedStatus = status.replace(/-/g, '_');
  const visibility = String(value.visibility ?? '').trim().toLowerCase();
  if (status && normalizedStatus !== 'available') return status;
  if (['restricted', 'public_metadata_only', 'local_only', 'unknown'].includes(visibility)) {
    return visibility === 'public_metadata_only' ? 'metadata_only' : visibility;
  }
  if (value.textAvailable === false || value.text_available === false) return 'metadata_only';
  if (value.available === false) return 'unavailable';
  return status || 'available';
}

function withoutPayload(value, keys = []) {
  if (!object(value)) return {};
  const excluded = new Set(keys);
  const result = {};
  for (const [key, item] of Object.entries(value)) if (!excluded.has(key)) result[key] = item;
  return clone(result);
}

function localize(value, locale = 'ru') {
  if (typeof value === 'string') return value;
  if (!object(value)) return '';
  const wanted = String(locale || 'ru').toLowerCase();
  const base = wanted.split('-')[0];
  return text(value[wanted], text(value[base], text(value.ru, text(value.en, Object.values(value).find(item => typeof item === 'string') || ''))));
}

function clone(value) {
  if (value === undefined) return undefined;
  if (typeof structuredClone === 'function') return structuredClone(value);
  return JSON.parse(JSON.stringify(value));
}

function providerMethod(provider, name) {
  if (!provider) return null;
  const aliases = {
    catalog: ['catalog', 'listDocuments', 'documents'],
    document: ['document', 'getDocument', 'readDocument'],
    window: ['window', 'textWindow', 'readWindow', 'segments'],
    resolve: ['resolve', 'resolveReference', 'resolveAnchor'],
    search: ['search', 'searchText'],
  };
  return aliases[name].map(key => provider[key]).find(value => typeof value === 'function') || null;
}

async function callProvider(provider, name, args) {
  const method = providerMethod(provider, name);
  if (!method) throw new Error(`provider-${name}-unavailable`);
  return await method.call(provider, args);
}

function thisSearchScope(provider, scope) {
  const capabilities = object(provider?.capabilities) ? provider.capabilities : {};
  const advertised = list(capabilities.searchScopes).map(value => String(value).toLowerCase());
  if (capabilities.search === false) return false;
  if (scope === 'corpus') return capabilities.corpusSearch === true || advertised.includes('corpus');
  if (scope === 'version') {
    // `document` is the provider-side alias defined by the v1 contract.
    return advertised.length === 0 || advertised.includes('version') || advertised.includes('document');
  }
  return false;
}

function valueRevision(value, primary, fallback = null) {
  return first(value?.[primary], value?.[primary.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`)], fallback);
}

export function versionIdentity(version = {}, document = {}) {
  return {
    documentId: text(first(document.id, document.documentId), ''),
    versionId: text(first(version.id, version.versionId, version.version_id, version.code, version.language), ''),
    language: text(first(version.language, version.lang, version.code), ''),
    sourceRevision: first(valueRevision(version, 'sourceRevision'), valueRevision(document, 'sourceRevision'), null),
    contentRevision: first(valueRevision(version, 'contentRevision'), valueRevision(version, 'revision'), valueRevision(document, 'contentRevision'), null),
  };
}

function rawReference(value) {
  if (!object(value)) return null;
  const nested = first(value.reference, value.anchor, value.sourceReference, value.source_reference);
  return object(nested) ? nested : value;
}

export function exactReference(value = {}, identity = {}) {
  return isReference(value) ? validateReference(value) : null;
}

export function referenceKey(value) {
  return isReference(value) ? canonicalReferenceKey(value) : '';
}

export function sameReference(left, right) {
  const leftKey = referenceKey(left); return Boolean(leftKey) && leftKey === referenceKey(right);
}

export function unitReference(unit, version = {}, document = {}, {start = 0, end = 0} = {}) {
  const existing = unit?.reference;
  if (isReference(existing)) {
    const checked = validateReference(existing);
    if (start === checked.selector.start && end === checked.selector.end) return checked;
    return validateReference({...checked, selector: {...checked.selector, start, end}});
  }
  if (object(version.source) && unit?.unitId) {
    try {
      return createReference({source: version.source, versionId: version.id || version.versionId, unitId: unit.unitId, start, end});
    } catch {
      // A provider response without a complete source identity can still be
      // rendered, but it cannot become a durable notebook anchor.
    }
  }
  return null;
}

export function normalizeVersion(raw = {}, document = {}) {
  const identity = versionIdentity(raw, document);
  const source = object(raw) ? raw : {};
  const payloadUnits = list(first(source.units, source.segments, source.paragraphs));
  const metadata = withoutPayload(source, ['units', 'segments', 'paragraphs', 'content']);
  const id = identity.versionId || text(first(raw.language, raw.lang), 'default');
  const status = text(first(raw.status, raw.availability, raw.textAvailability, raw.text_availability, raw.available === false ? 'unavailable' : null), 'available');
  const available = isTextAvailable({...raw, status});
  return {
    ...metadata,
    id,
    versionId: id,
    language: identity.language || text(raw.lang, ''),
    title: first(raw.title, document.title, ''),
    locator: first(raw.locator, raw.location, document.locator, ''),
    edition: first(raw.edition, raw.editionLabel, ''),
    translator: first(raw.translator, raw.translatorName, ''),
    role: first(raw.role, ''),
    source: object(raw.source) ? clone(raw.source) : null,
    revision: first(raw.revision, identity.contentRevision, null),
    status,
    availability: availabilityLabel({...raw, status}),
    available,
    sourceRevision: identity.sourceRevision,
    contentRevision: identity.contentRevision,
    unitCount: count(first(raw.unitCount, raw.unit_count, raw.count)),
    units: payloadUnits.slice(0, READER_LIMITS.windowUnits),
    unitsTruncated: payloadUnits.length > READER_LIMITS.windowUnits,
  };
}

export function normalizeDocument(raw = {}) {
  const source = object(raw) ? raw : {};
  const document = withoutPayload(source, ['versions', 'editions', 'representations']);
  const id = text(first(document.id, document.documentId, document.workId, document.work_id), '');
  const versionsValue = first(source.versions, source.editions, source.representations, []);
  const versions = Array.isArray(versionsValue)
    ? versionsValue.map(item => normalizeVersion(item, {...document, id}))
    : Object.entries(object(versionsValue) ? versionsValue : {}).map(([key, item]) => normalizeVersion({...item, id: item?.id ?? key, language: item?.language ?? key}, {...document, id}));
  return {
    ...document,
    id,
    documentId: id,
    title: first(document.title, document.name, id),
    author: first(document.author, document.creator, ''),
    work: first(document.work, document.title, document.name, id),
    status: text(first(document.status, document.availability, document.visibility), 'available'),
    availability: availabilityLabel(document),
    available: isTextAvailable(document),
    unitCount: count(first(document.unitCount, document.unit_count, document.count)),
    versions,
    versionMap: new Map(versions.map(version => [version.id, version])),
  };
}

export function normalizeUnit(raw, index, identity = {}) {
  const item = object(raw) ? clone(raw) : {text: text(raw)};
  const unitId = first(item.id, item.unitId, item.unit_id, item.segmentId, item.segment_id, item.textUnitId, item.text_unit_id, null);
  const providerReference = first(item.reference, item.anchor, item.sourceReference, null);
  const reference = typeof providerReference === 'string' ? null : exactReference(providerReference || item, {...identity, unitId});
  if (reference && !reference.unitId && unitId !== null && unitId !== undefined) reference.unitId = unitId;
  const value = first(item.text, item.content, item.value, item.body, item.wording, '');
  const heading = item.heading === true || ['heading', 'chapter', 'section', 'title'].includes(String(first(item.kind, item.type, '')).toLowerCase());
  return {
    ...item,
    id: unitId,
    unitId,
    ordinal: Number.isSafeInteger(item.ordinal) ? item.ordinal : Number.isSafeInteger(item.index) ? item.index : index,
    kind: text(first(item.kind, item.type), heading ? 'heading' : 'paragraph'),
    heading,
    label: first(item.label, item.locator, item.title, ''),
    text: typeof value === 'string' ? value : localize(value, identity.language || 'ru'),
    reference,
    sourceReference: typeof providerReference === 'string' ? providerReference : null,
  };
}

function cursorFrom(value, direction) {
  if (!object(value)) return first(value, null);
  return first(value[direction], value[`${direction}Cursor`], value[`${direction}_cursor`], value.cursor, value.token, null);
}

export function normalizeWindow(raw = {}, {document = {}, version = {}} = {}) {
  const identity = versionIdentity(version, document);
  const source = object(raw) ? raw : {units: raw};
  const rawUnits = first(source.units, source.items, source.segments, source.paragraphs, source.content, []);
  const units = list(rawUnits).slice(0, READER_LIMITS.windowUnits).map((item, index) => {
    const unit=normalizeUnit(item,index,identity);
    // Bind text to the manifest used for this response. A later manifest
    // refresh must never re-anchor a retained older window.
    unit.reference=unit.reference||unitReference(unit,version,document);
    return unit;
  });
  // Provider adapters are allowed to omit a cursor at a boundary. Keep the
  // public model explicit so a missing value cannot accidentally render as an
  // enabled Previous/Next control.
  const previous = cursorFrom(source, 'previous') ?? null;
  const next = cursorFrom(source, 'next') ?? null;
  const reference = isReference(first(source.reference, source.anchor, source.startReference, null))
    ? validateReference(first(source.reference, source.anchor, source.startReference, null))
    : null;
  return {
    ...withoutPayload(source, ['units', 'items', 'segments', 'paragraphs', 'content']),
    units,
    truncated: list(rawUnits).length > units.length,
    previous,
    next,
    hasPrevious: source.hasPrevious === true || source.has_previous === true || previous !== null,
    hasNext: source.hasNext === true || source.has_next === true || next !== null,
    reference,
    total: count(first(source.total, source.totalCount, source.total_count, source.count)),
    sourceRevision: first(source.sourceRevision, source.source_revision, identity.sourceRevision, null),
    contentRevision: first(source.contentRevision, source.content_revision, source.revision, identity.contentRevision, null),
  };
}

export function normalizeCatalog(raw = {}) {
  if (Array.isArray(raw)) return {items: raw.map(normalizeDocument), nextCursor: null, total: raw.length};
  const source = object(raw) ? raw : {};
  const items = list(first(source.items, source.documents, source.entries, source.results, source.matches)).map(normalizeDocument);
  return {
    ...withoutPayload(source, ['items', 'documents', 'entries', 'results', 'matches']),
    items,
    nextCursor: first(source.nextCursor, source.next_cursor, source.next, null),
    total: count(first(source.total, source.totalCount, source.total_count, source.count)),
  };
}

export function normalizeSearch(raw = {}, {document = {}, version = {}, scope = 'version'} = {}) {
  const source = object(raw) ? raw : {};
  const rawItems = list(Array.isArray(raw) ? raw : first(source.matches, source.results, source.items, []));
  const items = rawItems.slice(0, READER_LIMITS.searchResults).map((item, index) => {
    const match = object(item) ? clone(item) : {text: text(item)};
    const candidateReference = first(match.reference, match.anchor, null);
    const reference = isReference(candidateReference) ? validateReference(candidateReference) : null;
    return {
      ...match,
      id: first(match.id, match.matchId, `${scope}-${index}`),
      reference,
      sourceReference: typeof candidateReference === 'string' ? candidateReference : null,
      unitId: first(match.unitId, match.unit_id, reference?.unitId, null),
      snippet: text(first(match.snippet, match.text, match.excerpt), ''),
      start: first(match.start, match.offsetStart, reference?.selector?.start, null),
      end: first(match.end, match.offsetEnd, reference?.selector?.end, null),
      scope,
    };
  });
  return {
    ...withoutPayload(source, ['matches', 'results', 'items']),
    items,
    nextCursor: first(source.nextCursor, source.next_cursor, source.next, null),
    total: count(first(source.total, source.totalCount, source.total_count, source.count)),
    truncated: rawItems.length > items.length,
    scope,
  };
}

function escaped(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

export function searchUnits(units, query, {limit = READER_LIMITS.searchResults, scope = 'loaded'} = {}) {
  if (typeof query !== 'string' || query.length > READER_LIMITS.query) throw new Error('invalid-query');
  if (!query.trim()) return {items: [], nextCursor: null, total: 0, scope};
  const resultLimit = Number.isSafeInteger(limit)
    ? Math.max(1, Math.min(limit, READER_LIMITS.searchResults))
    : READER_LIMITS.searchResults;
  const matcher = new RegExp(escaped(query), 'giu');
  const items = [];
  for (const [index, unit] of list(units).entries()) {
    const value = text(unit?.text, '');
    matcher.lastIndex = 0;
    let hit;
    while ((hit = matcher.exec(value))) {
      const reference = isReference(unit?.reference) ? validateReference(unit.reference) : null;
      const unitId = reference?.unitId || unit?.unitId || unit?.id || null;
      const start = Array.from(value.slice(0, hit.index)).length;
      const end = start + Array.from(hit[0]).length;
      const matchReference = reference
        ? validateReference({...reference, selector: {...reference.selector, start, end}})
        : null;
      items.push({id: `${scope}-${index}-${hit.index}`, unitId, start, end, snippet: value, reference: matchReference, scope});
      // The bounded local scan stops before it can establish a corpus total.
      // Keep that uncertainty visible instead of presenting the cache cap as
      // an exact number of matches.
      if (items.length >= resultLimit) return {items, nextCursor: null, total: null, truncated: true, scope};
      if (!hit[0].length) matcher.lastIndex += 1;
    }
  }
  return {items, nextCursor: null, total: items.length, truncated: false, scope};
}

function identityFor(document, version) {
  return versionIdentity(version, document);
}

function keyFor(documentId, versionId) {
  return `${documentId}\u0000${versionId}`;
}

function availableVersion(document, versionId, locale) {
  const versions = list(document?.versions);
  if (versionId) return versions.find(item => item.id === versionId || item.versionId === versionId) || null;
  return versions.find(item => item.id === versionId || item.versionId === versionId)
    || versions.find(item => item.language === locale)
    || versions.find(item => item.available)
    || versions[0]
    || null;
}

function changedReference(reference, version, document = {}) {
  if (!reference || !version) return false;
  const target = object(reference.target) ? reference.target : reference;
  const identity = versionIdentity(version, {id: first(target.workId, reference.documentId, document.id)});
  const expectedDocumentId = first(target.workId, reference.documentId, null);
  if (expectedDocumentId && document.id && expectedDocumentId !== document.id) return true;
  if (reference.versionId && identity.versionId && reference.versionId !== identity.versionId) return true;
  const expectedRevision = first(
    target.textLayerSha256,
    reference.contentRevision,
    reference.sourceRevision,
    reference.revision,
  );
  const currentRevision = first(
    version.source?.textLayerSha256,
    version.contentRevision,
    version.revision,
    identity.contentRevision,
  );
  return Boolean(expectedRevision && currentRevision && expectedRevision !== currentRevision);
}

export function createCorpusReaderModel({provider, notebook = null, locale = 'ru', onChange = () => {}} = {}) {
  if (typeof onChange !== 'function') throw new Error('invalid-onChange');
  const state = {
    catalog: {items: [], query: '', nextCursor: null, total: null, busy: false, error: null},
    document: null,
    activeVersionId: null,
    windows: {},
    searches: {},
    windowErrors: {},
    referenceStatus: 'exact',
    referenceStates: {},
    error: null,
  };
  const requests = new Map();
  const windowUse = new Map();
  let visibleWindowKeys = new Set();
  let sequence = 0;
  let latestOpen = 0;
  let destroyed = false;

  const snapshot = () => ({
    ...clone(state),
    catalog: {...state.catalog, items: state.catalog.items.map(item => ({...item, versionMap: undefined}))},
    document: state.document ? {...state.document, versionMap: undefined} : null,
  });
  const notify = () => onChange(snapshot());

  function setReferenceStatus(documentId, versionId, status) {
    if (state.document?.id !== documentId) return;
    const key = keyFor(documentId, versionId);
    delete state.referenceStates[key];
    state.referenceStates[key] = status;
    const keys = Object.keys(state.referenceStates);
    for (const oldKey of keys.slice(0, Math.max(0, keys.length - 100))) delete state.referenceStates[oldKey];
    if (state.activeVersionId === versionId) state.referenceStatus = status;
  }

  async function run(key, operation) {
    if (destroyed) return null;
    const previous = requests.get(key);
    previous?.controller.abort();
    const controller = new AbortController();
    const token = ++sequence;
    const entry = {controller, token};
    requests.set(key, entry);
    try {
      const value = await operation(controller.signal);
      if (requests.get(key) !== entry || controller.signal.aborted || destroyed) return null;
      return value;
    } finally {
      if (requests.get(key) === entry) requests.delete(key);
    }
  }

  async function catalog({query = '', cursor = null, limit = READER_LIMITS.catalogPage} = {}) {
    if (typeof query !== 'string' || query.length > READER_LIMITS.query) throw new Error('invalid-query');
    state.catalog.busy = true; state.catalog.error = null; state.catalog.query = query; notify();
    try {
      const value = await run('catalog', signal => callProvider(provider, 'catalog', {query, cursor, limit: Math.min(limit, READER_LIMITS.catalogPage), signal}));
      if (!value) return null;
      const page = normalizeCatalog(value);
      const incoming = cursor ? [...state.catalog.items, ...page.items] : page.items;
      const unique = new Map(incoming.map(item => [item.id, item]));
      state.catalog.items = [...unique.values()].slice(-CATALOG_CACHE_LIMIT);
      state.catalog.nextCursor = page.nextCursor; state.catalog.total = page.total; state.catalog.busy = false; notify();
      return page;
    } catch (error) {
      state.catalog.busy = false; state.catalog.error = error; notify(); throw error;
    }
  }

  async function readDocument(documentId, {refresh = false} = {}) {
    const existing = state.catalog.items.find(item => item.id === documentId);
    if (existing && existing.versions.length && !refresh) return existing;
    const value = await run(`document:${documentId}`, signal => callProvider(provider, 'document', {documentId, signal}));
    if (!value) return null;
    const document = normalizeDocument(value);
    const index = state.catalog.items.findIndex(item => item.id === document.id);
    if (index < 0) state.catalog.items = [...state.catalog.items, document].slice(-CATALOG_CACHE_LIMIT);
    else state.catalog.items = state.catalog.items.map((item, itemIndex) => itemIndex === index ? document : item);
    return document;
  }

  async function open({documentId, versionId, reference = null, unitId = null, revision = null} = {}) {
    const openToken = ++latestOpen;
    // A deep link carries unitId/revision separately. Only a provider-issued
    // reference is treated as a validated source anchor.
    let requestedReference = isReference(reference) ? validateReference(reference) : null;
    let document = documentId ? await readDocument(documentId, {refresh: true}) : state.document || state.catalog.items[0];
    if (openToken !== latestOpen) return null;
    if (!document) {
      if (!state.catalog.items.length) await catalog();
      document = state.catalog.items[0] || null;
    }
    if (openToken !== latestOpen) return null;
    if (!document) throw new Error('document-unavailable');
    if (!document.versions?.length && document.id) document = await readDocument(document.id);
    if (openToken !== latestOpen) return null;
    const active = availableVersion(document, versionId, locale);
    if (!active) throw new Error('version-unavailable');
    if (!requestedReference && !unitId && !revision) {
      const remembered = await storedReading(document, active);
      if (remembered?.reference) requestedReference = remembered.reference;
    }
    if (openToken !== latestOpen) return null;
    if (state.document?.id !== document.id) state.referenceStates = {};
    state.document = document; state.activeVersionId = active.id; state.error = null;
    const linkReference = requestedReference || (unitId || revision ? {versionId: active.id, unitId, contentRevision: revision} : null);
    setReferenceStatus(document.id, active.id, changedReference(linkReference, active, document) ? 'changed' : 'exact');
    notify();
    // A deep link whose layer identity is already known to be stale must stay
    // a labelled stale link. Retargeting it to a current window would make a
    // hash/bookmark appear valid while silently changing its meaning.
    if (state.referenceStatus === 'changed') {
      state.error = null;
      notify();
      return null;
    }
    if (requestedReference && providerMethod(provider, 'resolve')) {
      const resolved = await run(`resolve:${document.id}\u0000${active.id}`, signal => callProvider(provider, 'resolve', {documentId: document.id, versionId: active.id, reference: exactReference(requestedReference, identityFor(document, active)), signal}));
      if (openToken !== latestOpen) return null;
      if (resolved) {
        if (Array.isArray(resolved.units)) {
          const window = normalizeWindow(resolved, {document, version: active});
          return applyWindow(document, active, window, requestedReference);
        }
        if (resolved.status === 'unavailable') {
          setReferenceStatus(document.id, active.id, 'unavailable'); notify();
          return null;
        }
        if (resolved.status === 'changed') {
          setReferenceStatus(document.id, active.id, 'changed');
          state.error = null;
          notify();
          return null;
        }
        if (resolved.status === 'exact' && isReference(resolved.reference)) {
          const exact = validateReference(resolved.reference);
          // The compact deep-link form is accepted by both the public fixture
          // and future transports. Keep the full reference in notebook state;
          // the window request only needs the stable unit and layer revision.
          return loadWindow({documentId: document.id, versionId: active.id, unitId: exact.unitId, revision: active.revision, openToken});
        }
      }
    }
    return loadWindow({documentId: document.id, versionId: active.id, reference: requestedReference, unitId, revision, openToken});
  }

  function applyWindow(document, version, window, requestedReference = null, requestedLink = null) {
    const key = keyFor(document.id, version.id);
    state.windows[key] = {...window, documentId: document.id, versionId: version.id, requestedReference: requestedReference ? exactReference(requestedReference, identityFor(document, version)) : null};
    delete state.windowErrors[key];
    windowUse.set(key, ++sequence);
    while (Object.keys(state.windows).length > 2) {
      const keys = Object.keys(state.windows);
      const candidates = keys.filter(item => !visibleWindowKeys.has(item));
      const victim = [...(candidates.length ? candidates : keys)].sort((left, right) => (windowUse.get(left) || 0) - (windowUse.get(right) || 0))[0];
      if (!victim) break;
      delete state.windows[victim];
      windowUse.delete(victim);
    }
    setReferenceStatus(document.id, version.id, changedReference(requestedReference || requestedLink, version, document) ? 'changed' : 'exact');
    state.error = null; notify();
    return state.windows[key];
  }

  async function loadWindow({documentId = state.document?.id, versionId = state.activeVersionId, reference = null, unitId = null, revision = null, direction = 'current', cursor = null, acceptCurrent = false, openToken = null} = {}) {
    let document = state.document?.id === documentId ? state.document : state.catalog.items.find(item => item.id === documentId);
    let refreshedDocument = null;
    if (acceptCurrent && document?.id) {
      document = await readDocument(document.id, {refresh: true});
      refreshedDocument = document;
    }
    if (openToken !== null && openToken !== latestOpen) return null;
    const version = availableVersion(document, versionId, locale);
    if (!document || !version) throw new Error('version-unavailable');
    if (direction === 'current' && !acceptCurrent && !reference && !unitId && !revision) {
      const remembered = await storedReading(document, version);
      if (openToken !== null && openToken !== latestOpen) return null;
      if (remembered?.reference) {
        return loadWindow({
          documentId: document.id,
          versionId: version.id,
          unitId: remembered.reference.unitId,
          revision: remembered.reference.target.textLayerSha256,
          openToken,
        });
      }
    }
    const key = keyFor(document.id, version.id);
    const current = state.windows[key];
    const token = cursor ?? (direction === 'next' ? current?.next : direction === 'previous' ? current?.previous : null);
    const requestedLink = unitId || revision ? {versionId: version.id, unitId, contentRevision: revision} : null;
    // Every displayed page is pinned to the manifest used to construct anchors.
    const pinnedRevision = revision || version.revision;
    try {
      let value;
      if (providerMethod(provider, 'window')) {
        value = await run(`window:${key}`, signal => callProvider(provider, 'window', {
          documentId: document.id,
          versionId: version.id,
          ...(pinnedRevision ? {revision: pinnedRevision} : {}),
          ...(isReference(reference) ? {reference: validateReference(reference)} : {}),
          ...(unitId ? {unitId} : {}),
          ...(revision ? {revision} : {}),
          direction,
          cursor: token,
          limit: READER_LIMITS.windowUnits,
          signal,
        }));
      } else {
        value = {units: version.units, reference};
      }
      if (!value) return null;
      if (openToken !== null && openToken !== latestOpen) return null;
      // Commit a refreshed manifest in the same model notification as the
      // newly fetched text. This keeps the source digest used for location
      // links in lockstep with the displayed current window.
      if (refreshedDocument && state.document?.id === refreshedDocument.id) state.document = refreshedDocument;
      const result = applyWindow(document, version, normalizeWindow(value, {document, version}), reference, requestedLink);
      if (acceptCurrent && !requestedLink && !reference) {
        setReferenceStatus(document.id, version.id, 'exact');
        state.error = null;
        notify();
      }
      return result;
    } catch (error) {
      if (openToken !== null && openToken !== latestOpen) return null;
      // Keep the last readable window. A transient failure must not blank a
      // passage that the researcher is already reading.
      if (error?.code === 'stale_reference') {
        setReferenceStatus(document.id, version.id, 'changed');
        if (current) state.windowErrors[key] = error;
        state.error = null;
        notify();
        return null;
      }
      if (error?.code === 'stale_cursor') {
        setReferenceStatus(document.id, version.id, 'changed');
        if (current) {
          state.windowErrors[key] = error;
          state.error = null;
        } else {
          state.error = error;
        }
        notify();
        return null;
      }
      if (['unavailable_reference', 'unavailable'].includes(error?.code)) {
        // A failed continuation must not turn an already delivered passage
        // into an unavailable source. Keep that page readable and expose the
        // delivery error to the host; only an initial/deep-link failure marks
        // the requested reference unavailable.
        if (current) {
          state.windowErrors[key] = error;
          state.error = null;
          notify();
          return null;
        }
        setReferenceStatus(document.id, version.id, 'unavailable');
        state.error = null;
        notify();
        return null;
      }
      if (current) {
        // A failed refresh or continuation must not blank an already readable
        // page. Keep the failure attached to that pane so the host can expose
        // it without turning a transport problem into a false source state.
        state.windowErrors[key] = error;
        state.error = null;
        notify();
        return null;
      }
      state.error = error; notify(); throw error;
    }
  }

  async function search({documentId = state.document?.id, versionId = state.activeVersionId, query = '', scope = 'loaded', cursor = null} = {}) {
    const document = state.document?.id === documentId ? state.document : state.catalog.items.find(item => item.id === documentId);
    const version = availableVersion(document, versionId, locale);
    if (!document || !version) throw new Error('version-unavailable');
    const key = keyFor(document.id, version.id);
    const current = state.windows[key];
    if (scope === 'loaded') {
      const result = searchUnits(current?.units || [], query, {scope});
      state.searches[key] = result; notify(); return result;
    }
    const capabilities = {
      loaded: true,
      version: Boolean(providerMethod(provider, 'search')) && thisSearchScope(provider, 'version'),
      corpus: Boolean(providerMethod(provider, 'search')) && thisSearchScope(provider, 'corpus'),
    };
    if (scope === 'version' && !capabilities.version) throw new Error('version-search-unavailable');
    if (scope === 'corpus' && !capabilities.corpus) throw new Error('corpus-search-unavailable');
    if (!['version', 'corpus'].includes(scope)) throw new Error('unsupported-search-scope');
    const providerScope = scope === 'version' ? 'document' : scope;
    const value = await run(`search:${key}:${scope}`, signal => callProvider(provider, 'search', {documentId: document.id, versionId: version.id, query, scope: providerScope, cursor, limit: READER_LIMITS.searchResults, signal}));
    if (!value) return null;
    const page = normalizeSearch(value, {document, version, scope});
    const previous = state.searches[key];
    const sameQuery = previous?.query === query && previous?.scope === scope;
    const items = cursor && sameQuery
      ? [...list(previous.items), ...page.items].slice(-SEARCH_CACHE_LIMIT)
      : page.items.slice(0, SEARCH_CACHE_LIMIT);
    const result = {...page, items, query, truncated: items.length < (page.total ?? items.length) && !page.nextCursor};
    state.searches[key] = result; notify(); return result;
  }

  const notebookCache = {
    notes: [], bookmarks: [], positions: [], active: null,
    preferences: {fontSize: 18, lineHeight: 1.9, width: 'comfortable', theme: 'night', mode: 'single', sidebar: true, inspector: false},
  };
  const notebookPageState = {
    cursor: null,
    nextCursor: null,
    total: null,
    documentId: null,
    busy: false,
    error: null,
  };
  let notebookReadyResolve;
  const notebookReady = new Promise(resolve => { notebookReadyResolve = resolve; });
  const pendingNotebookWrites = new Set();
  let notebookLastError = null;
  let notebookWriteSequence = 0;
  let notebookRevision = null;
  const settleNotebook = value => {
    if (value && typeof value.then === 'function') {
      const writeSequence = ++notebookWriteSequence;
      pendingNotebookWrites.add(value);
      value.then(
        () => { pendingNotebookWrites.delete(value); if (writeSequence === notebookWriteSequence) notebookLastError = null; notify(); },
        error => { pendingNotebookWrites.delete(value); if (writeSequence === notebookWriteSequence) notebookLastError = error; notify(); },
      ).catch(() => {});
    }
    return value;
  };
  const notebookState = () => ({...clone(notebookCache), ...clone(notebookPageState)});
  const resumeLoads = new Map();
  function versionReadingSlot(version, document) {
    if (!version?.source || !version.id || !document?.id) return null;
    try {
      // The helper derives a per-work/version key from a validated reference;
      // this seed is never saved and carries no guessed source position.
      const seed = createReference({source: version.source, versionId: version.id, unitId: '__reading_slot__'});
      return readingSlot(seed);
    } catch {
      return null;
    }
  }
  function cacheReading(reading, slot = reading?.slot || null) {
    if (!reading?.reference || !isReference(reading.reference)) return null;
    const record = {reference: clone(reading.reference), fraction: Number.isFinite(reading.offset) ? reading.offset : 0, slot: slot || reading.slot};
    const key = referenceKey(record.reference);
    notebookCache.positions = [...notebookCache.positions.filter(item => referenceKey(item.reference || item.anchor) !== key && item.slot !== record.slot), record].slice(-NOTEBOOK_CACHE_LIMIT);
    if (record.slot === 'primary') notebookCache.active = {documentId: reading.documentId || record.reference.target.workId, version: reading.versionId || record.reference.versionId};
    if (Number.isSafeInteger(reading.revision)) notebookRevision = Math.max(notebookRevision || 0, reading.revision);
    return record;
  }
  async function storedReading(document, version) {
    const cached = notebookCache.positions.find(item => {
      const reference = item.reference || item.anchor;
      return isReference(reference) && reference.target.workId === document?.id && reference.versionId === version?.id;
    });
    if (cached) return cached;
    if (typeof notebook?.loadReading !== 'function') return null;
    const slot = versionReadingSlot(version, document);
    if (!slot) return null;
    if (resumeLoads.has(slot)) return resumeLoads.get(slot);
    const promise = Promise.resolve(notebook.loadReading(slot)).then(reading => reading ? cacheReading(reading, slot) : null).finally(() => resumeLoads.delete(slot));
    resumeLoads.set(slot, promise);
    return promise;
  }
  const notebookCall = (name, ...args) => {
    if (typeof notebook?.[name] !== 'function') return null;
    const value = notebook[name](...args);
    return ['putNote', 'deleteNote', 'saveReading', 'setPreference', 'importPacket'].includes(name)
      ? settleNotebook(value)
      : value;
  };

  function mergeNotebookPage(items, {cursor = null, documentId = null} = {}) {
    const pageItems = list(items).filter(item => object(item) && (item.kind === 'note' || item.kind === 'bookmark'));
    const pageIds = new Set(pageItems.map(item => item.id).filter(value => typeof value === 'string'));
    const existing = cursor
      ? [...notebookCache.notes, ...notebookCache.bookmarks]
      : documentId
        ? [...notebookCache.notes, ...notebookCache.bookmarks].filter(item => item.documentId !== documentId)
        : [];
    const merged = new Map();
    for (const item of [...existing, ...pageItems]) {
      if (item?.id) merged.set(item.id, clone(item));
    }
    // A first page for a document is a replacement for that document's
    // visible records. Other document records remain known to the CAS cache.
    if (cursor && documentId) {
      for (const item of [...notebookCache.notes, ...notebookCache.bookmarks]) {
        if (item?.documentId !== documentId && item?.id && !merged.has(item.id)) merged.set(item.id, clone(item));
      }
    }
    const bounded = [...merged.values()].slice(-NOTEBOOK_CACHE_LIMIT);
    notebookCache.notes = bounded.filter(item => item.kind === 'note');
    notebookCache.bookmarks = bounded.filter(item => item.kind === 'bookmark');
    return pageIds;
  }

  async function notebookPage({cursor = null, documentId = null} = {}) {
    if (cursor !== null && typeof cursor !== 'string') throw new Error('invalid-cursor');
    if (documentId !== null && documentId !== undefined && (typeof documentId !== 'string' || !documentId)) {
      throw new Error('invalid-document-id');
    }
    const filter = documentId || null;
    notebookPageState.busy = true;
    notebookPageState.error = null;
    notebookPageState.cursor = cursor;
    notebookPageState.documentId = filter;
    notify();
    try {
      if (typeof notebook?.listNotes !== 'function') {
        const empty = {items: [], nextCursor: null, total: 0};
        mergeNotebookPage(empty.items, {cursor, documentId: filter});
        notebookPageState.nextCursor = null;
        notebookPageState.total = 0;
        notebookPageState.busy = false;
        notify();
        return empty;
      }
      const page = await notebook.listNotes({limit: READER_LIMITS.notebookPage, cursor, ...(filter ? {documentId: filter} : {})});
      const items = list(page?.items);
      mergeNotebookPage(items, {cursor, documentId: filter});
      notebookPageState.nextCursor = page?.nextCursor ?? null;
      notebookPageState.total = Number.isFinite(page?.total) ? page.total : null;
      notebookPageState.busy = false;
      notify();
      return {
        ...clone(page || {}),
        items: items.map(clone),
        nextCursor: page?.nextCursor ?? null,
        total: Number.isFinite(page?.total) ? page.total : null,
      };
    } catch (error) {
      notebookPageState.busy = false;
      notebookPageState.error = error;
      notebookLastError = error;
      notify();
      throw error;
    }
  }
  function mergeNotebookState(value) {
    if (!object(value)) return;
    if (Array.isArray(value.notes)) notebookCache.notes = value.notes.map(clone);
    if (Array.isArray(value.bookmarks)) notebookCache.bookmarks = value.bookmarks.map(clone);
    if (Array.isArray(value.positions)) notebookCache.positions = value.positions.map(clone);
    if (value.active) notebookCache.active = clone(value.active);
    if (object(value.preferences)) notebookCache.preferences = {...notebookCache.preferences, ...clone(value.preferences)};
  }
  async function hydrateNotebook() {
    try {
      if (typeof notebook?.listNotes === 'function') {
        const values = [];
        let cursor = null;
        do {
          const page = await notebook.listNotes({limit: 200, cursor});
          values.push(...list(page?.items));
          cursor = page?.nextCursor || null;
        } while (cursor && values.length < NOTEBOOK_CACHE_LIMIT);
        const bounded = values.slice(0, NOTEBOOK_CACHE_LIMIT);
        notebookCache.notes = bounded.filter(item => item.kind === 'note');
        notebookCache.bookmarks = bounded.filter(item => item.kind === 'bookmark');
        // Keep the continuation cursor visible to the notebook panel when
        // hydration reached the working-cache bound. Older records can then
        // be requested explicitly without loading an unbounded list at start.
        notebookPageState.nextCursor = cursor && values.length >= NOTEBOOK_CACHE_LIMIT ? cursor : null;
        for (const item of bounded) if (Number.isSafeInteger(item.revision)) notebookRevision = Math.max(notebookRevision || 0, item.revision);
      }
      if (typeof notebook?.loadReading === 'function') {
        notebookCache.positions = [];
        notebookCache.active = null;
        for (const slot of ['primary', 'secondary']) {
          const reading = await notebook.loadReading(slot);
          if (!reading) continue;
          cacheReading(reading, slot);
        }
      }
      if (typeof notebook?.getPreference === 'function') {
        for (const key of Object.keys(notebookCache.preferences)) {
          const value = await notebook.getPreference(key);
          if (value !== undefined) notebookCache.preferences[key] = clone(value);
        }
      }
    } catch (error) {
      state.error = error;
      notebookLastError = error;
    } finally {
      notebookReadyResolve(); notify();
    }
  }
  void hydrateNotebook();
  function anchorFor(unit, version = availableVersion(state.document, state.activeVersionId, locale), selection = null) {
    return unitReference(unit, version || {}, state.document || {}, {
      start: Number.isSafeInteger(selection?.start) ? selection.start : 0,
      end: Number.isSafeInteger(selection?.end) ? selection.end : 0,
    });
  }

  function notebookReference(value) {
    if (!isReference(value)) throw new Error('invalid-reference');
    return validateReference(value);
  }

  return {
    snapshot,
    limits: READER_LIMITS,
    catalog,
    open,
    loadWindow,
    search,
    notebookPage,
    normalizeDocument,
    notebookState,
    setVisibleVersions(versionIds = []) {
      const documentId = state.document?.id;
      visibleWindowKeys = new Set(list(versionIds).filter(value => typeof value === 'string').map(versionId => keyFor(documentId || '', versionId)));
      for (const key of visibleWindowKeys) if (windowUse.has(key)) windowUse.set(key, ++sequence);
      while (Object.keys(state.windows).length > 2) {
        const keys = Object.keys(state.windows);
        const candidates = keys.filter(item => !visibleWindowKeys.has(item));
        const victim = [...(candidates.length ? candidates : keys)].sort((left, right) => (windowUse.get(left) || 0) - (windowUse.get(right) || 0))[0];
        if (!victim) break;
        delete state.windows[victim];
        windowUse.delete(victim);
      }
      notify();
    },
    notebookStatus() {
      let status = null;
      try { status = typeof notebook?.status === 'function' ? notebook.status() : null; } catch (error) { notebookLastError = error; }
      return {
        adapter: status?.adapter || (notebook ? 'pending' : 'session'),
        persistent: status?.persistent === true,
        warning: status?.warning || null,
        pending: pendingNotebookWrites.size,
        error: notebookLastError?.message || null,
      };
    },
    ready: () => notebookReady,
    anchorFor,
    positionFor(reference) {
      return notebookCache.positions.find(item => referenceKey(item.reference || item.anchor) === referenceKey(reference)) || null;
    },
    async savePosition(reference, fraction = 0, slot = 'primary') {
      const checked = notebookReference(reference);
      const checkedSlot = slot === 'secondary' ? 'secondary' : 'primary';
      const record = {reference: clone(checked), fraction, slot: checkedSlot};
      const canonicalSlot = readingSlot(checked);
      const payload = {slot: canonicalSlot, documentId: checked.target.workId, versionId: checked.versionId, reference: checked, offset: checked.selector.start};
      let value = null;
      if (notebook?.saveReading) {
        const writes = [notebookCall('saveReading', payload)];
        // Keep the legacy primary/secondary resume slots useful to older
        // notebook consumers while the canonical slot carries every version.
        if (checkedSlot === 'primary') writes.push(notebookCall('saveReading', {...payload, slot: 'primary'}));
        else if (checkedSlot === 'secondary') writes.push(notebookCall('saveReading', {...payload, slot: 'secondary'}));
        const results = await Promise.all(writes);
        value = results[0];
        for (const result of results) if (Number.isSafeInteger(result?.revision)) notebookRevision = Math.max(notebookRevision || 0, result.revision);
      }
      const cachedRecord = {...record, slot: canonicalSlot};
      const bySlot = new Map(notebookCache.positions
        .filter(item => !sameReference(item.reference || item.anchor, checked))
        .map(item => [item.slot || 'primary', item]));
      bySlot.set(canonicalSlot, cachedRecord);
      if (checkedSlot === 'primary') notebookCache.active = {documentId: checked.target.workId, version: checked.versionId};
      notebookCache.positions = [...bySlot.values()].slice(-NOTEBOOK_CACHE_LIMIT);
      notify(); return value;
    },
    async toggleBookmark(reference, quote = '') {
      const checked = notebookReference(reference);
      const index = notebookCache.bookmarks.findIndex(item => sameReference(item.reference || item.anchor, checked));
      if (index >= 0) {
        const old = notebookCache.bookmarks[index];
        if (old?.id && notebook?.deleteNote) {
          try {
            const value = await notebookCall('deleteNote', old.id, undefined, old.revision);
            notebookCache.bookmarks = notebookCache.bookmarks.filter(item => item.id !== old.id);
            if (Number.isSafeInteger(value?.revision)) notebookRevision = value.revision;
            notify();
            return value;
          } catch (error) {
            notify();
            throw error;
          }
        }
        notebookCache.bookmarks = notebookCache.bookmarks.filter(item => item.id !== old.id);
        notify(); return null;
      }
      if (typeof quote !== 'string' || quote.length > READER_LIMITS.quote) throw new Error('quote-limit');
      const item = {id: `bookmark-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, kind: 'bookmark', reference: clone(checked), ...(quote ? {quote: boundedText(quote, READER_LIMITS.quote)} : {})};
      const value = notebook?.putNote ? await notebookCall('putNote', {...item, expectedRecordRevision: null}) : null;
      const saved = value?.item || item;
      notebookCache.bookmarks.push(saved);
      if (Number.isSafeInteger(value?.revision)) notebookRevision = value.revision;
      notify(); return value;
    },
    async saveNote(reference, value, quote = '') {
      if (typeof value !== 'string' || value.length > READER_LIMITS.note) throw new Error('note-limit');
      if (typeof quote !== 'string' || quote.length > READER_LIMITS.quote) throw new Error('quote-limit');
      const checked = notebookReference(reference);
      const old = notebookCache.notes.find(item => sameReference(item.reference || item.anchor, checked));
      const item = {id: old?.id || `note-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, kind: 'note', reference: clone(checked), text: value, ...(quote ? {quote: boundedText(quote, READER_LIMITS.quote)} : {})};
      const result = notebook?.putNote ? await notebookCall('putNote', {...item, expectedRecordRevision: old?.revision ?? null}) : null;
      const saved = result?.item || item;
      notebookCache.notes = [...notebookCache.notes.filter(entry => entry.id !== item.id), saved];
      if (Number.isSafeInteger(result?.revision)) notebookRevision = result.revision;
      notify(); return result;
    },
    async deleteNote(reference) {
      const checked = notebookReference(reference);
      const removed = notebookCache.notes.find(item => sameReference(item.reference || item.anchor, checked));
      if (!removed) return null;
      const value = notebook?.deleteNote ? await notebookCall('deleteNote', removed.id, undefined, removed.revision) : null;
      notebookCache.notes = notebookCache.notes.filter(item => item.id !== removed.id);
      if (Number.isSafeInteger(value?.revision)) notebookRevision = value.revision;
      notify(); return value;
    },
    exportNotebook() { return notebook?.exportPacket ? notebookCall('exportPacket') : JSON.stringify(notebookState()); },
    importNotebook(value) {
      if (!notebook?.importPacket) throw new Error('notebook-unavailable');
      const result = notebookCall('importPacket', value);
      if (result?.then) return result.then(() => hydrateNotebook());
      notify(); return result;
    },
    setPreferences(value) {
      notebookCache.preferences = {...notebookCache.preferences, ...clone(value)};
      const results = Object.entries(value || {}).map(([key, item]) => notebook?.setPreference ? notebookCall('setPreference', key, item) : null);
      notify(); return results.find(item => item?.then) || results.at(-1) || null;
    },
    searchCapabilities() {
      const hasSearch = Boolean(providerMethod(provider, 'search'));
      return {
        loaded: true,
        version: hasSearch && thisSearchScope(provider, 'version'),
        corpus: hasSearch && thisSearchScope(provider, 'corpus'),
      };
    },
    cancel() { for (const entry of requests.values()) entry.controller.abort(); requests.clear(); },
    destroy() { destroyed = true; for (const entry of requests.values()) entry.controller.abort(); requests.clear(); },
  };
}
