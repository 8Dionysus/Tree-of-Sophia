# Corpus reader data contract

This directory contains the frontend boundary for a future ToS corpus
reader. It is a read-only, page-bounded provider contract. The frontend owns
loading state, selection, layout, resume state, and navigation back to the
graph. ToS source, witness, segmentation, translation, rights, and review
surfaces remain authoritative for meaning and publication.

## Provider operations

`createCorpusProvider()` returns a provider with five required asynchronous
operations and an optional `structure` operation. The real application
receives either an explicit provider or an explicit caller-owned transport
adapter. No URL, HTTP verb, or backend route is inferred here.

For a transport adapter, the optional operation is supplied explicitly as a
caller-owned `structure` function:

```js
createTransportProvider({request, structure, capabilities});
```

The generic `request` callback is never assumed to know an unadvertised
operation.

```js
const provider = createCorpusProvider({provider: suppliedProvider});

await provider.catalog({query, cursor, limit, signal});
await provider.document({documentId, signal});
await provider.window({documentId, versionId, cursor, reference, limit, signal});
await provider.search({documentId, versionId, query, scope, cursor, limit, signal});
await provider.resolve({reference, signal});
if (provider.capabilities?.structure === true) {
  await provider.structure({documentId, versionId, cursor, limit, signal});
}
```

Paged operations enforce a maximum page size of 100 at the provider boundary;
the reader requests at most 40 text units for each pane and keeps at most two
version windows visible. Cursors are opaque to callers and become stale when
the provider's representation revision changes. `AbortSignal` is passed
through every operation and must be honoured by the eventual backend adapter.

`total` and `unitCount` are optional knowledge. A missing or `null` value is
normalized to an explicit unknown state; only a supplied non-negative integer
can be displayed as a count. An empty page with a non-null `nextCursor` is a
valid intermediate page. The reader exposes one explicit continuation action
for it, retains the previous readable page when a continuation fails, and
never turns an empty page into an automatic full scan. Providers and callers
must keep continuation pages bounded rather than fan out requests in the
background.

The bound provider also rejects text over 65,536 UTF-8 bytes per unit or
262,144 UTF-8 bytes per text page, and rejects metadata over 16,384 UTF-8
bytes per item or 131,072 UTF-8 bytes per page. These byte limits supplement
the character limits and apply equally to non-Latin text.

`catalog` returns `{items, nextCursor, total}`. Each catalog item is a work
summary. A summary may expose `status`, `availability`, `visibility`,
`textAvailable` and `unitCount: null` when only metadata is available, rights
restrict reading, or the count has not been reported. Such summaries remain
visible in the library. `document` returns a work manifest with one or more
versions. A version is the frontend's convenient name for a source
expression/edition/item/file/text-layer chain and contains:

```js
{
  id, label, language, role, revision, unitCount,
  status, availability, visibility, available,
  source: {
    workId, expressionId, editionId, itemId, fileId,
    fileSha256, textLayerRef, textLayerSha256
  }
}
```

For a metadata-only, restricted, expired, or otherwise unavailable version,
`revision`, `unitCount`, and `source` may be `null`. The catalog and manifest
still carry the availability state so a caller does not silently choose a
different version. A version can be selected for inspection, but `window`,
`search`, and `resolve` may report it as unavailable according to the
provider's capabilities and rights state.

`window` returns only the requested bounded unit page:

```js
{
  documentId, versionId, revision,
  units: [{id, ordinal, kind, text, language, reference, context?}],
  previousCursor, nextCursor, total
}
```

The optional `context.graphTarget` is an explicit `{kind: "node" | "relation",
id}` mapping supplied by the source/fixture. The UI never derives a graph ID
from a unit label, text, or ordinal.

