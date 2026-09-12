# Exact offline catalog contributions

`tos_access.catalog_semantics` owns one pure row-contribution and catalog renderer
used by native `knowledge_catalog` and the offline `catalog_index` SQLite reducer.
It preserves the complete existing catalog, not a summary or a second reduced
catalog schema. Owner header extensions, registry values, lenses and first-row
display forms retain their nested dictionary insertion order during copying and
emission; canonical binding hashes do not replace those carried values. No source,
semantic, rights or canon authority moves here.

## Caller-owned transaction API

```python
from tos_access.catalog_index import CatalogIndex, CatalogLimits
from tos_access.catalog_semantics import (
    CANONICAL_ORDER, CatalogInputs, CatalogRow, CatalogChange, catalog_digest,
    finalized_header,
)

# The owner has already opened a transaction on its private publication file.
inputs = CatalogInputs(source_header, entity_registry, relation_registry,
                       normalized_saved_lenses,
                       source_order_profile=CANONICAL_ORDER)
index = CatalogIndex(connection, CatalogLimits())
catalog = index.bootstrap(inputs, normalized_rows)
source_header = finalized_header(inputs, catalog)
# normalized_rows yields nodes before relations, each as:
# CatalogRow(kind, item['id'], (str(item['source_graph']), str(item['id'])), item)

# In another explicit owner transaction on this admitted index:
catalog = index.apply_delta(before_inputs, after_inputs, [
    CatalogChange('update', 'node', new_node['id'],
                  catalog_digest(old_node), new_node,
                  (str(new_node['source_graph']), str(new_node['id']))),
], expected_catalog_digest=catalog_digest(before_catalog))
source_header = finalized_header(after_inputs, catalog)
```

`bootstrap(inputs, rows)`, `apply_delta(before, after, changes)` and
`render(inputs)` return the exact catalog. They require an already active
transaction. They never begin, commit, roll back or close that transaction, and
never write prepared rows, search metadata, headers, epochs or a consumer binding.
The caller **must roll back the entire transaction on any exception**. A failed
instance refuses further work; create a new instance after rollback. Reopening
does not admit a different executable, registry, lens or normalization binding.
An absent or incompatible index requires explicit bootstrap, never a hidden
request-time fallback. Bootstrap refuses an existing `catalog_*` surface.
Interruptions such as `KeyboardInterrupt` propagate unchanged and also invalidate
the instance. SQLite can itself abort a transaction on an allocation error; the
index never performs an automatic rollback or retry.

`CatalogInputs` copies the full row-free source header, registries and normalized
saved lenses. `CatalogInputs.from_graph(graph, corpus, philosophy, entities,
relations, source_order_profile=...)` is a convenience for an already assembled
native graph. It invokes the existing saved-lens normalizer; it does not build a
graph. Index state binds the exact owner inputs, source header, source-order
profile, projector executable bytes and rendered catalog digest. Executable
binding is conservatively wider than catalog-only functions: unrelated edits to
`knowledge.py` can require explicit index bootstrap, without changing the graph
normalization fingerprint merely because this catalog refactor exists.

An explicit **draft after header** is mandatory. Known row-owned graph count fields
already present in that header are recomputed; unknown extensions are preserved.
The reducer does not invent additional count fields. In particular it does not
copy an old global `semantic_validation` report into a new header: a current
report or an owner-authorized omission must come from the stronger semantic
owner. Recounting rows is not semantic validation. `finalized_header(inputs,
catalog)` transfers the returned counts into that explicit header. Index state
binds this **final output header**, not the draft. The owner must use it for the
prepared publication and construct subsequent before/render inputs from it.
Before and render validate the exact final header digest without normalizing it:
tampered known counts or a changed semantic report therefore refuse. This
draft-after/final-before asymmetry avoids requiring the caller to duplicate
the index's row recount while keeping semantic/unknown fields owner-controlled.

## Identity and exact order

Caller identity is `(kind, exact id)`; private compact contributor doc integers
are independent of prepared/search addresses. Changes require the SHA-256 of
the exact full normalized row's sorted, compact, Unicode JSON bytes
(`catalog_digest`), not its `content_revision` member. Missing/wrong old digests,
duplicate targets, duplicate identities and duplicate owner order keys refuse.

`source-graph-id-v1` (`CANONICAL_ORDER`) requires exactly the native graph order
key `(str(source_graph), str(id))` from each carrier. An escaped UTF-8 BLOB
preserves Python tuple/codepoint ordering, including NUL and prefix cases; JSON
text order, length-first keys and hidden doc-id tie breaking are not used.
Insertions and source moves do not renumber unrelated rows. The separate
`owner-sequence-v1` profile accepts explicit unique nonnegative 63-bit integer
positions for custom source sequences. Inserts require a position; updates may
retain it. A bounded delta can swap positions or reuse a deleted position inside
the transaction. The profiles cannot be mixed within an index.

## Reversible facts and work bounds

Each contributor retains a compact compressed fact vector, a row digest and
sealed minimal endpoint/route facts, not the full carrier. Interned atoms serve
reversible positive counters and ordered occurrences. Heads retain the first
survivor for each distinct value. A changed contributor updates counter
differences and changed postings; untouched contributors are not rewritten.

