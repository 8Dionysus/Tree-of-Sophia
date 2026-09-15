import {referenceKey, validateReference} from './model.mjs';

export const CORPUS_PROVIDER_VERSION = 'tos.corpus.provider.v1';
export const CORPUS_OPERATIONS = Object.freeze([
  'catalog',
  'document',
  'window',
  'search',
  'resolve',
]);
export const CORPUS_OPTIONAL_OPERATIONS = Object.freeze(['structure']);
export const CORPUS_ALL_OPERATIONS = Object.freeze([...CORPUS_OPERATIONS, ...CORPUS_OPTIONAL_OPERATIONS]);

export const CORPUS_LIMITS = Object.freeze({
  catalog: 100,
  window: 100,
  search: 100,
  structure: 100,
  maxVersions: 100,
  maxPageTextCharacters: 262_144,
  maxUnitTextCharacters: 65_536,
  // Character limits remain useful for predictable DOM work.  UTF-8 limits
  // additionally bound the bytes a transport can place in one response; a
  // multibyte script must not evade the page budget merely by having fewer
  // JavaScript UTF-16 code units.
  maxPageTextBytes: 262_144,
  maxUnitTextBytes: 65_536,
  maxMetadataCharacters: 16_384,
  maxPageMetadataCharacters: 131_072,
  maxMetadataBytes: 16_384,
  maxPageMetadataBytes: 131_072,
  queryCharacters: 256,
});

const encoder = typeof TextEncoder === 'function' ? new TextEncoder() : null;
function utf8Bytes(value) {
  const text = String(value);
  if (encoder) return encoder.encode(text).byteLength;
  // The fallback is only for old runtimes without TextEncoder.  It keeps the
  // same replacement-character behaviour for lone surrogates closely enough
  // for a defensive transport bound.
  try { return unescape(encodeURIComponent(text)).length; } catch { return text.length * 3; }
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function requiredString(value, name) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new CorpusProviderError('invalid_request', `${name} must be a non-empty string`);
  }
  return value;
}

function optionalString(value, name) {
  if (value === undefined || value === null) return null;
  return requiredString(value, name);
}

function nonNegativeInteger(value, name) {
  if (!Number.isInteger(value) || value < 0) {
    throw new CorpusProviderError('invalid_request', `${name} must be a non-negative integer`);
  }
  return value;
}

function pageLimit(value, operation) {
  const limit = value === undefined ? 25 : value;
  if (!Number.isInteger(limit) || limit < 1 || limit > CORPUS_LIMITS[operation]) {
    throw new CorpusProviderError(
      'invalid_request',
      `${operation}.limit must be an integer from 1 to ${CORPUS_LIMITS[operation]}`,
      operation,
    );
  }
  return limit;
}

function assertPage(value, operation) {
  if (!isRecord(value)) {
    throw new CorpusProviderError('invalid_response', `${operation} must return an object`, operation);
  }
  return value;
}

function assertArray(value, name, operation) {
  if (!Array.isArray(value)) {
    throw new CorpusProviderError('invalid_response', `${name} must be an array`, operation);
  }
  return value;
}

function assertCursor(value, name, operation) {
  if (value !== null && value !== undefined && typeof value !== 'string') {
    throw new CorpusProviderError('invalid_response', `${name} must be a string or null`, operation);
  }
  return value ?? null;
}

function responseString(value, name, operation, max = CORPUS_LIMITS.maxMetadataCharacters) {
  if (typeof value !== 'string' || value.length === 0 || value.length > max) {
    throw new CorpusProviderError('invalid_response', `${name} must be a bounded non-empty string`, operation);
  }
  return value;
}

function responseMetadata(value, name, operation, max = CORPUS_LIMITS.maxMetadataCharacters) {
  let encoded;
  try {
    encoded = JSON.stringify(value);
  } catch {
    throw new CorpusProviderError('invalid_response', `${name} must be serializable metadata`, operation);
  }
  if (typeof encoded !== 'string' || encoded.length > max || utf8Bytes(encoded) > Math.min(max, CORPUS_LIMITS.maxMetadataBytes)) {
    throw new CorpusProviderError('invalid_response', `${name} exceeds the metadata bound`, operation);
  }
  return encoded.length;
}

