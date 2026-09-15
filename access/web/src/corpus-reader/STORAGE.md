# Corpus reader notebook storage

`createCorpusNotebook()` is the durable personal state boundary for the
corpus reader. It stores user-authored notes, bookmarks, reading slots and
small interface preferences. It stores exact validated references and an
optional user-selected quote, but never stores a provider window or an
implicit copy of source text. A source revision therefore leaves the old
reference available for review instead of silently retargeting it.

The browser adapter uses IndexedDB database `tos-corpus-reader-v1`, schema
version 1. Its record stores are:

| Store | Key | Indexes | Contents |
| --- | --- | --- | --- |
| `meta` | `key` | `byKey` | one monotonic notebook revision record |
| `notes` | `id` | `byDocumentUpdated`, `byUpdated`, `byReference` | one note or bookmark per record |
| `readings` | `slot` | `byDocument` | legacy `primary`/`secondary` plus bounded per-version reading positions |
| `preferences` | `key` | `byKey` | small JSON-cloneable UI values |

Every mutating transaction reads the `meta` revision in the same IndexedDB
transaction; a transaction that changes records advances it once. `putNote`,
`deleteNote` and import accept an optional `expectedRevision`; a stale value
fails with `CorpusNotebookError.code === "conflict"`, including
`expectedRevision` and `actualRevision`. `putNote` and `deleteNote` also offer
a targeted note-record compare-and-swap, so unrelated reading edits need not
be treated as note conflicts. This is the caller-visible compare-and-swap
boundary for tabs sharing one database.

## Factory and status

```js
const notebook = createCorpusNotebook({
  indexedDB: globalThis.indexedDB,
  dbName: 'tos-corpus-reader-v1',
});

notebook.status();
```

The factory is synchronous; database opening is lazy and each operation is
asynchronous. `status()` returns `{adapter, persistent, warning, dbName,
closed}`. If IndexedDB is absent or cannot be opened, the notebook uses an
ephemeral memory adapter and reports `persistent: false` together with
`warning: "storage-unavailable"` (or the open error code). Passing
`adapter: "memory"` is the explicit test mode and reports
`warning: "memory-only"`. The UI must expose this state and must not call a
memory session "saved".

The dependency-free test adapter can share state between notebook instances:

```js
const memoryStore = createMemoryCorpusNotebookState();
const first = createCorpusNotebook({adapter: 'memory', memoryStore});
const second = createCorpusNotebook({adapter: 'memory', memoryStore});
```

## Operations

`getNote(id)` reads one exact record or returns `null`. Recovery uses this
bounded lookup to distinguish a committed write from a lost acknowledgement.

`putNote({id?, reference, quote?, text?, kind, expectedRevision?, expectedRecordRevision?})` validates
the exact reference with `model.mjs`, derives the document filter from
`reference.target.workId`, and returns `{item, revision}`. The item has an
opaque id, `kind` (`note` or `bookmark`), the exact reference and key, optional
quote/text, timestamps and its write revision. A supplied id updates that
record while retaining its creation time. `expectedRevision` guards the global
notebook revision. `expectedRecordRevision` is a targeted compare-and-swap for
that note: an integer must match the stored note's record revision, while
`null` asserts that the supplied id is absent. A targeted conflict reports the
note id and actual record revision, so an unrelated reading save does not
cause a false note conflict.

`deleteNote(id, expectedRevision?, expectedRecordRevision?)` returns `{id, revision}`. A missing id is
an explicit `not-found` error. A supplied targeted record revision is checked
in the same transaction as the delete. `listNotes({documentId?, limit?, cursor?})`
returns exactly `{items, nextCursor}`. Cursors are opaque, bound to the
document filter and sorted by updated time; the default page size is 50 and
the maximum is 200.

