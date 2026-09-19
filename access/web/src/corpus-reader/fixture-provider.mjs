import {
  CORPUS_LIMITS,
  CorpusProviderCursorError,
  CorpusProviderError,
  CorpusProviderUnsupportedScopeError,
  throwIfAborted,
  validateOperationArgs,
} from './provider.mjs';
import {createReference, referenceKey, validateReference} from './model.mjs';

const DEFAULT_DOCUMENT_COUNT = 10_000;
const DEFAULT_UNIT_COUNT = 100_000;
const DEFAULT_LANGUAGES = ['de', 'en', 'ru'];
const UNITS_PER_CHAPTER = 33;

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function assertPositiveInteger(value, name, fallback) {
  const result = value === undefined ? fallback : value;
  if (!Number.isInteger(result) || result < 1) throw new TypeError(`${name} must be a positive integer`);
  return result;
}

function assertDigest(value, name) {
  if (typeof value !== 'string' || !/^[a-f0-9]{64}$/.test(value)) {
    throw new TypeError(`${name} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function digest(value) {
  // This is only a deterministic identity generator for public synthetic
  // fixtures. It is not used as cryptographic evidence.
  let hash = 2166136261;
  const text = String(value);
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  const words = [hash];
  const constants = [2246822519, 3266489917, 668265263, 374761393, 1103515245, 1597334677, 3812015801];
  for (const constant of constants) {
    const previous = words[words.length - 1];
    words.push(Math.imul(previous ^ hash, constant) >>> 0);
  }
  return words.map(word => word.toString(16).padStart(8, '0')).join('');
}

function pad(value, width = 6) {
  return String(value).padStart(width, '0');
}

function workId(index) {
  return `tos.work.fixture-${pad(index + 1)}`;
}

function expressionId(index, language) {
  return `tos.expression.fixture-${pad(index + 1)}-${language}`;
}

function editionId(index, language) {
  return `tos.edition.fixture-${pad(index + 1)}-${language}`;
}

function itemId(index, language) {
  return `tos.item.fixture-${pad(index + 1)}-${language}`;
}

function fileId(index, language) {
  return `tos.file.fixture-${pad(index + 1)}-${language}`;
}

function textLayerRef(index, language) {
  return `fixture/text/${pad(index + 1)}/${language}/source.txt`;
}

function sourceFor(index, language, revision) {
  const document = workId(index);
  const expression = expressionId(index, language);
  const edition = editionId(index, language);
  const item = itemId(index, language);
  const file = fileId(index, language);
  return {
    workId: document,
    expressionId: expression,
    editionId: edition,
    itemId: item,
    fileId: file,
    fileSha256: digest(`${file}:file:${revision}`),
    textLayerRef: textLayerRef(index, language),
    textLayerSha256: digest(`${file}:text-layer:${revision}`),
  };
}

function versionId(index, language) {
  return expressionId(index, language);
}

function unitId(index, language, ordinal) {
  // The first eight hex digits carry only a synthetic fixture ordinal. The
  // remaining digest keeps the opaque ID unique across document/version.
  const ordinalPart = ordinal.toString(16).padStart(8, '0');
  return `tos.text-unit.sid-${ordinalPart}${digest(`${index}:${language}:${ordinal}`).slice(0, 24)}`;
}

function ordinalFromUnitId(value) {
  if (typeof value !== 'string' || !/^tos\.text-unit\.sid-[a-f0-9]{32}$/.test(value)) return null;
  const ordinal = Number.parseInt(value.slice('tos.text-unit.sid-'.length, 'tos.text-unit.sid-'.length + 8), 16);
  return Number.isSafeInteger(ordinal) && ordinal > 0 ? ordinal : null;
}

function unitKind(ordinal) {
  if ((ordinal - 1) % UNITS_PER_CHAPTER === 0) return 'section';
  if (ordinal % 11 === 0) return 'note';
  if (ordinal % 5 === 0) return 'verse_line';
  return 'paragraph';
}

function unitReference(ordinal) {
  const chapter = Math.floor((ordinal - 1) / UNITS_PER_CHAPTER) + 1;
  const withinChapter = ((ordinal - 1) % UNITS_PER_CHAPTER) + 1;
  return `chapter-${pad(chapter, 3)}/unit-${pad(withinChapter, 3)}`;
}

function fixtureText(language, kind, index, ordinal) {
  const number = pad(ordinal, 6);
  const chapter = Math.floor((ordinal - 1) / UNITS_PER_CHAPTER) + 1;
  const suffix = kind === 'section'
    ? `Chapter ${chapter}: a synthetic section for work ${pad(index + 1)}`
    : kind === 'note'
      ? `Synthetic note ${number} records a bounded contextual aside.`
      : kind === 'verse_line'
        ? `A measured synthetic line ${number} keeps its line boundary.`
        : `Synthetic paragraph ${number} carries a stable source unit.`;
  if (language === 'de') return `DE — ${suffix}`;
  if (language === 'ru') return `RU — ${suffix}`;
  if (language === 'ar') {
    const arabic = kind === 'section'
      ? `الفصل ${chapter}: قسم اصطناعي للعمل ${pad(index + 1)}`
      : kind === 'note'
        ? `ملاحظة اصطناعية ${number} تحفظ سياقاً محدوداً.`
        : `مقطع اصطناعي ${number} يحفظ وحدة مصدر مستقرة.`;
    return `AR — ${arabic}`;
  }
  if (language === 'el' || language === 'grc') {
    const greek = kind === 'section'
      ? `Κεφάλαιο ${chapter}: συνθετική ενότητα για το έργο ${pad(index + 1)}`
      : kind === 'note'
        ? `Συνθετική σημείωση ${number} διατηρεί το τοπικό πλαίσιο.`
        : `Συνθετικό απόσπασμα ${number} διατηρεί σταθερή μονάδα πηγής.`;
    return `GRC — ${greek}`;
  }
  return `EN — ${suffix}`;
}

function cursorEncode(payload) {
  return `tos-cursor-1.${encodeURIComponent(JSON.stringify(payload))}`;
}

function cursorDecode(value, operation) {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string' || !value.startsWith('tos-cursor-1.')) {
    throw new CorpusProviderCursorError('The corpus cursor is malformed', {operation});
  }
  try {
    const payload = JSON.parse(decodeURIComponent(value.slice('tos-cursor-1.'.length)));
    if (!isRecord(payload) || payload.operation !== operation || !Number.isInteger(payload.offset) || payload.offset < 0) {
      throw new Error('invalid cursor payload');
    }
    return payload;
  } catch (error) {
    if (error instanceof CorpusProviderCursorError) throw error;
    throw new CorpusProviderCursorError('The corpus cursor is malformed', {operation});
  }
}

function failureError(operation, configured) {
  if (configured instanceof Error) return configured;
  if (isRecord(configured)) {
    return new CorpusProviderError(
      typeof configured.code === 'string' ? configured.code : 'fixture_failure',
      typeof configured.message === 'string' ? configured.message : `Synthetic ${operation} failure`,
      operation,
      configured.details,
    );
  }
  return new CorpusProviderError('fixture_failure', `Synthetic ${operation} failure`, operation);
}

function abortError(signal) {
  if (signal?.reason instanceof Error) return signal.reason;
  const error = new Error('The fixture corpus request was aborted');
  error.name = 'AbortError';
  return error;
}

function wait(ms, signal) {
  throwIfAborted(signal);
  if (ms <= 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    let timer = setTimeout(done, ms);
    const onAbort = () => {
      clearTimeout(timer);
      timer = null;
      reject(abortError(signal));
    };
    const cleanup = () => signal?.removeEventListener('abort', onAbort);
    function done() {
      if (timer === null) return;
      timer = null;
      cleanup();
      resolve();
    }
    signal?.addEventListener('abort', onAbort, {once: true});
  });
}

function yieldToEventLoop(signal) {
  throwIfAborted(signal);
  return new Promise((resolve, reject) => {
    let timer = setTimeout(done, 0);
    const onAbort = () => {
      clearTimeout(timer);
      timer = null;
      reject(abortError(signal));
    };
    const cleanup = () => signal?.removeEventListener('abort', onAbort);
    function done() {
      if (timer === null) return;
      timer = null;
      cleanup();
      resolve();
    }
    signal?.addEventListener('abort', onAbort, {once: true});
  });
}

function queryText(value) {
  return String(value).trim().toLocaleLowerCase();
}

function sourceEquals(left, right) {
  return Object.keys(right).every(key => left[key] === right[key]);
}

function optionValue(option, context) {
  if (typeof option === 'function') return option(context);
  if (!isRecord(option)) return option;
  const keys = [
    context.operation,
    context.documentId,
    context.versionId,
    context.documentId && context.language ? `${context.documentId}:${context.language}` : null,
    Number.isSafeInteger(context.documentIndex) && context.language ? `${context.documentIndex + 1}:${context.language}` : null,
    Number.isSafeInteger(context.documentIndex) ? String(context.documentIndex + 1) : null,
  ].filter(Boolean);
  for (const key of keys) if (Object.prototype.hasOwnProperty.call(option, key)) return option[key];
  return undefined;
}

function setContains(set, index, document) {
  return set.has(index) || set.has(index + 1) || set.has(document);
}

function normalizeAvailability(value, fallback = 'available') {
  if (value === undefined || value === null) return {status: fallback};
  if (value === true) return {status: 'available'};
  if (value === false) return {status: 'unavailable'};
  if (typeof value === 'string') return {status: value};
  if (isRecord(value)) {
    const visibility = String(value.visibility ?? '').trim().toLowerCase();
    const status = value.status ?? value.availability
      ?? (value.available === false || value.textAvailable === false || value.text_available === false ? 'unavailable' : null)
      ?? (['restricted', 'public_metadata_only', 'local_only', 'unknown'].includes(visibility) ? visibility : fallback);
    return {...value, status};
  }
  throw new TypeError('fixture availability must be a boolean, string or object');
}

function normalizedStatus(value) {
  return String(value?.status ?? value?.availability ?? '').trim().toLowerCase().replace(/-/g, '_');
}

function carriesText(value) {
  if (value.available === false || value.textAvailable === false || value.text_available === false) return false;
  return !['unavailable', 'metadata_only', 'metadata', 'restricted', 'public_metadata_only', 'local_only', 'link_only', 'missing', 'expired', 'text_unavailable', 'withheld', 'unknown', 'pending', 'not_available', 'not_indexed', 'rights_unknown'].includes(normalizedStatus(value))
    && !['restricted', 'public_metadata_only', 'local_only', 'unknown'].includes(String(value.visibility ?? '').trim().toLowerCase());
}

function unknownCount(option, context) {
  const value = optionValue(option, context);
  return value === true || value === 'unknown' || value === 'null' || value === null;
}

function pageShouldBeEmpty(option, context) {
  if (context.cursor?.emptyPage === true) return false;
  const value = optionValue(option, context);
  if (value === true) return context.offset === 0;
  if (Array.isArray(value)) return value.includes(context.offset);
  if (typeof value === 'number') return value === context.offset;
  if (typeof value === 'string') return value === 'first' ? context.offset === 0 : false;
  return false;
}

/**
 * A lazy public fixture provider. It materializes only requested catalog
 * items and text units; even a 10,000-document/100,000-unit corpus is kept as
 * arithmetic parameters rather than arrays of source payloads.
 */
export function createFixtureProvider(options = {}) {
  if (!isRecord(options)) throw new TypeError('fixture options must be an object');
  const documentCount = assertPositiveInteger(options.documentCount, 'documentCount', DEFAULT_DOCUMENT_COUNT);
  const unitCount = assertPositiveInteger(options.unitCount, 'unitCount', DEFAULT_UNIT_COUNT);
  const unitsPerDocument = options.unitsPerDocument === undefined
    ? null
    : assertPositiveInteger(options.unitsPerDocument, 'unitsPerDocument', 1);
  const longDocumentUnits = options.longDocumentUnits === undefined
    ? null
    : assertPositiveInteger(options.longDocumentUnits, 'longDocumentUnits', 1);
  if (unitsPerDocument !== null && longDocumentUnits !== null) {
    throw new TypeError('unitsPerDocument and longDocumentUnits are mutually exclusive');
  }
  const languages = options.languages === undefined ? [...DEFAULT_LANGUAGES] : [...options.languages];
  if (languages.length === 0 || languages.some(language => typeof language !== 'string' || language.length === 0)) {
    throw new TypeError('languages must contain at least one non-empty language tag');
  }
  const originalLanguage = options.originalLanguage ?? languages[0];
  if (!languages.includes(originalLanguage)) throw new TypeError('originalLanguage must be in languages');
  const maxLatency = options.latencyMs === undefined ? 0 : options.latencyMs;
  if (typeof maxLatency !== 'number' && typeof maxLatency !== 'function') {
    throw new TypeError('latencyMs must be a non-negative number or function');
  }
  if (typeof maxLatency === 'number' && (!Number.isFinite(maxLatency) || maxLatency < 0)) {
    throw new TypeError('latencyMs must be a non-negative number');
  }

  const unknownTotals = options.unknownTotals ?? options.unknownTotal ?? false;
  const unknownUnitCounts = options.unknownUnitCounts ?? options.unknownUnitCount ?? false;
  let emptyPageConfig = options.emptyPages ?? options.emptyIntermediatePages ?? {};
  const metadataOnlyDocuments = new Set(options.metadataOnlyDocuments ?? []);
  const restrictedDocuments = new Set(options.restrictedDocuments ?? []);
  const unavailableDocuments = new Set(options.unavailableDocuments ?? []);
  const configuredDocumentAvailability = options.documentAvailability ?? options.documentAvailabilities ?? null;
  const configuredVersionAvailability = options.versionAvailability ?? options.versionAvailabilities ?? null;
  const includeCatalogVersions = options.includeCatalogVersions === true
    || configuredVersionAvailability !== null
    || metadataOnlyDocuments.size > 0
    || restrictedDocuments.size > 0
    || unavailableDocuments.size > 0;
  if (!['boolean', 'function'].includes(typeof unknownTotals) && !isRecord(unknownTotals)) {
    throw new TypeError('unknownTotals must be a boolean, object or function');
  }
  if (!['boolean', 'function'].includes(typeof unknownUnitCounts) && !isRecord(unknownUnitCounts)) {
    throw new TypeError('unknownUnitCounts must be a boolean, object or function');
  }
  if (!['boolean', 'function'].includes(typeof emptyPageConfig) && !isRecord(emptyPageConfig) && !Array.isArray(emptyPageConfig)) {
    throw new TypeError('emptyPages must be a boolean, array, object or function');
  }

  let latencyMs = maxLatency;
  let epoch = 0;
  let globalRevision = digest('fixture:revision:0');
  const revisionOverrides = new Map();
  const failures = new Map();
  const changedReferences = new Set();
  const responseVariants = new Map(Object.entries(options.responseVariants ?? options.invalidPackets ?? {}));
  const metricsState = {
    requests: Object.fromEntries(['catalog', 'document', 'window', 'search', 'resolve', 'structure'].map(operation => [operation, 0])),
    failed: 0,
    aborted: 0,
    activeRequests: 0,
    maxActiveRequests: 0,
    materializedCatalogItems: 0,
    materializedUnits: 0,
    maxPageItems: 0,
    maxPageUnits: 0,
  };

  function unitsForDocument(index) {
    if (unitsPerDocument !== null) return unitsPerDocument;
    const base = Math.floor(unitCount / documentCount);
    const remainder = unitCount % documentCount;
    if (index === 0 && longDocumentUnits !== null) return longDocumentUnits;
    return base + (index < remainder ? 1 : 0);
  }

  const baselineFirstUnits = Math.floor(unitCount / documentCount) + (unitCount % documentCount > 0 ? 1 : 0);
  const totalUnits = unitsPerDocument !== null
    ? documentCount * unitsPerDocument
    : longDocumentUnits !== null
      ? unitCount - baselineFirstUnits + longDocumentUnits
      : unitCount;

  function documentIndex(document) {
    if (typeof document !== 'string') return null;
    const match = /^tos\.work\.fixture-(\d+)$/.exec(document);
    if (!match) return null;
    const index = Number.parseInt(match[1], 10) - 1;
    return Number.isSafeInteger(index) && index >= 0 && index < documentCount ? index : null;
  }

  function currentRevision(index, language) {
    const key = `${workId(index)}:${language}`;
    return revisionOverrides.get(key) ?? globalRevision;
  }

  function documentAvailability(index) {
    const document = workId(index);
    let state = normalizeAvailability(optionValue(configuredDocumentAvailability, {
      operation: 'document', documentIndex: index, documentId: document,
    }));
    if (setContains(metadataOnlyDocuments, index, document)) state = {...state, status: 'metadata_only'};
    if (setContains(restrictedDocuments, index, document)) state = {...state, status: 'restricted'};
    if (unavailableDocuments.has(document)) state = {...state, status: 'unavailable'};
    return {
      ...state,
      status: state.status ?? 'available',
      available: carriesText(state),
      textAvailable: carriesText(state),
      unitCount: carriesText(state) ? unitsForDocument(index) : null,
      visibility: state.visibility ?? (carriesText(state) ? 'public' : 'public_metadata_only'),
    };
  }

  function versionAvailability(index, language) {
    const documentState = documentAvailability(index);
    const configured = optionValue(configuredVersionAvailability, {
      operation: 'version', documentIndex: index, documentId: workId(index),
      language, versionId: versionId(index, language),
    });
    const state = normalizeAvailability(configured, documentState.status);
    const merged = {
      ...documentState,
      ...state,
      status: state.status ?? documentState.status,
    };
    return {
      ...merged,
      available: carriesText(merged),
      textAvailable: carriesText(merged),
      unitCount: carriesText(merged) ? unitsForDocument(index) : null,
      visibility: merged.visibility ?? (carriesText(merged) ? 'public' : 'public_metadata_only'),
    };
  }

  function unknownTotal(operation, args, offset = 0) {
    return unknownCount(unknownTotals, {operation, ...args, offset});
  }

  function unknownUnitCount(operation, args) {
    return unknownCount(unknownUnitCounts, {operation, ...args});
  }


  function versionSource(index, language) {
    return sourceFor(index, language, currentRevision(index, language));
  }

  function languagesForDocument() {
    return languages;
  }

  function versionFor(index, value) {
    for (const language of languagesForDocument()) {
      if (versionId(index, language) === value) return language;
    }
    return null;
  }

  function itemFor(index, language, ordinal) {
    const kind = unitKind(ordinal);
    const context = {
      chapter: Math.floor((ordinal - 1) / UNITS_PER_CHAPTER) + 1,
      kind,
    };
    const graphTarget = graphTargetFor(index, language, ordinal, kind);
    if (graphTarget !== undefined) context.graphTarget = graphTarget;
    metricsState.materializedUnits += 1;
    return {
      id: unitId(index, language, ordinal),
      ordinal,
      kind,
      text: fixtureText(language, kind, index, ordinal),
      language,
      reference: unitReference(ordinal),
      context,
    };
  }

  function graphTargetFor(index, language, ordinal, kind) {
    const mapping = options.graphTarget ?? options.graphTargets;
    if (mapping === undefined) return undefined;
    const value = typeof mapping === 'function'
      ? mapping({documentIndex: index, documentId: workId(index), language, ordinal, kind})
      : mapping[`${index + 1}:${language}:${ordinal}`] ?? mapping[`${index + 1}:${ordinal}`] ?? mapping[kind];
    if (value === undefined || value === null) return undefined;
    if (!isRecord(value) || !['node', 'relation'].includes(value.kind) || typeof value.id !== 'string' || value.id.length === 0) {
      throw new TypeError('graphTarget mapping values must be {kind: node|relation, id}');
    }
    return {kind: value.kind, id: value.id};
  }

  async function run(operation, args, action) {
    metricsState.requests[operation] += 1;
    metricsState.activeRequests += 1;
    metricsState.maxActiveRequests = Math.max(metricsState.maxActiveRequests, metricsState.activeRequests);
    try {
      throwIfAborted(args.signal);
      const configuredLatency = typeof latencyMs === 'function' ? latencyMs(operation, args) : latencyMs;
      if (!Number.isFinite(configuredLatency) || configuredLatency < 0) throw new TypeError('latencyMs returned an invalid value');
      await wait(configuredLatency, args.signal);
      throwIfAborted(args.signal);
      const configuredFailure = failures.get(operation);
      const failure = typeof configuredFailure === 'function' ? configuredFailure(args) : configuredFailure;
      if (failure) throw failureError(operation, failure);
      return applyResponseVariant(operation, args, await action());
    } catch (error) {
      if (error?.name === 'AbortError') metricsState.aborted += 1;
      else if (error?.code === 'fixture_failure') metricsState.failed += 1;
      throw error;
    } finally {
      metricsState.activeRequests -= 1;
    }
  }

  function decodePageCursor(operation, cursor, expected) {
    const payload = cursorDecode(cursor, operation);
    if (payload === null) return {offset: 0};
    if (payload.epoch !== epoch || Object.entries(expected).some(([key, value]) => payload[key] !== value)) {
      throw new CorpusProviderCursorError('The fixture cursor belongs to an older corpus revision', {operation});
    }
    return payload;
  }

  function applyResponseVariant(operation, args, response) {
    const configured = responseVariants.get(operation);
    if (configured === undefined || configured === null || configured === false) return response;
    if (typeof configured === 'function') return configured({operation, args, response});
    if (isRecord(configured)) return {...response, ...configured};
    switch (configured) {
      case 'unknown-total':
      case 'unknown_total':
        return {...response, total: null};
      case 'omit-total':
      case 'omit_total': {
        const {total: ignored, ...rest} = response;
        return rest;
      }
      case 'empty-page':
      case 'empty_page':
        return response.units
          ? {...response, units: []}
          : {...response, items: []};
      case 'duplicate-unit':
      case 'duplicate_unit':
        return response.units?.length ? {...response, units: [...response.units, response.units[0]]} : response;
      case 'oversized-text':
      case 'oversized_text':
        return response.units?.length
          ? {...response, units: [{...response.units[0], text: 'x'.repeat(CORPUS_LIMITS.maxUnitTextCharacters + 1)}, ...response.units.slice(1)]}
          : response;
      case 'wrong-identity':
      case 'wrong_identity':
        return response.documentId ? {...response, documentId: `${response.documentId}:wrong`} : response;
      case 'missing-next-cursor':
      case 'missing_next_cursor':
        return {...response, nextCursor: 42};
      default:
        throw new TypeError(`unknown fixture response variant ${configured}`);
    }
  }

  function catalogItem(index) {
    metricsState.materializedCatalogItems += 1;
    const availability = documentAvailability(index);
    const item = {
      id: workId(index),
      title: `Fixture Work ${pad(index + 1)}`,
      kind: 'work',
      languages: [...languages],
      unitCount: (unknownUnitCount('catalog', {documentId: workId(index)}) || !availability.available) ? null : availability.unitCount,
      status: availability.status,
      availability: availability.status,
      visibility: availability.visibility,
      textAvailable: availability.textAvailable,
    };
    if (includeCatalogVersions) {
      item.versions = languages.map(language => {
        const version = versionAvailability(index, language);
        return {
          id: versionId(index, language),
          label: language === originalLanguage ? `Original (${language})` : `Translation (${language})`,
          language,
          role: language === originalLanguage ? 'original' : 'translation',
          status: version.status,
          availability: version.status,
          visibility: version.visibility,
          available: version.available,
          textAvailable: version.textAvailable,
          unitCount: unknownUnitCount('catalog', {documentId: workId(index), versionId: versionId(index, language)}) ? null : version.unitCount,
        };
      });
    }
    return item;
  }

  function structureItem(index, language, chapter) {
    const ordinal = (chapter - 1) * UNITS_PER_CHAPTER + 1;
    return {
      id: `tos.structure.fixture-${pad(index + 1)}-${language}-chapter-${pad(chapter, 4)}`,
      label: `Chapter ${pad(chapter, 3)}`,
      unitId: unitId(index, language, ordinal),
      level: 0,
      hasChildren: false,
    };
  }

  function manifest(index) {
    const availability = documentAvailability(index);
    return {
      id: workId(index),
      title: `Fixture Work ${pad(index + 1)}`,
      contentPosture: 'public_synthetic_contract_exercise',
      status: availability.status,
      availability: availability.status,
      visibility: availability.visibility,
      textAvailable: availability.textAvailable,
      unitCount: unknownUnitCount('document', {documentId: workId(index)}) ? null : availability.unitCount,
      source: {workId: workId(index)},
      versions: languages.map(language => {
        const version = versionAvailability(index, language);
        const result = {
          id: versionId(index, language),
          label: language === originalLanguage ? `Original (${language})` : `Translation (${language})`,
          language,
          role: language === originalLanguage ? 'original' : 'translation',
          status: version.status,
          availability: version.status,
          visibility: version.visibility,
          available: version.available,
          textAvailable: version.textAvailable,
          unitCount: unknownUnitCount('document', {documentId: workId(index), versionId: versionId(index, language)}) ? null : version.unitCount,
        };
        if (version.available) {
          const source = versionSource(index, language);
          result.revision = source.textLayerSha256;
          result.source = source;
        }
        return result;
      }),
    };
  }

  function resolveStart(index, language, args, revision) {
    const reference = args.reference === undefined || args.reference === null
      ? (args.unitId ? {unitId: args.unitId, revision: args.revision} : null)
      : args.reference;
    if (reference === null) return 0;
    let ordinal;
    if (isRecord(reference) && !reference.schemaVersion && Object.prototype.hasOwnProperty.call(reference, 'unitId')) {
      ordinal = ordinalFromUnitId(reference.unitId);
      if (!Number.isInteger(ordinal) || ordinal < 1 || ordinal > unitsForDocument(index) ||
        reference.unitId !== unitId(index, language, ordinal)) {
        throw new CorpusProviderError('invalid_request', 'The deep link unit does not belong to this version', 'window');
      }
      if (reference.revision !== revision) {
        throw new CorpusProviderError('stale_reference', 'The deep link revision does not match the current text layer', 'window');
      }
    } else if (isRecord(reference) && reference.schemaVersion) {
      const canonical = validateReference(reference);
      if (canonical.versionId !== versionId(index, language)) {
        throw new CorpusProviderError('invalid_request', 'The reference targets another version', 'window');
      }
      ordinal = ordinalFromUnitId(canonical.unitId);
      if (!Number.isInteger(ordinal) || ordinal < 1 || ordinal > unitsForDocument(index) ||
        canonical.unitId !== unitId(index, language, ordinal)) {
        throw new CorpusProviderError('invalid_request', 'The text reference unit does not belong to this version', 'window');
      }
      if (canonical.target.textLayerSha256 !== versionSource(index, language).textLayerSha256) {
        throw new CorpusProviderError('stale_reference', 'The text reference targets an older layer', 'window');
      }
    } else if (typeof reference === 'string') {
      const match = /unit-(\d+)$/.exec(reference);
      ordinal = match ? Math.floor((Number.parseInt(match[1], 10) - 1) % UNITS_PER_CHAPTER) + 1 : null;
    }
    if (!Number.isInteger(ordinal) || ordinal < 1 || ordinal > unitsForDocument(index)) {
      throw new CorpusProviderError('unavailable_reference', 'The text reference cannot be resolved by this fixture', 'window');
    }
    if (isRecord(reference) && Object.prototype.hasOwnProperty.call(reference, 'unitId') &&
      reference.unitId !== unitId(index, language, ordinal)) {
      throw new CorpusProviderError('invalid_request', 'The deep link unit does not belong to this version', 'window');
    }
    return ordinal - 1;
  }

  const provider = {
    async catalog(rawArgs = {}) {
      const args = validateOperationArgs('catalog', rawArgs);
      return run('catalog', args, async () => {
        const query = queryText(args.query);
        const cursor = decodePageCursor('catalog', args.cursor, {query});
        const items = [];
        let total = 0;
        for (let index = 0; index < documentCount; index += 1) {
          const id = workId(index);
          const title = `Fixture Work ${pad(index + 1)}`;
          if (query && !`${id} ${title}`.toLocaleLowerCase().includes(query)) continue;
          total += 1;
          if (total <= cursor.offset) continue;
          if (items.length < args.limit) items.push(catalogItem(index));
        }
        metricsState.maxPageItems = Math.max(metricsState.maxPageItems, items.length);
        const empty = pageShouldBeEmpty(emptyPageConfig, {operation: 'catalog', ...args, offset: cursor.offset, cursor});
        const visibleItems = empty ? [] : items;
        const nextOffset = cursor.offset + items.length;
        const normalNext = nextOffset < total ? cursorEncode({operation: 'catalog', offset: nextOffset, epoch, query}) : null;
        const nextCursor = empty && normalNext
          ? cursorEncode({operation: 'catalog', offset: cursor.offset, epoch, query, emptyPage: true})
          : normalNext;
        return {
          items: visibleItems,
          nextCursor,
          total: unknownTotal('catalog', args, cursor.offset) ? null : total,
        };
      });
    },

    async document(rawArgs = {}) {
      const args = validateOperationArgs('document', rawArgs);
      return run('document', args, async () => {
        const index = documentIndex(args.documentId);
        if (index === null) throw new CorpusProviderError('unavailable', 'The fixture document is unavailable', 'document');
        if (unavailableDocuments.has(args.documentId)) {
          throw new CorpusProviderError('unavailable', 'The fixture document is marked unavailable', 'document');
        }
        return manifest(index);
      });
    },

    async window(rawArgs = {}) {
      const args = {...rawArgs};
      const normalized = validateOperationArgs('window', args);
      return run('window', normalized, async () => {
        const index = documentIndex(normalized.documentId);
        if (index === null || unavailableDocuments.has(normalized.documentId)) {
          throw new CorpusProviderError('unavailable', 'The fixture document is unavailable', 'window');
        }
        const language = versionFor(index, normalized.versionId);
        if (language === null) throw new CorpusProviderError('unavailable', 'The fixture version is unavailable', 'window');
        const availability = versionAvailability(index, language);
        if (!availability.available) throw new CorpusProviderError('unavailable', 'The fixture version is not available for text reading', 'window');
        const revision = versionSource(index, language).textLayerSha256;
        if(normalized.revision && normalized.revision!==revision)throw new CorpusProviderError('stale_reference','The requested text layer has changed','window');
        const expected = {documentId: normalized.documentId, versionId: normalized.versionId, revision};
        const cursor = decodePageCursor('window', normalized.cursor, expected);
        const start = normalized.cursor === null ? resolveStart(index, language, normalized, revision) : cursor.offset;
        const total = unitsForDocument(index);
        const units = [];
        for (let ordinal = start + 1; ordinal <= Math.min(total, start + normalized.limit); ordinal += 1) {
          units.push(itemFor(index, language, ordinal));
        }
        metricsState.maxPageUnits = Math.max(metricsState.maxPageUnits, units.length);
        const empty = pageShouldBeEmpty(emptyPageConfig, {operation: 'window', ...normalized, offset: start, cursor});
        const visibleUnits = empty ? [] : units;
        const nextOffset = start + units.length;
        const previousOffset = Math.max(0, start - normalized.limit);
        const normalNext = nextOffset < total ? cursorEncode({operation: 'window', offset: nextOffset, epoch, ...expected}) : null;
        const nextCursor = empty && normalNext
          ? cursorEncode({operation: 'window', offset: start, epoch, ...expected, emptyPage: true})
          : normalNext;
        return {
          documentId: normalized.documentId,
          versionId: normalized.versionId,
          revision,
          units: visibleUnits,
          previousCursor: start > 0 ? cursorEncode({operation: 'window', offset: previousOffset, epoch, ...expected}) : null,
          nextCursor,
          total: unknownTotal('window', normalized, start) ? null : total,
        };
      });
    },

    async search(rawArgs = {}) {
      const args = validateOperationArgs('search', rawArgs);
      return run('search', args, async () => {
        if (args.scope !== 'document' && args.scope !== 'version') throw new CorpusProviderUnsupportedScopeError(args.scope);
        const index = documentIndex(args.documentId);
        const language = index === null ? null : versionFor(index, args.versionId);
        if (index === null || language === null || unavailableDocuments.has(args.documentId)) {
          throw new CorpusProviderError('unavailable', 'The fixture version is unavailable', 'search');
        }
        const availability = versionAvailability(index, language);
        if (!availability.available) throw new CorpusProviderError('unavailable', 'The fixture version is not available for search', 'search');
        const revision = versionSource(index, language).textLayerSha256;
        const query = queryText(args.query);
        const cursor = decodePageCursor('search', args.cursor, {
          documentId: args.documentId,
          versionId: args.versionId,
          revision,
          query,
          scope: args.scope,
        });
        const items = [];
        let total = 0;
        for (let ordinal = 1; ordinal <= unitsForDocument(index); ordinal += 1) {
          if (ordinal > 1 && (ordinal - 1) % 512 === 0) {
            await yieldToEventLoop(args.signal);
            throwIfAborted(args.signal);
          }
          const unit = itemFor(index, language, ordinal);
          if (!unit.text.toLocaleLowerCase().includes(query)) continue;
          total += 1;
          if (total <= cursor.offset) continue;
          if (items.length < args.limit) {
            items.push({unitId: unit.id, reference: unit.reference, excerpt: unit.text.slice(0, 180)});
          }
        }
        metricsState.maxPageItems = Math.max(metricsState.maxPageItems, items.length);
        const empty = pageShouldBeEmpty(emptyPageConfig, {operation: 'search', ...args, offset: cursor.offset, cursor});
        const visibleItems = empty ? [] : items;
        const nextOffset = cursor.offset + items.length;
        const normalNext = nextOffset < total
          ? cursorEncode({operation: 'search', offset: nextOffset, epoch, documentId: args.documentId, versionId: args.versionId, revision, query, scope: args.scope})
          : null;
        const nextCursor = empty && normalNext
          ? cursorEncode({operation: 'search', offset: cursor.offset, epoch, documentId: args.documentId, versionId: args.versionId, revision, query, scope: args.scope, emptyPage: true})
          : normalNext;
        return {
          items: visibleItems,
          nextCursor,
          total: unknownTotal('search', args, cursor.offset) ? null : total,
        };
      });
    },

    async structure(rawArgs = {}) {
      const args = validateOperationArgs('structure', rawArgs);
      return run('structure', args, async () => {
        const index = documentIndex(args.documentId);
        if (index === null || unavailableDocuments.has(args.documentId)) {
          throw new CorpusProviderError('unavailable', 'The fixture document is unavailable', 'structure');
        }
        const language = versionFor(index, args.versionId);
        if (language === null) throw new CorpusProviderError('unavailable', 'The fixture version is unavailable', 'structure');
        const availability = versionAvailability(index, language);
        if (!availability.available) throw new CorpusProviderError('unavailable', 'The fixture version is not available for structure', 'structure');
        const revision = versionSource(index, language).textLayerSha256;
        const expected = {documentId: args.documentId, versionId: args.versionId, revision};
        const cursor = decodePageCursor('structure', args.cursor, expected);
        const total = Math.ceil(unitsForDocument(index) / UNITS_PER_CHAPTER);
        const items = [];
        for (let chapter = cursor.offset + 1; chapter <= Math.min(total, cursor.offset + args.limit); chapter += 1) {
          items.push(structureItem(index, language, chapter));
        }
        metricsState.maxPageItems = Math.max(metricsState.maxPageItems, items.length);
        const empty = pageShouldBeEmpty(emptyPageConfig, {operation: 'structure', ...args, offset: cursor.offset, cursor});
        const visibleItems = empty ? [] : items;
        const nextOffset = cursor.offset + items.length;
        const normalNext = nextOffset < total
          ? cursorEncode({operation: 'structure', offset: nextOffset, epoch, ...expected})
          : null;
        const nextCursor = empty && normalNext
          ? cursorEncode({operation: 'structure', offset: cursor.offset, epoch, ...expected, emptyPage: true})
          : normalNext;
        return {
          documentId: args.documentId,
          versionId: args.versionId,
          revision,
          items: visibleItems,
          nextCursor,
          total: unknownTotal('structure', args, cursor.offset) ? null : total,
        };
      });
    },

    async resolve(rawArgs = {}) {
      const args = validateOperationArgs('resolve', rawArgs);
      return run('resolve', args, async () => {
        const reference = args.reference;
        const index = documentIndex(reference.target.workId);
        if (index === null || unavailableDocuments.has(reference.target.workId)) return {status: 'unavailable'};
        const language = versionFor(index, reference.versionId);
        if (language === null) return {status: 'unavailable'};
        if (!versionAvailability(index, language).available) return {status: 'unavailable'};
        const ordinal = ordinalFromUnitId(reference.unitId);
        if (ordinal === null || ordinal > unitsForDocument(index)) return {status: 'unavailable'};
        if (reference.unitId !== unitId(index, language, ordinal)) return {status: 'unavailable'};
        const currentSource = versionSource(index, language);
        const expected = createReference({
          source: currentSource,
          versionId: reference.versionId,
          unitId: reference.unitId,
          start: reference.selector.start,
          end: reference.selector.end,
        });
        if (changedReferences.has(referenceKey(reference))) return {status: 'changed', reference: expected};
        return sourceEquals(reference.target, currentSource) ? {status: 'exact', reference} : {status: 'changed', reference: expected};
      });
    },
  };

  function setRevision(revision, target = null) {
    assertDigest(revision, 'revision');
    if (target === null || target === undefined) {
      globalRevision = revision;
    } else if (typeof target === 'string') {
      for (const language of languages) revisionOverrides.set(`${target}:${language}`, revision);
    } else if (isRecord(target) && typeof target.documentId === 'string' && typeof target.versionId === 'string') {
      const index = documentIndex(target.documentId);
      const language = index === null ? null : versionFor(index, target.versionId);
      if (language === null) throw new TypeError('setRevision target version is unknown');
      revisionOverrides.set(`${target.documentId}:${language}`, revision);
    } else {
      throw new TypeError('setRevision target must be a document ID or {documentId, versionId}');
    }
    epoch += 1;
    return revision;
  }

  const controls = {
    setLatency(value) {
      if (!Number.isFinite(value) || value < 0) throw new TypeError('latency must be a non-negative number');
      latencyMs = value;
    },
    setRevision,
    bumpRevision(target = null) {
      const next = digest(`fixture:revision:${epoch + 1}:${target ?? 'all'}`);
      setRevision(next, target);
      return next;
    },
    setFailure(operation, failure = true) {
      if (!['catalog', 'document', 'window', 'search', 'resolve', 'structure'].includes(operation)) throw new TypeError('unknown fixture operation');
      if (!failure) failures.delete(operation);
      else failures.set(operation, failure);
    },
    clearFailure(operation) {
      failures.delete(operation);
    },
    setUnavailable(document, value = true) {
      if (value) unavailableDocuments.add(document);
      else unavailableDocuments.delete(document);
    },
    setResponseVariant(operation, variant = null) {
      if (!['catalog', 'document', 'window', 'search', 'resolve', 'structure'].includes(operation)) throw new TypeError('unknown fixture operation');
      if (variant === null || variant === false) responseVariants.delete(operation);
      else responseVariants.set(operation, variant);
    },
    setEmptyPages(value = false) {
      if (value !== false && !['boolean', 'function'].includes(typeof value) && !isRecord(value) && !Array.isArray(value)) {
        throw new TypeError('empty page configuration must be a boolean, array, object or function');
      }
      // This control is deliberately scoped to the test fixture. It changes
      // only synthetic continuation behaviour and never a production source.
      emptyPageConfig = value;
    },
    markChanged(reference) {
      changedReferences.add(referenceKey(reference));
    },
    clearChanged(reference) {
      changedReferences.delete(referenceKey(reference));
    },
    resetMetrics() {
      for (const operation of Object.keys(metricsState.requests)) metricsState.requests[operation] = 0;
      for (const key of ['failed', 'aborted', 'activeRequests', 'maxActiveRequests', 'materializedCatalogItems', 'materializedUnits', 'maxPageItems', 'maxPageUnits']) metricsState[key] = 0;
    },
  };

  function metrics() {
    return {
      version: 'tos.fixture.metrics.v1',
      logical: {documentCount, unitCount, unitsPerDocument, longDocumentUnits, totalUnits},
      requests: {...metricsState.requests},
      failed: metricsState.failed,
      aborted: metricsState.aborted,
      activeRequests: metricsState.activeRequests,
      maxActiveRequests: metricsState.maxActiveRequests,
      materializedCatalogItems: metricsState.materializedCatalogItems,
      materializedUnits: metricsState.materializedUnits,
      maxPageItems: metricsState.maxPageItems,
      maxPageUnits: metricsState.maxPageUnits,
    };
  }

  return Object.freeze({
    ...provider,
    controls,
    metrics,
    getMetrics: metrics,
    limits: CORPUS_LIMITS,
    capabilities: Object.freeze({catalog: true, document: true, window: true, searchScopes: ['version', 'document'], resolve: true, structure: true}),
  });
}