function responseCount(value, name, operation) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new CorpusProviderError('invalid_response', `${name} must be a non-negative safe integer`, operation);
  }
  return value;
}

function optionalResponseCount(value, name, operation) {
  if (value === undefined || value === null) return null;
  return responseCount(value, name, operation);
}

function responseText(value, name, operation) {
  const text = responseString(value, name, operation, CORPUS_LIMITS.maxUnitTextCharacters);
  if (utf8Bytes(text) > CORPUS_LIMITS.maxUnitTextBytes) {
    throw new CorpusProviderError('invalid_response', `${name} exceeds the UTF-8 text bound`, operation);
  }
  return text;
}

function availabilityStatus(value) {
  if (!isRecord(value)) return String(value ?? '').trim().toLowerCase();
  return String(value.status ?? value.availability ?? value.textAvailability ?? '').trim().toLowerCase();
}

function versionCarriesText(value) {
  if (!isRecord(value)) return true;
  if (value.available === false || value.textAvailable === false || value.text_available === false) return false;
  const status = availabilityStatus(value).replace(/-/g, '_');
  const visibility = String(value.visibility ?? '').trim().toLowerCase();
  return ![
    'unavailable', 'metadata_only', 'metadata', 'restricted', 'link_only',
    'public_metadata_only', 'local_only', 'missing', 'expired', 'text_unavailable',
    'withheld', 'unknown', 'pending', 'not_available', 'not_indexed', 'rights_unknown',
  ].includes(status) && !['restricted', 'public_metadata_only', 'local_only', 'unknown'].includes(visibility);
}

function validateOptionalSource(value, name, operation, expectedWorkId = null) {
  if (value === undefined || value === null) return null;
  return validateSourceMetadata(value, name, operation, expectedWorkId);
}

function responseDigest(value, name, operation) {
  const digest = responseString(value, name, operation, 128);
  if (!/^[a-f0-9]{64}$/.test(digest)) {
    throw new CorpusProviderError('invalid_response', `${name} must be a lowercase SHA-256 digest`, operation);
  }
  return digest;
}

function validateSourceMetadata(value, name, operation, expectedWorkId = null) {
  if (!isRecord(value)) {
    throw new CorpusProviderError('invalid_response', `${name} must be an object`, operation);
  }
  const source = {
    workId: responseString(value.workId, `${name}.workId`, operation),
    expressionId: responseString(value.expressionId, `${name}.expressionId`, operation),
    editionId: responseString(value.editionId, `${name}.editionId`, operation),
    itemId: responseString(value.itemId, `${name}.itemId`, operation),
    fileId: responseString(value.fileId, `${name}.fileId`, operation),
    fileSha256: responseDigest(value.fileSha256, `${name}.fileSha256`, operation),
    textLayerRef: responseString(value.textLayerRef, `${name}.textLayerRef`, operation),
    textLayerSha256: responseDigest(value.textLayerSha256, `${name}.textLayerSha256`, operation),
  };
  if (expectedWorkId !== null && source.workId !== expectedWorkId) {
    throw new CorpusProviderError('invalid_response', `${name}.workId does not match the requested document`, operation);
  }
  responseMetadata(value, name, operation);
  return source;
}

