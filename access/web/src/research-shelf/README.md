# Personal research shelf

`research-shelf/` owns the browser's local, user-owned index of research
addresses. It stores no source wording, exploration cursor, runtime handle or
command. A shelf record has exactly:

```text
{ id, title, type, target, collectionIds, createdAt, updatedAt, revision }
```

The five target types are `material`, `form`, `text`, `lens` and `route`.
`material` keeps graph kind/id plus source and content revisions. An optional
Claim binding is passed through `validateReading` as one reading entry, so the
exact Claim closure is retained. `form` uses one of the seven owner-declared
roles and an exact `{id,version,digest}` form reference. `text` delegates to
the corpus reader's `validateReference`, preserving both the v1 corpus and
native references. `lens` delegates to `validateDraft`. `route` keeps an exact
material origin and only the bounded exploration profile, direction, depth,
source list and predicate list.

## Async storage seam

`createResearchShelfStore()` opens `tos-research-shelf-v1` in IndexedDB. The
database has independent `records`, `collections` and `meta` object stores.
Writes increment a global generation and a per-object revision. Updating or
deleting requires the current record revision; a stale writer receives a
`ResearchShelfError` with `code: "conflict"`. Lists are descending by
`updatedAt,id`, default to 24 records and accept at most 100. Cursors carry
the generation and exact type/collection filter; a changed shelf or filter
invalidates a cursor.

`createMemoryResearchShelfAdapter()` and
`createMemoryResearchShelfStore()` implement the same asynchronous adapter
shape for unit tests. If IndexedDB is absent or cannot be opened, the store
falls back to memory and `status().warning` is explicitly
`"storage-unavailable"` (an explicitly requested memory adapter reports
`"memory-only"`). The fallback never pretends to be persistent.

The adapter methods are the documented future sync seam:
`getMeta`, `getRecord`, `listRecords`, `saveRecord`, `deleteRecord`,
`getCollection`, `listCollections`, `saveCollection`, `deleteCollection`,
`exportPacket`, `importPacket` and `close`. They are all async and carry
expected revisions/generation. No network or sync implementation belongs in
this package yet.

`export()` produces `tos.research_shelf.export.v1` and preserves canonical
typed targets. `import()` is additive and validates the complete packet before
an atomic transaction; an existing id with different content rejects the
whole import.

Deleting a collection atomically removes that membership from its records while
preserving the records and their other collections. Each affected record gets
a new revision and a nondecreasing update time, so a stale editor cannot restore
the deleted association unnoticed. The shelf generation changes once for the
whole operation. IndexedDB visits only the collection's membership index; it
does not load the whole shelf. A failed member update rolls back every change,
including the collection deletion.

`migrate(packet, {source})` is an explicit supplied-packet import from a real
owner export. `source` must be `reading-resume`, `research-workspace`, or
`workspace-copy`; the corresponding owner decoder is used before any shelf
record is prepared. A reading entry becomes one exact `material` address. A
workspace session keeps its notes, route poses, selections, hypotheses and
proposals in their owner packet and reports each skipped item with a reason.
Exact lens drafts may be supplied separately as `lenses` when importing a
`research-workspace` packet; a full `workspace-copy` already carries its
validated saved lenses. The result includes `{retained, skipped}` alongside
the additive import counts. The supplied packet is detached and never read
from or removed from `localStorage` or another implicit namespace. There is no
synthetic shelf export v0 migration.

Retained material and lens records have no owner-supplied shelf creation time.
When `migrate()` has no `now` override, both record timestamps use the stable
`MIGRATION_RECORD_TIMESTAMP` value `1970-01-01T00:00:00.000Z`. It marks unknown
local chronology for the imported record; it is not the material's production
time or the owner export's preparation time. The generated shelf packet may
still carry the current preparation time in `exportedAt`, which does not take
part in record equality. An explicit `now` remains the timestamp override, and
`workspace-copy.exportedAt` is never used as a record timestamp.

## Contextual mount

```js
const shelf = mountResearchShelf({
  host: document.body,
  locale: () => document.documentElement.lang || 'ru',
  onOpen: value => value.type === 'text'
    ? openNotebookNote(value)
    : openResearchRecord(value),
  onOpenView: ({element}) => { host.inert = true; pauseSky(); },
  onCloseView: ({element}) => { host.inert = false; resumeSky(); },
  onError: error => report(error),
  notebook, // optional existing corpus notebook; never copied into shelf
});

await shelf.open();
await shelf.save(input, {expectedRevision: 1});
const page = await shelf.list({limit: 24});
await shelf.remove(record.id, record.revision);
await shelf.removeCollection(collection.id, collection.revision);
shelf.close();
await shelf.destroy();
```

The mounted view searches only the currently loaded shelf page, exposes
collection filters and editing, reports conflicts with a save-copy action,
and provides paginated import/export. Existing corpus notebook notes are
shown by calling `notebook.listNotes({limit: 24, cursor})`; they remain in
their notebook store and have their own pagination. A note action calls
`onOpen({type: 'text', target: {reference: note.reference}, note})`; `note`
is the notebook object returned by `listNotes`, not a copied shelf record.
Shelf record actions call `onOpen(record)` with a detached shelf record.
`onOpenView` runs when the dialog becomes active and `onCloseView` runs before
focus returns to the opener; both receive `{element, opener}` so a host can
set `inert` and pause its background motion. Escape is stopped at the dialog
boundary and Tab remains inside it. `remove` and `removeCollection` require
the record's current revision and therefore surface a concurrent change as a
CAS conflict.
All visible values use DOM `textContent` or input values, and the CSS includes
mobile, keyboard focus and reduced-motion handling.

## Browser acceptance fixture

Open `/static/fixtures/research-shelf.html` from the Vite development server to run the
synthetic IndexedDB check. It uses a fresh database name and verifies two
connections' record CAS, cursor invalidation after a write, all-or-nothing
additive import, and reopening the same database. Save its compact result with
the local acceptance evidence; runtime proof files do not belong in this source package.
