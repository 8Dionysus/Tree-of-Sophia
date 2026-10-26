# Corpus reader data contract

This directory contains the frontend boundary for a future ToS corpus
reader. It is a read-only, page-bounded provider contract. The frontend owns
loading state, selection, layout, resume state, and navigation back to the
graph. ToS source, witness, segmentation, translation, rights, and review
surfaces remain authoritative for meaning and publication.

## Provider operations

`createCorpusProvider()` returns a provider with five required asynchronous
operations and optional `structure` and `analyze` operations. The real application
receives either an explicit provider or an explicit caller-owned transport
adapter. No URL, HTTP verb, or backend route is inferred here.

For a transport adapter, optional operations are supplied explicitly as
caller-owned functions:

```js
createTransportProvider({request, structure, analyze, capabilities});
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
if (provider.capabilities?.analyze === true) {
  await provider.analyze({reference, form, language, signal});
}
```

Paged operations enforce a maximum page size of 100 at the provider boundary;
the reader requests at most 40 text units for each pane and keeps at most two
version windows visible. Cursors are opaque to callers and become stale when
the provider's representation revision changes. `AbortSignal` is passed
through every operation and must be honoured by the eventual backend adapter.

`analyze` is an optional, selected-occurrence operation. The caller passes the
exact validated reference and the literal form sliced from its Unicode
code-point selector; the adapter must not normalize the form or move the
selector. The language is the selected source version's language, independent
of interface language. The bounded response is bound back to the exact same
reference:

```js
{
  status: 'available' | 'unavailable',
  reference,
  reason?: 'unrecognized' | 'not_processed' | 'provider_error' |
    'unsupported_language' | 'source_changed' | 'form_mismatch',
  analysisVersion?: string,
  source?: {id, label?, version?, url?},
  candidates?: [{lemma, morphology?, gloss?, source?, sourceRef?, alternatives?, dictionaryUrl?}],
  dictionaryLinks?: [{label, url}],
  mappings?: [{status:'proposed', reviewStatus:'model_proposal_owner_review_pending',
    sourceReference, sourceQuote, targetReference, targetQuote,
    sourceVersionId, targetVersionId, sourceRevision, targetRevision,
    targetVersionLabel, targetLanguage, reviewBasis, evidence}],
  mappingStatus?: 'available' | 'none'
}
```

Available results require a version and at least one supplied candidate;
candidate order is not a best-analysis claim. An unavailable result still
echoes the exact reference and may provide dictionary links for navigation.
The reader displays only supplied candidates, alternatives, attribution and
links. Literal concordance uses the existing version-scoped `search` operation
and its bounded cursor; it does not fetch the full work or infer a lemma.
Dictionary URLs are limited to HTTP(S). A form search link remains distinct
from a lemma-specific candidate link. Optional mappings are source-bound,
exact-span model proposals; the provider re-resolves the target reference and
checks its quote before returning it. A missing or stale target yields an empty
mapping list with `mappingStatus: 'none'`, without changing morphology status.
The reader preserves alternatives and analysis errors, labels proposals as
unreviewed, and never writes a review or canon claim.

`total` and `unitCount` are optional knowledge. A missing or `null` value is
normalized to an explicit unknown state; only a supplied non-negative integer
can be displayed as a count. An empty page with a non-null `nextCursor` is a
valid intermediate page. The reader exposes one explicit continuation action
for it, retains the previous readable page when a continuation fails, and
never turns an empty page into an automatic full scan. Providers and callers
must keep continuation pages bounded rather than fan out requests in the
background. A missing `window.limit` defaults to forty units, matching the
reader's per-window bound.

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
The shared `tos-web-rules::corpus_reference_rule_v1` owns normalization, key
ordering, version/document IDs, and the native-reference code-point rules;
the browser modules marshal values through its initialized WASM binding. A
reference contains no text or excerpt:

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
reader. A provider may supply an exact source-bound proposal from that packet;
it remains weaker than human review and canon and is opened only after an exact
target resolve.

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