function validateCatalogPage(value, args = {}) {
  const page = assertPage(value, 'catalog');
  const items = assertArray(page.items, 'catalog.items', 'catalog');
  if (items.length > args.limit || items.length > CORPUS_LIMITS.catalog) {
    throw new CorpusProviderError('invalid_response', 'catalog.items exceeds the requested page bound', 'catalog');
  }
  const nextCursor = assertCursor(page.nextCursor, 'catalog.nextCursor', 'catalog');
  const total = optionalResponseCount(page.total, 'catalog.total', 'catalog');
  let metadataCharacters = 0;
  for (const item of items) {
    if (!isRecord(item)) throw new CorpusProviderError('invalid_response', 'catalog.items must contain objects', 'catalog');
    metadataCharacters += responseString(item.id, 'catalog.items[].id', 'catalog').length;
    metadataCharacters += responseString(item.title, 'catalog.items[].title', 'catalog').length;
    if (Array.isArray(item.languages)) {
      for (const language of item.languages) metadataCharacters += responseString(language, 'catalog.items[].languages[]', 'catalog', 64).length;
    }
    if (item.unitCount !== undefined && item.unitCount !== null && (!Number.isSafeInteger(item.unitCount) || item.unitCount < 0)) {
      throw new CorpusProviderError('invalid_response', 'catalog.items[].unitCount must be non-negative or null when unknown', 'catalog');
    }
    metadataCharacters += responseMetadata(item, 'catalog.items[]', 'catalog');
  }
  if (metadataCharacters > CORPUS_LIMITS.maxPageMetadataCharacters || utf8Bytes(JSON.stringify(items)) > CORPUS_LIMITS.maxPageMetadataBytes) {
    throw new CorpusProviderError('invalid_response', 'catalog metadata exceeds the page bound', 'catalog');
  }
  return {
    ...page,
    items: items.map(item => ({
      ...item,
      unitCount: optionalResponseCount(item.unitCount, 'catalog.items[].unitCount', 'catalog'),
    })),
    nextCursor,
    total,
  };
}

function validateDocumentPage(value, args = {}) {
  const page = assertPage(value, 'document');
  responseString(page.id, 'document.id', 'document');
  if (args.documentId !== undefined && page.id !== args.documentId) {
    throw new CorpusProviderError('invalid_response', 'document.id does not match the requested document', 'document');
  }
  responseString(page.title, 'document.title', 'document');
  if (!Array.isArray(page.versions) || page.versions.length === 0) {
    throw new CorpusProviderError('invalid_response', 'document.versions must be a non-empty array', 'document');
  }
  if (page.versions.length > CORPUS_LIMITS.maxVersions) {
    throw new CorpusProviderError('invalid_response', 'document.versions exceeds the metadata bound', 'document');
  }
  let metadataCharacters = responseMetadata({id: page.id, title: page.title}, 'document metadata', 'document');
  const versionIds = new Set();
  const validatedSources = new Map();
  for (const version of page.versions) {
    if (!isRecord(version)) throw new CorpusProviderError('invalid_response', 'document.versions must contain objects', 'document');
    responseString(version.id, 'document.versions[].id', 'document');
    if (versionIds.has(version.id)) {
      throw new CorpusProviderError('invalid_response', 'document.versions contains duplicate IDs', 'document');
    }
    versionIds.add(version.id);
    responseString(version.language, 'document.versions[].language', 'document', 64);
    responseString(version.role, 'document.versions[].role', 'document', 64);
    const carriesText = versionCarriesText(version);
    if (version.revision !== undefined && version.revision !== null) responseString(version.revision, 'document.versions[].revision', 'document', 128);
    else if (carriesText) {
      throw new CorpusProviderError('invalid_response', 'document.versions[].revision is required for an available version', 'document');
    }
    if (version.unitCount !== undefined && version.unitCount !== null && (!Number.isSafeInteger(version.unitCount) || version.unitCount < 0)) {
      throw new CorpusProviderError('invalid_response', 'document.versions[].unitCount must be non-negative or null when unknown', 'document');
    }
    if (carriesText && (version.source === undefined || version.source === null)) {
      throw new CorpusProviderError('invalid_response', 'document.versions[].source is required for an available version', 'document');
    }
    const source = validateOptionalSource(version.source, 'document.versions[].source', 'document', page.id);
    validatedSources.set(version.id, source);
    metadataCharacters += responseMetadata(version, 'document.versions[]', 'document');
  }
  if (metadataCharacters > CORPUS_LIMITS.maxPageMetadataCharacters || utf8Bytes(JSON.stringify(page.versions)) > CORPUS_LIMITS.maxPageMetadataBytes) {
    throw new CorpusProviderError('invalid_response', 'document metadata exceeds the page bound', 'document');
  }
  return {
    ...page,
    unitCount: optionalResponseCount(page.unitCount, 'document.unitCount', 'document'),
    versions: page.versions.map(version => ({
      ...version,
      revision: version.revision ?? null,
      source: validatedSources.get(version.id),
      unitCount: optionalResponseCount(version.unitCount, 'document.versions[].unitCount', 'document'),
    })),
  };
}

