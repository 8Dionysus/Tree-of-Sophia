/**
 * Small, source-oriented values shared by the corpus reader provider and UI.
 *
 * A reference identifies a representation of a source text and a position in
 * it.  It deliberately contains no delivered wording and no graph revision;
 * those belong to the provider response and to the graph reader separately.
 */

import {NATIVE_REFERENCE_SCHEMA,validateNativeReference,nativeReferenceKey,nativeReferenceDocumentId,nativeReferenceVersionId} from './native-reference.mjs';

export const CORPUS_REFERENCE_SCHEMA = 'tos.corpus.reader.reference.v1';
export const TEXT_SELECTOR_TYPE = 'text_position';
export const TEXT_POSITION_UNIT = 'unicode_code_point';
export const TEXT_INTERVAL = 'half_open';

const SHA256 = /^[a-f0-9]{64}$/;

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function requiredString(value, name) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new TypeError(`${name} must be a non-empty string`);
  }
  return value;
}

function boundedDigest(value, name) {
  requiredString(value, name);
  if (!SHA256.test(value)) {
    throw new TypeError(`${name} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function nonNegativeInteger(value, name) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${name} must be a non-negative integer`);
  }
  return value;
}

function canonicalTarget(target) {
  if (!isRecord(target)) {
    throw new TypeError('reference.target must be an object');
  }
  return {
    workId: requiredString(target.workId, 'reference.target.workId'),
    expressionId: requiredString(target.expressionId, 'reference.target.expressionId'),
    editionId: requiredString(target.editionId, 'reference.target.editionId'),
    itemId: requiredString(target.itemId, 'reference.target.itemId'),
    fileId: requiredString(target.fileId, 'reference.target.fileId'),
    fileSha256: boundedDigest(target.fileSha256, 'reference.target.fileSha256'),
    textLayerRef: requiredString(target.textLayerRef, 'reference.target.textLayerRef'),
    textLayerSha256: boundedDigest(target.textLayerSha256, 'reference.target.textLayerSha256'),
  };
}

function canonicalSelector(selector) {
  if (!isRecord(selector)) {
    throw new TypeError('reference.selector must be an object');
  }
  if (selector.type !== TEXT_SELECTOR_TYPE) {
    throw new TypeError(`reference.selector.type must be ${TEXT_SELECTOR_TYPE}`);
  }
  if (selector.positionUnit !== TEXT_POSITION_UNIT) {
    throw new TypeError(`reference.selector.positionUnit must be ${TEXT_POSITION_UNIT}`);
  }
  if (selector.interval !== TEXT_INTERVAL) {
    throw new TypeError(`reference.selector.interval must be ${TEXT_INTERVAL}`);
  }
  const start = nonNegativeInteger(selector.start, 'reference.selector.start');
  const end = nonNegativeInteger(selector.end, 'reference.selector.end');
  if (end < start) {
    throw new RangeError('reference.selector.end must not precede start');
  }
  return {
    type: TEXT_SELECTOR_TYPE,
    start,
    end,
    positionUnit: TEXT_POSITION_UNIT,
    interval: TEXT_INTERVAL,
  };
}

/**
 * Validate and normalize a durable text reference.
 *
 * The returned object is intentionally a projection of identity fields only;
 * arbitrary fields such as `text`, `excerpt`, or a graph snapshot cannot
 * accidentally become part of a saved reference.
 */
export function validateReference(value) {
  if(value?.schemaVersion===NATIVE_REFERENCE_SCHEMA)return validateNativeReference(value);
  if (!isRecord(value)) {
    throw new TypeError('corpus reference must be an object');
  }
  if (value.schemaVersion !== CORPUS_REFERENCE_SCHEMA) {
    throw new TypeError(`corpus reference schemaVersion must be ${CORPUS_REFERENCE_SCHEMA}`);
  }
  return {
    schemaVersion: CORPUS_REFERENCE_SCHEMA,
    target: canonicalTarget(value.target),
    versionId: requiredString(value.versionId, 'reference.versionId'),
    unitId: requiredString(value.unitId, 'reference.unitId'),
    selector: canonicalSelector(value.selector),
  };
}

/**
 * Construct a reference from provider source metadata and a unit selector.
 * This is useful to UI code when a user selects text in a delivered unit.
 */
export function createReference({source, versionId, unitId, start = 0, end = 0}) {
  if (!isRecord(source)) {
    throw new TypeError('source must be an object');
  }
  return validateReference({
    schemaVersion: CORPUS_REFERENCE_SCHEMA,
    target: {
      workId: source.workId,
      expressionId: source.expressionId,
      editionId: source.editionId,
      itemId: source.itemId,
      fileId: source.fileId,
      fileSha256: source.fileSha256,
      textLayerRef: source.textLayerRef,
      textLayerSha256: source.textLayerSha256,
    },
    versionId,
    unitId,
    selector: {
      type: TEXT_SELECTOR_TYPE,
      start,
      end,
      positionUnit: TEXT_POSITION_UNIT,
      interval: TEXT_INTERVAL,
    },
  });
}

/**
 * Stable key for local stores, URL state, and provider-side resolve calls.
 * Fixed field order avoids depending on object insertion order supplied by a
 * transport adapter.
 */
export function referenceKey(value) {
  if(value?.schemaVersion===NATIVE_REFERENCE_SCHEMA)return nativeReferenceKey(value);
  const reference = validateReference(value);
  const target = reference.target;
  const selector = reference.selector;
  return JSON.stringify([
    reference.schemaVersion,
    target.workId,
    target.expressionId,
    target.editionId,
    target.itemId,
    target.fileId,
    target.fileSha256,
    target.textLayerRef,
    target.textLayerSha256,
    reference.versionId,
    reference.unitId,
    selector.type,
    selector.start,
    selector.end,
    selector.positionUnit,
    selector.interval,
  ]);
}

export function isReference(value) {
  try {
    validateReference(value);
    return true;
  } catch {
    return false;
  }
}

export function referenceDocumentId(value){
  return value?.schemaVersion===NATIVE_REFERENCE_SCHEMA?nativeReferenceDocumentId(value):validateReference(value).target.workId;
}
export function referenceVersionId(value){
  return value?.schemaVersion===NATIVE_REFERENCE_SCHEMA?nativeReferenceVersionId(value):validateReference(value).versionId;
}
