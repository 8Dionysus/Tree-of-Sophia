import {referenceKey, validateReference} from './model.mjs';

const SCHEMA = 'tos.corpus.note-draft.v1';
const MAX_BYTES = 65536;
const sameNote = (note, draft) => note?.kind === 'note' && note.text === draft.text
  && (note.quote || '') === draft.quote && referenceKey(note.reference) === referenceKey(draft.reference);

function decode(raw) {
  if (typeof raw !== 'string' || new TextEncoder().encode(raw).length > MAX_BYTES) throw new Error('Note draft exceeds its recovery limit.');
  const value = JSON.parse(raw);
  if (!value || value.schema !== SCHEMA || typeof value.id !== 'string' || !/^recovered-[a-zA-Z0-9-]{1,100}$/.test(value.id)
      || typeof value.text !== 'string' || !value.text.trim() || value.text.length > 64000
      || typeof value.quote !== 'string' || value.quote.length > 64000
      || (value.originalNoteId !== null && (typeof value.originalNoteId !== 'string' || value.originalNoteId.length > 512))) {
    throw new Error('Invalid note recovery draft.');
  }
  return {...value, reference: validateReference(value.reference)};
}

// One small synchronous recovery slot bridges the IndexedDB debounce/unload
// boundary. It is scoped to this notebook and tab, never a notebook cache.
export function createNoteDraftJournal({notebook, storage, key, makeId = () => crypto.randomUUID()} = {}) {
  key ??= SCHEMA + ':' + (notebook?.status?.().dbName || 'session');
  let activeId = null;
  const store = () => storage === undefined ? globalThis.sessionStorage : storage;
  const read = () => {
    const raw = store()?.getItem(key);
    return raw == null ? null : decode(raw);
  };
  const remove = id => { if (read()?.id === id) store().removeItem(key); };
  return {
    capture({reference, text, quote = '', originalNoteId = null}) {
      const previous = read();
      if (previous && previous.id !== activeId) throw new Error('A previous note draft still needs recovery.');
      if (!text.trim()) { if (activeId) remove(activeId); activeId = null; return; }
      const id = activeId || 'recovered-' + makeId();
      const raw = JSON.stringify({schema: SCHEMA, id, reference: validateReference(reference), text, quote, originalNoteId});
      decode(raw);
      const target = store();
      if (!target) throw new Error('Note recovery storage is unavailable.');
      target.setItem(key, raw); activeId = id;
    },
    acknowledge({reference, text}) {
      const value = read();
      if (value?.id === activeId && value.text === text && referenceKey(value.reference) === referenceKey(reference)) {
        remove(activeId); activeId = null;
      }
    },
    async recover() {
      const value = read();
      if (!value) return false;
      if (!notebook?.getNote || !notebook?.putNote) throw new Error('Notebook recovery is unavailable.');
      const original = value.originalNoteId ? await notebook.getNote(value.originalNoteId) : null;
      const recovered = await notebook.getNote(value.id);
      if (!sameNote(original, value) && !sameNote(recovered, value)) {
        // A recovery copy must never overwrite a newer edit or resurrect a
        // note another tab deleted. Its stable ID also handles a lost ack.
        await notebook.putNote({id: value.id, kind: 'note', reference: value.reference,
          text: value.text, ...(value.quote ? {quote: value.quote} : {}), expectedRecordRevision: null});
      }
      if (!notebook.status().persistent) throw new Error('The recovered draft is still only in memory. Export it before closing.');
      remove(value.id);
      return !sameNote(original, value);
    },
  };
}