function expectedWindowRevision(args = {}) {
  if (typeof args.revision === 'string') return args.revision;
  const reference = args.reference;
  if (!isRecord(reference)) return null;
  if (typeof reference.revision === 'string') return reference.revision;
  if (reference.schemaVersion && isRecord(reference.target) && typeof reference.target.textLayerSha256 === 'string') {
    return reference.target.textLayerSha256;
  }
  return null;
}

function validateWindowPage(value, args = {}) {
  const page = assertPage(value, 'window');
  responseString(page.documentId, 'window.documentId', 'window');
  responseString(page.versionId, 'window.versionId', 'window');
  if (page.documentId !== args.documentId || page.versionId !== args.versionId) {
    throw new CorpusProviderError('invalid_response', 'window identity does not match the requested document/version', 'window');
  }
  responseString(page.revision, 'window.revision', 'window', 128);
  const expectedRevision = expectedWindowRevision(args);
  if (expectedRevision !== null && page.revision !== expectedRevision) {
    throw new CorpusProviderError('invalid_response', 'window.revision does not match the requested text layer', 'window');
  }
  const units = assertArray(page.units, 'window.units', 'window');
  if (units.length > args.limit || units.length > CORPUS_LIMITS.window) {
    throw new CorpusProviderError('invalid_response', 'window.units exceeds the requested page bound', 'window');
  }
  const previousCursor = assertCursor(page.previousCursor, 'window.previousCursor', 'window');
  const nextCursor = assertCursor(page.nextCursor, 'window.nextCursor', 'window');
  const total = optionalResponseCount(page.total, 'window.total', 'window');
  let textCharacters = 0;
  let textBytes = 0;
  let metadataCharacters = responseMetadata({documentId: page.documentId, versionId: page.versionId, revision: page.revision}, 'window metadata', 'window');
  const unitIds = new Set();
  const ordinals = new Set();
  for (const unit of units) {
    if (!isRecord(unit)) throw new CorpusProviderError('invalid_response', 'window.units must contain objects', 'window');
    responseString(unit.id, 'window.units[].id', 'window');
    if (unitIds.has(unit.id)) {
      throw new CorpusProviderError('invalid_response', 'window.units contains duplicate IDs', 'window');
    }
    unitIds.add(unit.id);
    if (!Number.isInteger(unit.ordinal) || unit.ordinal < 1) {
      throw new CorpusProviderError('invalid_response', 'window.units[].ordinal must be positive', 'window');
    }
    if (ordinals.has(unit.ordinal)) {
      throw new CorpusProviderError('invalid_response', 'window.units contains duplicate ordinals', 'window');
    }
    ordinals.add(unit.ordinal);
    responseString(unit.kind, 'window.units[].kind', 'window', 128);
    responseText(unit.text, 'window.units[].text', 'window');
    responseString(unit.language, 'window.units[].language', 'window', 64);
    responseString(unit.reference, 'window.units[].reference', 'window');
    textCharacters += unit.text.length;
    textBytes += utf8Bytes(unit.text);
    metadataCharacters += responseMetadata({...unit, text: undefined}, 'window.units[] metadata', 'window');
  }
  if (textCharacters > CORPUS_LIMITS.maxPageTextCharacters || textBytes > CORPUS_LIMITS.maxPageTextBytes) {
    throw new CorpusProviderError('invalid_response', 'window text exceeds the page bound', 'window');
  }
  if (metadataCharacters > CORPUS_LIMITS.maxPageMetadataCharacters || utf8Bytes(JSON.stringify(units.map(unit => ({...unit, text: undefined})))) > CORPUS_LIMITS.maxPageMetadataBytes) {
    throw new CorpusProviderError('invalid_response', 'window metadata exceeds the page bound', 'window');
  }
  return {...page, previousCursor, nextCursor, total};
}