`search` currently defines `scope: "version"` (with `"document"` accepted as
an adapter alias) for searching the requested work version. The model exposes
three distinct UI scopes: `loaded` searches only the currently retained text
window, `version` searches the selected version through the provider, and
`corpus` is enabled only when `capabilities.searchScopes` includes `corpus` or
`capabilities.corpusSearch === true`. A provider may advertise additional
scopes later, but the UI must show the scope and must not present a
loaded-window result as a corpus-wide negative result. Search calls are made
only for an advertised scope.
Search results are bounded `{unitId, reference, excerpt}` records.
The local `loaded` search also stops at its bounded result cap; when that cap
is reached its `total` remains unknown rather than claiming the cap is exact.

`structure` is an optional hierarchical-navigation seam. A provider advertises
it only by supplying the operation; the bound provider then exposes
`capabilities.structure === true`. It returns a bounded page of explicit
source-owned anchors:

```js
{
  documentId, versionId, revision,
  items: [{id, label, unitId, level, parentId?, hasChildren?}],
  nextCursor, total
}
```

The fixture implements a flat table of contents: it emits one level-0 chapter
item for each fixture section, with a section `unitId` generated by the same
opaque unit identity function as `window`. The synthetic section cadence is
one section per 33 units. The UI consumes the supplied `unitId` and does not
derive anchors from labels, ordinals, or chapter arithmetic. Providers that
do not implement this optional operation leave `capabilities.structure`
absent; a contents UI should handle that unsupported state explicitly.

## Durable reference

`model.mjs` exports `validateReference`, `referenceKey`, and `createReference`.
A reference contains no text or excerpt:

```js
{
  schemaVersion: "tos.corpus.reader.reference.v1",
  target: {
    workId, expressionId, editionId, itemId, fileId,
    fileSha256, textLayerRef, textLayerSha256
  },
  versionId,
  unitId,
  selector: {
    type: "text_position",
    start, end,
    positionUnit: "unicode_code_point",
    interval: "half_open"
  }
}
```

The target fields correspond to the stable source scope and source layer in
`ToS/contracts/source-text-unit-packet-v1.schema.json`; the selector follows
the text-position form used by `source-anchor-v2.schema.json`. The text-layer
digest is part of the identity, so `resolve` can return `exact`, `changed`,
or `unavailable` without using a graph snapshot revision. Translation
alignment is a separate source-owned packet and is not inferred by the
reader. A future alignment-aware provider may supply reviewed mappings.

The `window` operation also accepts a compact deep-link target:

```js
{unitId, revision}
```

It may be supplied as `reference: {unitId, revision}` or as the equivalent
top-level `unitId` and `revision` operation arguments. It is accepted only
when `unitId` belongs to the requested `documentId` and `versionId`, and
`revision` equals the current representation revision. A full durable
reference performs the same version and text-layer checks.

## Synthetic fixture

`createFixtureProvider({documentCount, unitCount, latencyMs, ...})` is a
deterministic public synthetic contract exercise. Its default logical shape is
10,000 works and 100,000 units. Catalog entries, manifests, units, and search
results are generated on demand; the fixture never allocates a corpus-sized
array or stores source payload copies.

`unitCount` is distributed across works by default. For scale scenarios,
`unitsPerDocument` gives every work the same logical length. The
`longDocumentUnits` option gives only the first work a long text while leaving
the remaining catalog entries small; `metrics().logical.totalUnits` reports
the resulting logical total, separately from the input `unitCount`.

The fixture includes multilingual original/translation versions and bounded
section, paragraph, verse-line, and note units, including Arabic (`ar`) and
Greek (`el`/`grc`) synthetic text when those languages are requested. It can expose explicit
`context.graphTarget` mappings, delayed requests, aborts, unavailable works,
failures, changed references, and revision changes through `provider.controls`.
`provider.metrics()` reports logical corpus size, request counts, active
requests, and maximum page materialization for scale tests.

Synthetic IDs and wording are test data only. They do not assert a scholarly
segmentation, translation, review, rights decision, graph relation, or canon
status.
