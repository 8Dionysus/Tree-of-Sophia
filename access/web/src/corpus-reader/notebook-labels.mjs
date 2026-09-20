import {NATIVE_REFERENCE_SCHEMA} from './native-reference.mjs';

export const NOTEBOOK_LABEL_LIMITS = Object.freeze({
  concurrent: 4,
  cache: 128,
});

const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value);

/**
 * Return the corpus work id carried by a notebook reference.
 * Native references have a separate identity and are handled by the native
 * reader callback, so they must never be sent to the corpus document API.
 */
export function notebookReferenceWorkId(reference) {
  if (!isRecord(reference) || reference.schemaVersion === NATIVE_REFERENCE_SCHEMA) return null;
  const value = reference.target?.workId;
  return typeof value === 'string' && value.length > 0 ? value : null;
}

export function notebookFallbackLabel(record, {index = 0, locale = 'ru', passage = 'Passage', rangeLabel = 'characters'} = {}) {
  const selector = record?.reference?.selector;
  const hasRange = Number.isSafeInteger(selector?.start) && Number.isSafeInteger(selector?.end) && selector.end > selector.start;
  const ordinal = Math.max(0, Number.isSafeInteger(index) ? index : 0) + 1;
  const details = [String(ordinal)];
  if (hasRange) details.push(`${selector.start}–${selector.end} ${rangeLabel}`);
  const timestamp = typeof record?.createdAt === 'string' ? Date.parse(record.createdAt) : NaN;
  if (Number.isFinite(timestamp)) {
    try { details.push(new Intl.DateTimeFormat(locale, {dateStyle: 'medium'}).format(new Date(timestamp))); } catch {}
  }
  return `${passage} · ${details.join(' · ')}`;
}

/**
 * Resolve notebook document metadata without changing the active reader.
 * Requests are deduplicated, concurrency-limited, and retained in a small
 * LRU cache. A failed lookup resolves to null so labels can keep their
 * generic, non-identifying fallback without surfacing an unhandled rejection.
 */
export function createNotebookMetadataResolver({
  readDocument,
  maxConcurrent = NOTEBOOK_LABEL_LIMITS.concurrent,
  maxEntries = NOTEBOOK_LABEL_LIMITS.cache,
} = {}) {
  if (typeof readDocument !== 'function') throw new TypeError('read-document-required');
  const concurrency = Number.isSafeInteger(maxConcurrent) && maxConcurrent > 0 ? maxConcurrent : NOTEBOOK_LABEL_LIMITS.concurrent;
  const capacity = Number.isSafeInteger(maxEntries) && maxEntries > 0 ? maxEntries : NOTEBOOK_LABEL_LIMITS.cache;
  const cache = new Map();
  const pending = new Map();
  const queue = [];
  let active = 0;
  let disposed = false;

  function remember(documentId, document) {
    cache.delete(documentId);
    cache.set(documentId, document);
    while (cache.size > capacity) cache.delete(cache.keys().next().value);
  }

  function pump() {
    while (!disposed && active < concurrency && queue.length) {
      const request = queue.shift();
      active += 1;
      Promise.resolve()
        .then(() => readDocument(request.documentId))
        .then(document => {
          const resolved = document?.id === request.documentId ? document : null;
          if (!disposed) remember(request.documentId, resolved);
          if (!request.settled) {
            request.settled = true;
            request.resolve(resolved);
          }
        }, () => {
          if (!disposed) remember(request.documentId, null);
          if (!request.settled) {
            request.settled = true;
            request.resolve(null);
          }
        })
        .finally(() => {
          active -= 1;
          if (pending.get(request.documentId) === request) pending.delete(request.documentId);
          pump();
        });
    }
  }

  function resolve(documentId) {
    if (disposed || typeof documentId !== 'string' || !documentId.length) return Promise.resolve(null);
    if (cache.has(documentId)) {
      const value = cache.get(documentId);
      cache.delete(documentId);
      cache.set(documentId, value);
      return Promise.resolve(value);
    }
    const existing = pending.get(documentId);
    if (existing) return existing.promise;
    let resolvePromise;
    const promise = new Promise(resolveValue => { resolvePromise = resolveValue; });
    const request = {documentId, promise, resolve: resolvePromise, settled: false};
    queue.push(request);
    pending.set(documentId, request);
    pump();
    return promise;
  }

  return {
    resolve,
    dispose() {
      disposed = true;
      for (const request of pending.values()) {
        if (request.settled) continue;
        request.settled = true;
        request.resolve(null);
      }
      queue.length = 0;
      pending.clear();
    },
    clear() {
      cache.clear();
    },
    stats() {
      return {active, queued: queue.length, pending: pending.size, cache: cache.size, disposed};
    },
  };
}