Attribute array-member counts preserve multiplicity and JSON types. At most
five distinct eligible examples per row/field are retained, but candidates from
**all rows** remain indexed: deleting the first five can promote later values.
The original 180-codepoint JSON limit, first encounter, `1` versus `1.0`, `False`,
negative zero, null exclusion, recursive dictionary fields and safe-field rules
remain exact. Representative labels come from the first surviving group row.
Facet multiplicity and casefold ties retain first encounter. Registries, mapped
versus fallback counts, claim relation-type counts, saved lenses, presentation,
capabilities and all seven entity routes use the shared renderer.

Node route-mask changes select the union of its indexed from/to incident
relations. Final node summaries move before route recomputation; changed relation
endpoints are evaluated in the final transaction state. Self-loops count once,
both endpoints are required, and a dangling relation refuses. Label/provenance
changes with unchanged route masks do not traverse adjacency. Bootstrap streams
nodes before relations and does not keep a second graph or full-row map.
Native in-memory rebuild needs only the first five examples because it has no
deletions; the index intentionally retains more candidate occurrences.

Rendering reads bounded aggregate groups and ordered distinct heads, never full
contributors or raw rows. Delta checks the bound before catalog and renders the
after catalog, so it is not constant work independent of output vocabulary.
Storage is proportional to contributor facts and eligible occurrences, not just
five global examples. Default refusal limits are 4096 changes, 16384 affected
relations, 8 MiB per input row, 64 MiB selected delta bytes, 100000 aggregate
entries, 64 MiB decoded aggregate atoms, 16 MiB output and 4 GiB for the **whole
shared SQLite file**. These are refusal caps, not resource forecasts or host
write permission. Bootstrap is an explicit full stream, not bounded by the delta
change count. SQL mutation accounting belongs to the enclosing publication owner;
this module does not install or replace that owner's SQLite progress handler.
`PRAGMA max_page_count` is tightened to the smaller of the existing owner ceiling
and this whole-file byte cap (rounded down to pages), never raised. SQLite thus
refuses page allocation before exceeding the cap. The owner must admit realistic
full-index and journal/WAL capacity separately. Obsolete interned atoms may persist until a new bootstrap;
the whole-file cap remains enforced.

Selected row/fact/summary digests, endpoint identity, aggregate catalog digests,
and explicit transaction binding detect declared framing and selected damage.
This is not a cryptographic proof of completeness of an arbitrarily modified
hidden occurrence or adjacency index. An admitted owner-built baseline and its
controlled mutation path remain assumptions; no malicious-co-owner guarantee or
source-independent semantic acceptance is claimed.

## Verification

### Prepared publication join

`tos_access.prepared_catalog` connects this reducer to the existing full-carrier,
lens and search publication in the **same caller-owned SQLite transaction**:

```python
from tos_access.prepared_catalog import (
    bootstrap_prepared_catalog_transaction,
    apply_catalogued_prepared_delta_transaction,
)

# Explicit offline attachment to an already admitted prepared baseline.
# Caller starts the transaction and rolls back the whole transaction on failure.
receipt = bootstrap_prepared_catalog_transaction(
    connection, expected_binding=binding, inputs=before_inputs)
# No binding/clock/header/row/search content changes during attachment.

result = apply_catalogued_prepared_delta_transaction(
    connection, expected_binding=binding,
    before_inputs=before_inputs, after_inputs=draft_after_inputs,
    changes=prepared_changes)
# Commit only after any stronger owner checks. On ANY exception: whole rollback.
# Retain result['binding'] and the FINAL result['source_header'], not the draft.
```

Attachment streams prepared rows in their owner source order and verifies their
full emitted-row digests and exact indexed identity. The rendered catalog and
final header must equal the selected baseline before commit. This is an explicit
full bootstrap, not an update and not a hidden reader fallback.

Delta checks the exact prepared binding, descriptor/header, registry digests and
before catalog, then copies only the bounded selected full old/new rows. The
catalog's expected old-row digests are computed from those verified carriers.
An input iterator cannot change an earlier captured item between the catalog and
prepared write passes. Canonical catalog order comes from the row's exact
`(source_graph,id)`; owner-sequence order uses `PreparedChange.source_order`.
Insertions still need an explicit prepared sparse order token, even with the
canonical catalog profile. Updates preserve that token unless explicitly moved.

The shared publication byte/page cap applies to the whole file; the SQL mutation
budget accounts for both catalog and prepared/search/lens mutations. A failed
operation never commits, but may have changed the open transaction: the caller
must roll it back, including its own earlier work. Only the final joined receipt
and commit can establish a new local publication. Reader activation is separate.

The caller still owns dependency-complete normalization, the explicit current
semantic report, source revision and source/assessment transition. This join does
not verify source growth or grant semantic acceptance. Its receipt states
`source_transition_verified=false`, `semantic_acceptance=false` and
`consumer_switched=false`; those are not claims about stronger owner work.

### Regression evidence

The frozen oracle in `access/tests/catalog_oracle.json` was captured from
`knowledge_catalog` at commit `900202441e1c8894676efef90c420df2601598e5`
**before** refactoring. An additional unsorted compact JSON oracle was captured
directly from that same immutable commit; it detects nested owner-value and lens
dictionary-order drift hidden by canonical hashing. Explicit small carriers cover
first-row deletion, more than five examples, empty graph, legacy mode, role
changes, reversed source order and ordered nested extensions. Both new reducers
must match both original digests, not merely each other. Additional delta tests exercise transactional rollback,
Unicode ordering, bounded adjacency, source moves, counter disappearance,
corruption, budgets and source-free rendering. Tests do not establish full-corpus
bootstrap cost, CI, publication, consumer activation or runtime acceptance.