function validateSearchPage(value, args = {}) {
  const page = assertPage(value, 'search');
  const items = assertArray(page.items, 'search.items', 'search');
  if (items.length > args.limit || items.length > CORPUS_LIMITS.search) {
    throw new CorpusProviderError('invalid_response', 'search.items exceeds the requested page bound', 'search');
  }
  const nextCursor = assertCursor(page.nextCursor, 'search.nextCursor', 'search');
  const total = optionalResponseCount(page.total, 'search.total', 'search');
  let metadataCharacters = 0;
  for (const item of items) {
    if (!isRecord(item)) throw new CorpusProviderError('invalid_response', 'search.items must contain objects', 'search');
    responseString(item.unitId, 'search.items[].unitId', 'search');
    responseString(item.reference, 'search.items[].reference', 'search');
    responseString(item.excerpt, 'search.items[].excerpt', 'search', CORPUS_LIMITS.maxUnitTextCharacters);
    metadataCharacters += responseMetadata(item, 'search.items[]', 'search');
  }
  if (metadataCharacters > CORPUS_LIMITS.maxPageMetadataCharacters || utf8Bytes(JSON.stringify(items)) > CORPUS_LIMITS.maxPageMetadataBytes) {
    throw new CorpusProviderError('invalid_response', 'search metadata exceeds the page bound', 'search');
  }
  return {...page, nextCursor, total};
}

function validateStructurePage(value, args = {}) {
  const page = assertPage(value, 'structure');
  responseString(page.documentId, 'structure.documentId', 'structure');
  responseString(page.versionId, 'structure.versionId', 'structure');
  if (page.documentId !== args.documentId || page.versionId !== args.versionId) {
    throw new CorpusProviderError('invalid_response', 'structure identity does not match the requested document/version', 'structure');
  }
  responseString(page.revision, 'structure.revision', 'structure', 128);
  const items = assertArray(page.items, 'structure.items', 'structure');
  if (items.length > args.limit || items.length > CORPUS_LIMITS.structure) {
    throw new CorpusProviderError('invalid_response', 'structure.items exceeds the requested page bound', 'structure');
  }
  const nextCursor = assertCursor(page.nextCursor, 'structure.nextCursor', 'structure');
  const total = optionalResponseCount(page.total, 'structure.total', 'structure');
  let metadataCharacters = responseMetadata({documentId: page.documentId, versionId: page.versionId, revision: page.revision}, 'structure metadata', 'structure');
  const itemIds = new Set();
  const unitIds = new Set();
  for (const item of items) {
    if (!isRecord(item)) throw new CorpusProviderError('invalid_response', 'structure.items must contain objects', 'structure');
    responseString(item.id, 'structure.items[].id', 'structure');
    if (itemIds.has(item.id)) {
      throw new CorpusProviderError('invalid_response', 'structure.items contains duplicate IDs', 'structure');
    }
    itemIds.add(item.id);
    responseString(item.label, 'structure.items[].label', 'structure');
    responseString(item.unitId, 'structure.items[].unitId', 'structure');
    if (unitIds.has(item.unitId)) {
      throw new CorpusProviderError('invalid_response', 'structure.items contains duplicate unit IDs', 'structure');
    }
    unitIds.add(item.unitId);
    if (!Number.isSafeInteger(item.level) || item.level < 0) {
      throw new CorpusProviderError('invalid_response', 'structure.items[].level must be a non-negative safe integer', 'structure');
    }
    if (item.parentId !== undefined && item.parentId !== null) responseString(item.parentId, 'structure.items[].parentId', 'structure');
    if (item.hasChildren !== undefined && typeof item.hasChildren !== 'boolean') {
      throw new CorpusProviderError('invalid_response', 'structure.items[].hasChildren must be boolean', 'structure');
    }
    metadataCharacters += responseMetadata(item, 'structure.items[]', 'structure');
  }
  if (metadataCharacters > CORPUS_LIMITS.maxPageMetadataCharacters || utf8Bytes(JSON.stringify(items)) > CORPUS_LIMITS.maxPageMetadataBytes) {
    throw new CorpusProviderError('invalid_response', 'structure metadata exceeds the page bound', 'structure');
  }
  return {...page, nextCursor, total};
}

