import {referenceKey, validateReference} from '../corpus-reader/model.mjs';

export const RESEARCH_SHELF_SOURCE_PREFIX = 'research-shelf:';

const canonicalReference = value => {
  const reference = validateReference(value);
  return {reference, key: referenceKey(reference)};
};

async function recordForReference(shelf, key) {
  let cursor = null;
  do {
    const page = await shelf.list({type: 'text', limit: 100, ...(cursor ? {cursor} : {})});
    const match = page.items.find(record => {
      try { return referenceKey(validateReference(record.target?.reference)) === key; } catch { return false; }
    });
    if (match) return match;
    cursor = page.nextCursor;
  } while (cursor);
  return null;
}

async function stableRecordId(key) {
  const digest = await globalThis.crypto.subtle.digest('SHA-256', new TextEncoder().encode(key));
  return `source-${[...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

/** Store the exact typed reference in the shelf and return its bounded owner address. */
export async function ensureSourceRecord(shelf, value, title = '') {
  if (!shelf || typeof shelf.get !== 'function' || typeof shelf.save !== 'function' || typeof shelf.list !== 'function') {
    throw new TypeError('The research shelf owner is unavailable.');
  }
  const checked = canonicalReference(value);
  const found = await recordForReference(shelf, checked.key);
  if (found) return `${RESEARCH_SHELF_SOURCE_PREFIX}${found.id}`;
  const id = await stableRecordId(checked.key);
  const existing = await shelf.get(id);
  if (existing) {
    try {
      if (referenceKey(validateReference(existing.target?.reference)) === checked.key) return `${RESEARCH_SHELF_SOURCE_PREFIX}${id}`;
    } catch { /* report an ID collision below */ }
    throw new Error(`The research shelf source address ${id} is occupied by a different record.`);
  }
  await shelf.save({id, type: 'text', title: String(title || checked.reference.unitId).slice(0, 256),
    target: {reference: checked.reference}, collectionIds: []});
  return `${RESEARCH_SHELF_SOURCE_PREFIX}${id}`;
}

/** Resolve compact shelf addresses and legacy canonical JSON without retargeting. */
export async function resolveSourceLocator(locator, shelf, provider) {
  if (typeof locator !== 'string' || locator.length > 1_024) return {status: 'invalid', reference: null};
  let reference;
  let recordId = null;
  if (locator.startsWith(RESEARCH_SHELF_SOURCE_PREFIX)) {
    recordId = locator.slice(RESEARCH_SHELF_SOURCE_PREFIX.length);
    if (!recordId || recordId.length > 256) return {status: 'invalid', reference: null};
    const record = await shelf.get(recordId);
    if (!record || record.type !== 'text') return {status: 'unavailable', reference: null, recordId};
    try { reference = validateReference(record.target?.reference); }
    catch { return {status: 'invalid', reference: null, recordId}; }
  } else {
    try {
      reference = validateReference(JSON.parse(locator));
      if (JSON.stringify(reference) !== locator) return {status: 'invalid', reference: null};
    } catch { return {status: 'invalid', reference: null}; }
  }
  try {
    const result = await provider.resolve({reference});
    if (result.status !== 'exact') return {status: result.status, reference, recordId};
    return {status: 'exact', reference: result.reference, recordId};
  } catch {
    return {status: 'unavailable', reference, recordId};
  }
}