`saveReading({slot, documentId, versionId, reference, offset?})` returns
`{reading, revision}`. `loadReading(slot)` returns the stored reading or
`null`. The compatibility slots `primary` and `secondary` remain available.
For a position that must survive switching among more than two versions, use
`readingSlot(reference)` to derive a canonical key of the form
`reading:<encoded-work-id>:<encoded-version-id>`, then pass that key to both
methods. Canonical slots must match the reference's work and version. The
reading store admits at most 10,000 records; an import or new slot over that
bound fails atomically with `limit`. Reading positions are separate from notes
and are never rewritten when the source changes. UI code should load a
per-version slot on demand and reserve `primary` for the last-opened fallback.

`getPreference(key)` returns a cloned value or `undefined`. `setPreference`
returns `{key, value, revision}`. Preference values are cloneable JSON-like
values bounded to 256 KiB; they are not a text cache.

`exportPacket()` returns an object with this versioned envelope:

```js
{
  schema: 'tos_corpus_reader_notebook_v1',
  version: 1,
  revision: 12,
  exportedAt: '2026-09-13T00:00:00.000Z',
  notes: [...],
  readings: [...],
  preferences: [{key, value}]
}
```

`importPacket(packet, {expectedRevision?})` accepts this object or its JSON
string. It validates the complete packet, including every exact reference and
reference key, before starting one atomic merge transaction. A missing note id
is added; an exact existing note record is a no-op; the same id with different
contents raises `conflict` and adds nothing from that packet. Existing reading
slots and preference keys are preserved locally; only absent slots and keys
are imported. Canonical per-version reading slots are validated against their
references and count toward the 10,000 reading bound. This keeps a file import
from clearing unrelated user work.

`expectedRevision` is an optional local compare-and-swap guard. A stale value
raises `conflict` before any packet record is changed. A merge that adds at
least one record receives one new local revision; an exact no-op keeps the
current revision. Invalid packets, conflicts and quota errors leave the
previous notebook intact. The packet's global `revision` is provenance for
the export and is not trusted as the current browser revision.

`close()` closes the database. All later operations fail with `code ===
"closed"`; calling it again is harmless.

## Pending editor draft and document exit

The corpus editor keeps one nonempty draft in a notebook-scoped `sessionStorage`
slot, at most 64 KiB of encoded JSON. It contains the exact reference, personal
text and bounded quotation. This small synchronous recovery slot covers reload
or navigation before the 450 ms IndexedDB autosave completes. It is cleared
only after a durable write, and an older completion cannot clear a newer draft.
Notebook records, history and text caches remain in their existing stores.

Before notebook hydration, a pending draft is recovered as a separate note.
An already committed matching note or matching recovery ID prevents a duplicate;
recovery never overwrites a newer edit in another tab or revives its deleted
record. Failed recovery preserves the slot and reports the failure. Memory-only
fallback does not clear it or claim durability. Session storage itself lasts
for the browser tab; it is not a guarantee against closing the tab or browser
before a write has committed.

Visibility loss and `beforeunload` initiate a flush. Reader destruction returns
the pending flush promise, and the owning host waits before closing IndexedDB.
The unload warning remains while a draft or write is pending. Recovery handles
the case where the browser cannot finish asynchronous exit work.

## Error surface

The notebook exposes `CorpusNotebookError` with stable `code` values:
`invalid-input`, `invalid-reference`, `invalid-packet`, `invalid-cursor`,
`conflict`, `not-found`, `limit`, `quota`, `storage-error`,
`storage-unavailable` and `closed`. IndexedDB quota failures remain
`quota`; they are not converted to a successful memory write. An open-time
failure may fall back to memory, but its warning remains visible through
`status()`.

## Migration boundary

There is no implicit migration from `src/reader/notebook.mjs`, its
`localStorage` key, the Observatory reading state, or the installed demo.
Their paragraph ordinals and bounded anchors are a different identity
system. A future migration may import an explicit legacy archive into a
separate user-visible archive record after review; it must never translate
ordinals into corpus references or silently alter the existing localStorage
payload. Export/import is the current safe handoff.

The implementation is tested with the memory adapter in Node/Vitest. Real
browser IndexedDB upgrade, reload persistence, two-tab conflict behavior and
quota handling still require browser verification by the owning UI route.