function validateResolvePage(value, args = {}) {
  const page = assertPage(value, 'resolve');
  if (!['exact', 'changed', 'unavailable'].includes(page.status)) {
    throw new CorpusProviderError('invalid_response', 'resolve.status is invalid', 'resolve');
  }
  if (page.reference !== undefined && page.reference !== null) {
    let reference;
    try {
      reference = validateReference(page.reference);
    } catch {
      throw new CorpusProviderError('invalid_response', 'resolve.reference is not a valid corpus reference', 'resolve');
    }
    if (page.status === 'exact' && args.reference !== undefined && referenceKey(reference) !== referenceKey(args.reference)) {
      throw new CorpusProviderError('invalid_response', 'resolve.exact reference does not match the requested reference', 'resolve');
    }
  }
  return page;
}

const VALIDATORS = Object.freeze({
  catalog: validateCatalogPage,
  document: validateDocumentPage,
  window: validateWindowPage,
  search: validateSearchPage,
  resolve: validateResolvePage,
  structure: validateStructurePage,
});

export class CorpusProviderError extends Error {
  constructor(code, message, operation = null, details = undefined) {
    super(message);
    this.name = 'CorpusProviderError';
    this.code = code;
    this.operation = operation;
    if (details !== undefined) this.details = details;
  }
}

export class CorpusProviderUnavailableError extends CorpusProviderError {
  constructor(message = 'No corpus provider is configured') {
    super('provider_unavailable', message);
    this.name = 'CorpusProviderUnavailableError';
  }
}

export class CorpusProviderCursorError extends CorpusProviderError {
  constructor(message = 'The cursor is invalid or stale', details = undefined) {
    super('stale_cursor', message, null, details);
    this.name = 'CorpusProviderCursorError';
  }
}

export class CorpusProviderUnsupportedScopeError extends CorpusProviderError {
  constructor(scope) {
    super('unsupported_scope', `The corpus provider does not support search scope ${scope}`, 'search', {scope});
    this.name = 'CorpusProviderUnsupportedScopeError';
  }
}

function abortError(signal) {
  if (signal?.reason instanceof Error) return signal.reason;
  const error = new Error('The corpus request was aborted');
  error.name = 'AbortError';
  return error;
}

export function throwIfAborted(signal) {
  if (signal?.aborted) throw abortError(signal);
}

export function assertCorpusProvider(provider) {
  if (!isRecord(provider)) throw new TypeError('provider must be an object');
  for (const operation of CORPUS_OPERATIONS) {
    if (typeof provider[operation] !== 'function') {
      throw new TypeError(`provider.${operation} must be a function`);
    }
  }
  if (provider.structure !== undefined && typeof provider.structure !== 'function') {
    throw new TypeError('provider.structure must be a function when supplied');
  }
  return provider;
}

/**
 * Add request/response guards around an explicitly supplied provider. No
 * network route or transport convention is inferred here.
 */
export function bindCorpusProvider(provider) {
  assertCorpusProvider(provider);
  const bound = {};
  for (const operation of CORPUS_OPERATIONS) {
    bound[operation] = async (args = {}) => {
      const normalized = validateOperationArgs(operation, args);
      const result = await provider[operation](normalized);
      return VALIDATORS[operation](result, normalized);
    };
  }
  if (typeof provider.structure === 'function') {
    bound.structure = async (args = {}) => {
      const normalized = validateOperationArgs('structure', args);
      const result = await provider.structure(normalized);
      return VALIDATORS.structure(result, normalized);
    };
  }
  const capabilities = isRecord(provider.capabilities) ? {...provider.capabilities} : {};
  if (typeof provider.structure === 'function') capabilities.structure = true;
  else delete capabilities.structure;
  if (Object.keys(capabilities).length > 0) bound.capabilities = Object.freeze(capabilities);
  return Object.freeze(bound);
}

/**
 * Adapt a caller-owned transport. The transport chooses how operation names
 * become HTTP, IPC, or another request; this module never invents a route.
 */
