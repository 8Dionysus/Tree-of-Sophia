# Local prepared publication

`tos_access.prepared_publication` is an explicit offline owner API for
`tos_local_prepared_read_model_v1`. It accepts a coherent normalized graph and
catalog selected by its caller. It creates one new SQLite file with mode 0600,
in one transaction. It never calls a graph builder, writes authored sources,
switches a consumer, generates edge SQL, deploys, or grants semantic admission.

The profile carries exact compact full-row JSON and emitted-byte digests,
catalog, lens metadata/order, required identity/adjacency indices, a publication
clock and compressed search-v3. It deliberately has no legacy search tables,
row-per-gram index, search-v2 capability, static edge responses or edge runtime.
The common lens-v2 reader header binds this distinct local schema. Search text
uses Python default JSON separators with sorted keys and lower(), preserving
the existing search reference; it is not the compact carrier representation.

## Explicit bootstrap

```python
from tos_access.prepared_publication import publish_prepared
binding = publish_prepared(new_path, graph=normalized_graph,
                           catalog=coherent_catalog)
```

Graph/catalog source revisions must agree and include the exact normalization
binding and source authority boundary. The caller owns source assembly, path
normalization, schema/semantic validation, rights and the final consumer choice.
The API mechanically checks framing, exact identities, duplicate targets and
endpoint closure; this does not substitute for those owner obligations.

Bootstrap traverses explicit repeatable row lists to calculate a reproducible
descriptor, then streams each carrier into SQLite once alongside search. The
descriptor binds the profile, search algorithm, physical search storage version, source header, catalog digest
and an ordered SHA-256 stream of kind, exact ID, numeric address, source-order
token and exact compact-row digest. It rechecks that stream during insertion.
The search header binds the **final** publication snapshot, avoiding a cyclic
header hash. Random search cursor incarnation is outside reproducible content
identity; this is not a claim of SQLite file byte reproducibility.

The descriptor schema is `tos_local_prepared_revision_v2`. Delta maintenance
checks its bounded stored bytes, exact framing/digest, profile, algorithm,
physical storage version and capabilities before row mutation. Earlier
descriptors or another storage version require an explicit new-file bootstrap;
they are not silently adopted even when the forward query ABI is unchanged.

`publish_prepared_rows(new_path, source_header=header, catalog=catalog,
row_factory=rows)` accepts an explicit repeatable `rows(kind)` factory instead
of in-memory row lists. The header excludes `nodes` and `relations`. The factory
is called once per kind for the descriptor/count pass, then once per kind for
the insertion pass, nodes before relations. It must return the same normalized
rows in the same order. No transformed row collection is retained by the writer.
Per-row, histogram and output storage limits still apply; the first pass also
refuses a row count above the mutation budget. A changed, exhausted or failed
second pass rolls back the new file. This seam can consume disk-backed rows;
it neither assembles source dependencies nor makes a full bootstrap incremental.

Both bootstrap entries accept the explicit pair `search_scratch_path` and
`search_scratch_limits=BulkBootstrapLimits(max_bytes=..., max_mutations=...)`
from `tos_access.compressed_search_bootstrap`. Supplying both selects the
[scratch-backed bulk initializer](COMPRESSED_SEARCH_V3.md#explicit-scratch-backed-bulk-bootstrap);
omitting both keeps the buffered initializer. There is no automatic fallback
or retry. The owner reserves the main database, its rollback journal/headroom
and the independently capped scratch file before execution. Scratch has an
exclusive, non-adopting lifecycle and is removed on success or failure; a
foreign/replacement inode is never deleted.

Bulk changes physical block layout, not the publication descriptor, binding,
full rows, exact search semantics or later delta API. Before bulk work the
publisher reserves its already-written header, four fresh carrier DML rows
per input row (carrier, lens order, emitted digest, stable address map), and
the final prepared state. The remaining mutation allowance covers search-main
plus scratch writes. The final whole-publication check counts main
`total_changes` plus scratch mutations once each. Individual budget-limited
search page boundaries can differ between physical layouts; the complete
result stream and snapshot-bound continuation contract remain unchanged.

Producer-owned `(kind, exact ID) -> doc_id` addresses are stable. Bootstrap
assigns monotonic addresses and sparse source-order tokens in native row-list
order, with stride `2**32`. Search uses those tokens only to break equal-lower-ID
ties, never SQL iteration order. Deleted numeric addresses are never reused;
prepared and search high-water marks must agree.

## Addressed storage delta

`apply_prepared_delta(path, expected_binding=..., source_header=...,
catalog=..., changes=...)` accepts typed `PreparedChange` insert/update/delete
targets. The source header excludes node/relation lists and belongs to the new
catalog snapshot. Insertions require an explicit source-order token chosen by
the owner; updates retain their token unless explicitly changed. Retokenization
inside a tie group is the caller's bounded dependency responsibility.

One transaction updates selected rows/digests, identity map, lens ordering and
histograms, compressed search, high-water, header and epoch. It rejects a stale
binding, normalization drift, foreign/missing endpoints and a node deletion
leaving incident relations. Normalization drift requires an explicit new-file
bootstrap. Every successful delta advances the epoch, including content returns
to an earlier state, so old selected bindings remain stale.

Delta revision identity is history-addressed: its descriptor binds the previous
data revision, new source header/catalog and ordered explicit change frames.
Equivalent final rows reached by bootstrap or a different delta history need
not share `data_revision`. No full-vs-delta version or byte equality is claimed.
The algorithm does not scan the full graph or regenerate complete edge SQL.
The caller still owns dependency-complete source-to-normalized assembly and
global semantic invariants; storage locality does not prove source boundedness.

`apply_prepared_delta_transaction(connection, ...)` allows a larger owner
transaction to include a sentinel or another publication lane. The caller must
already hold a transaction, and must roll it all back on **any** exception,
including SQLite errors which may themselves abort it. The function never
begins, commits, rolls back or closes that connection. There is no automatic
retry. File wrappers own the transaction and close resources.

## Budgets and selection

Default owner limits: whole SQLite file 64 MiB, compact row 1 MiB, metadata
8 MiB, retained delta compact inputs 16 MiB, 4096 delta targets and two million
SQL mutations. These are refusal bounds, not memory forecasts or permission to
write. A larger profile requires explicit compatible reader limits. Search
receives the cap for the **whole** shared database and cannot increase a stricter
owner cap. SQLite rollback journals require separate host capacity headroom.
Failure removes only a newly created bootstrap inode; existing files are never
overwritten. Delta failures leave the preceding publication selected.

The returned binding can be explicitly selected with `PublishedKnowledgeReadModel`
and the separate `SearchStore` API. This module installs no public route and
changes no CLI/HTTP/MCP default. `CAPABILITIES` and the revision descriptor name
only this offline local profile. A selected read verifies its declared framing
and budgets; it is not an acceptance claim for the entire source corpus.

Focused checks: `PYTHONPATH=access/src:access/tests python -m unittest
access/tests/test_prepared_publication.py`. Test ownership lives in
`tests/test_inventory.json`; standalone access lane remains command authority.