export function createTransportProvider({request, structure = null, capabilities = null} = {}) {
  if (typeof request !== 'function') {
    throw new TypeError('createTransportProvider requires caller-supplied request(operation, args)');
  }
  if (structure !== null && typeof structure !== 'function') {
    throw new TypeError('createTransportProvider.structure must be a function when supplied');
  }
  if (capabilities !== null && !isRecord(capabilities)) {
    throw new TypeError('createTransportProvider.capabilities must be an object when supplied');
  }
  const provider = {};
  for (const operation of CORPUS_OPERATIONS) {
    provider[operation] = (args = {}) => request(operation, args);
  }
  if (structure !== null) provider.structure = structure;
  if (capabilities !== null) provider.capabilities = capabilities;
  return bindCorpusProvider(provider);
}

export function createUnavailableProvider(reason = 'The real corpus provider is not connected yet') {
  const fail = async () => {
    throw new CorpusProviderUnavailableError(reason);
  };
  return Object.freeze({
    ...Object.fromEntries(CORPUS_OPERATIONS.map(operation => [operation, fail])),
    capabilities: Object.freeze({
      catalog: false,
      document: false,
      window: false,
      search: false,
      resolve: false,
      structure: false,
      searchScopes: Object.freeze([]),
    }),
  });
}

export function createCorpusProvider({provider = null, transport = null} = {}) {
  if (provider !== null && transport !== null) {
    throw new TypeError('provide either provider or transport, not both');
  }
  if (provider !== null) return bindCorpusProvider(provider);
  if (transport !== null) return createTransportProvider(transport);
  return createUnavailableProvider();
}

export const createProvider = createCorpusProvider;

export function validateOperationArgs(operation, args = {}) {
  if (!CORPUS_ALL_OPERATIONS.includes(operation)) throw new TypeError(`unknown corpus operation ${operation}`);
  if (!isRecord(args)) throw new CorpusProviderError('invalid_request', `${operation} arguments must be an object`, operation);
  const normalized = {...args};
  if (operation === 'catalog') {
    if (args.query === undefined) normalized.query = '';
    else if (typeof args.query === 'string') normalized.query = args.query;
    else throw new CorpusProviderError('invalid_request', 'catalog.query must be a string', operation);
    normalized.cursor = optionalString(args.cursor, 'catalog.cursor');
    normalized.limit = pageLimit(args.limit, operation);
  } else if (operation === 'document') {
    normalized.documentId = requiredString(args.documentId, 'document.documentId');
  } else if (operation === 'window') {
    normalized.documentId = requiredString(args.documentId, 'window.documentId');
    normalized.versionId = requiredString(args.versionId, 'window.versionId');
    if (args.unitId !== undefined && args.unitId !== null) normalized.unitId = requiredString(args.unitId, 'window.unitId');
    if (args.revision !== undefined && args.revision !== null) normalized.revision = requiredString(args.revision, 'window.revision');
    normalized.cursor = optionalString(args.cursor, 'window.cursor');
    normalized.limit = pageLimit(args.limit, operation);
  } else if (operation === 'search') {
    normalized.documentId = requiredString(args.documentId, 'search.documentId');
    normalized.versionId = requiredString(args.versionId, 'search.versionId');
    normalized.query = requiredString(args.query, 'search.query');
    if (normalized.query.length > CORPUS_LIMITS.queryCharacters) {
      throw new CorpusProviderError('invalid_request', `search.query is limited to ${CORPUS_LIMITS.queryCharacters} characters`, operation);
    }
    normalized.scope = args.scope === undefined ? 'document' : requiredString(args.scope, 'search.scope');
    normalized.cursor = optionalString(args.cursor, 'search.cursor');
    normalized.limit = pageLimit(args.limit, operation);
  } else if (operation === 'structure') {
    normalized.documentId = requiredString(args.documentId, 'structure.documentId');
    normalized.versionId = requiredString(args.versionId, 'structure.versionId');
    normalized.cursor = optionalString(args.cursor, 'structure.cursor');
    normalized.limit = pageLimit(args.limit, operation);
  } else if (operation === 'resolve') {
    normalized.reference = validateReference(args.reference);
  }
  return normalized;
}

export {validateCatalogPage, validateDocumentPage, validateWindowPage, validateSearchPage, validateResolvePage, validateStructurePage};
