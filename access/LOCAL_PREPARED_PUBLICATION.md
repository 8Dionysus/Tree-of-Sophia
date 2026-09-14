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

Generic lens scans use the source-scope indexes before sorting candidate IDs,
so unrelated source graphs do not consume every page's SQLite work budget.
New publications include these indexes. An existing selected snapshot can be
upgraded explicitly with `ensure_source_scope_indexes(db)` from
`tos_access.prepared_publication`, inside a caller-owned offline transaction.
Reserve index/storage work before invoking it on a large file. It validates
existing index definitions and is idempotent; it does not alter source rows,
metadata, publication epoch, data revision or cursors. No request creates DDL.
Older snapshots remain readable with the bounded legacy scan until this step
is performed. The reader does not raise budgets or make broad native filters
cheap merely because their source scope is indexed.

### Compact lens rendering carrier

`compact_lens_carrier.compact_lens_carrier(kind, emitted_json)` is the pure
`tos_compact_lens_carrier_v1` input projection for compact rendering. It binds
the exact complete source-row SHA-256 and its own seed SHA-256. `render(language)`
uses the existing compact carrier and human-form selector; it has no full-record
mode. It preserves unknown non-omitted fields and all form inputs, including
invalid collections so that invalid/over-budget states cannot become "missing".
The seed itself is internal and may retain form inputs in `attributes`; consumers
receive `render()`, never the seed as a replacement normalized/source record.

`supports_compact_lens_carrier(bound_spec)` excludes full output, full-text seed
search, and predicates, path filters, grouping or sorting that need omitted
attributes, source-record/readable sidecars or canonical Claim source text (or
their parent containers). An omitted field is not a negative query result.
The planner must use another exact plan or return its explicit bounded refusal.

An optional local prepared store can be installed explicitly with
`compact_lens_store.prepare_compact_lens_store_transaction(db,
expected_binding=binding, limits=CompactStoreLimits(...))` inside a caller-owned
offline transaction. Reserve storage and apply a SQLite file cap before a large
installation. Input rows, source bytes and retained seed bytes have independent
bounds; failure requires rollback of the complete transaction, including DDL.
Installation verifies every source digest and preserves publication identity,
epoch and full records. An existing store requires an explicit migration; it is
never rebuilt by a read request.

For covered compact requests the local lens reader selects this store, checking
its snapshot/epoch binding and each selected source/seed digest. Full inspection,
uncovered filters and full output retain the complete-row path. The local
prepared delta writer maintains inserted, changed and deleted seeds atomically
with complete records, search and the successor publication. Rollback covers all
lanes. Base-table triggers invalidate compact state if an older writer changes
records without maintaining it; compact reads and subsequent deltas then refuse
the stale store instead of using it. Triggers do not compute or admit hashes.

The separate optional membership index below can remove supported selectors
from native candidate scanning. Both installers default to local prepared v1;
an explicitly selected v9 D1 SQLite snapshot additionally requires
`expected_read_model_schema='tos_cloudflare_edge_read_model_v9'`. The same exact
binding/digest admission and caller-owned offline transaction apply. This API
does not import into D1, switch a consumer, or maintain an external D1 writer.
Broad stored lenses remain bounded by all other reader and
query budgets. A carrier checksum is not source admission or index-completeness
proof.

### Exact view/layer membership

`lens_membership_index.prepare_membership_index_transaction(db,
expected_binding=binding, ...)` explicitly installs the complete local index
for normalized `view_ids` and `graph_layers` string arrays. It verifies source
digests, refuses malformed or over-budget arrays, bounds source rows/bytes and
index entries, and requires caller-owned transaction, storage reservation/file
cap and rollback on failure. Full records and publication identity do not move.

The reader compiles positive `contains`, scalar `eq` and `in` conditions in
`all`/`any` groups to exact indexed counts and ordered keyset windows. It retains
case-sensitive list semantics, source scope and relation traversal regime.
Empty/negative predicates, array equality, property bindings, mixed unsupported
groups or over-budget Boolean SQL expansion keep the bounded native path; they
are not approximated by an empty posting. Generic sorts/path/seed constraints
also retain their existing native evaluation where required.

Membership entries and compact seeds participate in the existing addressed
prepared transaction, including metadata-only successors and rollback. Base
record or direct index mutation invalidates membership state until the owner
writer seals the exact successor. No request builds, repairs or re-admits an
index. Existing snapshots without this optional lane remain readable.

The native D1 lens reader admits these optional versioned stores against the
exact v9 header digest, normalization binding, data/source revisions and
publication epoch. It rechecks admitted state before returning a result, keeping
mid-query invalidation separate from initially stale/unavailable data. Full
inspection and uncovered predicates still read complete records. Boolean index
plans have a smaller D1 parameter allowance than local SQLite; excessive plans
retain the bounded native fallback. The addressed prepared-to-D1 delta route
also maintains explicitly installed stores: it verifies the selected old seeds
and complete membership rows, stages only changed rows and seals both stores
inside the same revision-guarded publication trigger. Reverse publication seals
the restored data against a new actual epoch; it does not reuse an old publication
identity. Published lens cursors independently bind the complete admitted
`tos_published_knowledge_snapshot_v1` (including data revision, epoch and header
digest) together with the v7 result fingerprint. The result fingerprint remains
content-scoped; the cursor is publication-scoped. The same snapshot can resume
after a runtime restart, but a changed publication or an ABA rollback requires
a fresh query even when the bounded result is identical. Legacy unbound lens
cursors are rejected with 409/restart, not silently reinterpreted. Immutable
graph-only execution retains its existing content-bound continuation format.
Staging alone leaves the previous stores readable, and partial staging or
intervening invalidation aborts the whole publication. Capture/read/retention
and SQL budgets include the auxiliary rows. No query or delta capture installs
the optional tables. The full SQL producer also emits and seals both stores,
and its full-producer deltas maintain them through the same guarded publication.
An older baseline without the auxiliary descriptor requires an explicit initial
migration; it does not acquire an applicable delta. See the
[D1 producer contract](deploy/cloudflare-worker/README.md) for its independent
output budgets and maintenance-bootstrap boundary. Older writers invalidate
the store binding instead of silently repairing it. These implemented routes
do not migrate an existing D1 database automatically or broaden the addressed
prepared-pair profile beyond its declared bibliographic transition scope.

For both local and native D1 readers, endpoint policy `both` retains exact pair
probes for small selected bases; above 64 nodes it scans actual outgoing
incidence with a from-only index instead of enumerating every possible pair.
External degree still consumes the existing work budget. Index availability
alone is not proof that a full wide result fits row, byte or VM budgets.

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

## New-file bootstrap with an explicit search donor

`publish_prepared_rows(..., search_reuse=PreparedSearchReuse(old_path,
old_binding, ...))` can avoid regenerating unchanged compressed search postings
during an explicit normalization migration. Import `PreparedSearchReuse` from
`tos_access.prepared_search_reuse`. It is mutually exclusive with bulk-search
scratch arguments. It never relaxes the ordinary delta normalization guard.

The donor is a caller-selected, already admitted local prepared search snapshot,
opened read-only for one transaction. Its exact publication/search binding,
storage/algorithm/Unicode versions, fixed search schema, bootstrap descriptor,
data revision, catalog/lens digests, complete dense address population and every
source carrier checksum/address/order are checked. Delta-history donors are
not supported. Stored search text, ranking values and filters must exactly match
the old complete row projection; display equality alone is insufficient.
The owner's term generator checks each sealed reverse frame against the old
source row. A bounded document/dictionary audit then compares streaming forward
memberships with reverse-frame counts and digests, including posting counts,
kinds, ordering, fences and orphan text/value records. Corruption is refused,
not repaired. These are mechanical integrity checks, not source or semantic
admission. Reuse still reads the full donor and regenerates its term sets for
validation; it avoids rebuilding unchanged postings, not all global work.

The fresh normalized row factory still runs twice and its complete emitted-row
digest must agree. The source population and sparse source-order tokens must
match the donor exactly. Changed complete rows become bounded search-document
replacements, including sidecar changes, because full JSON is searchable.
Unchanged text/postings are copied through the fixed search-v3 schema. A new
descriptor, header, catalog, lens and cursor incarnation bind the successor;
the donor and existing reader are untouched. Donor read bytes, copied bytes and
rows, retained batch bytes, changed documents and whole-file/mutation bounds
are explicit. Additional defaults bound donor queries (two million), SQLite VM
steps (one billion, metered in blocks of at most 1000), and conservatively
charged retained validation state (128 MiB). State charges are refusal limits,
not measured RSS; per-row decoding/term work remains subject to the existing
document limits. The optional `progress` callback receives phase/counter
dictionaries before commit; callback failure aborts the successor, and its
`search_successor_prepared` phase does not claim a committed publication.
Any refusal or interruption removes only the new file; there is
no hidden fallback to a full rebuild.

This publishes the base read model only. Existing semantic/catalog maintenance,
source inputs/dependencies, agent-context, compact and membership indexes are
not copied or re-admitted under a different normalizer. Their existing owner
bootstrap APIs must build the required successor lanes before a caller selects
that complete profile. D1 conversion/import, consumer switch and rollback
selection remain separate operations.

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

## Measured full-corpus baseline (2026-09-12)

The explicit bulk bootstrap at implementation `c13004a277dca6f32ba3480cff76320df94afd0d`
and source `7920ed81a54ddc5ac4e34d579f0c477b2e67ba3a` completed with
42,487 nodes and 62,504 relations. Its SQLite file is 4,120,215,552 bytes;
source revision is `98577ef4431523f7f809ff29db800958b85c68df092016b85919992883230de2`.
This is a retained measurement, not current-source or consumer admission.
All rows have a mapping, but 18,242 reported semantic gaps remain; mapping and
mechanical validity do not resolve those gaps or accept their contents.

The measured profile explicitly raised the database cap to 4 GiB and reserved
12 GiB of host headroom. Bootstrap took 4,164.912 s wall time and 3,874.420 s
CPU, with a reported cgroup peak of 1.9G and swap peak of 1.4G. Search used
37,015,552 bytes of scratch, 1,515,815 blocks and 21,101,011 main-plus-scratch
mutations. This cost is a full migration baseline, not a permissible small-edit
path, latency promise or portable host forecast. It does not justify raising
the default 64 MiB profile for every caller.

Selected-reader measurements below use fresh processes with the OS page cache
unchanged. Times measure local operation work, not process startup, HTTP,
network transfer or rendering. Repeat ranges are three observations, not
percentiles or a service-level guarantee.

| Operation | First (ms) | Repeats (ms) | UTF-8 response bytes |
| --- | ---: | ---: | ---: |
| Complete catalog | 183.7 | 142.7–174.1 | 3,382,747 |
| Search first page | 55.4 | 16.0–20.7 | 201,119 |
| Full focus | 283.6 | 268.6–281.3 | 3,177,556 |
| Exploration first page | 73.5 | 52.2–53.8 | 84,389 |
| Node inspection, 20 incident relations | 39.0 | 19.6–20.2 | 554,366 |
| Relation inspection | 18.4 | 3.6–3.8 | 63,392 |
| Compact Person seed lens | 269.7 | 192.0–195.6 | 612,327 |
| Compact Concept seed lens | 114.3 | 54.2–55.1 | 131,859 |

The compact canaries select `source-claims:identity:tos.agent.friedrich-nietzsche`
and `source-claims:identity:tos.crosscutting-concept.freedom`, respectively,
through `seed.node_ids`, ru, depth 1, either/all, explain, limits 48/96/20.
They return 30/29 and 7/6 nodes/relations. They are not the browser's default
focus/overview request. Search continuation was 16.7 ms, replay 13.2 ms;
exploration continuation was 68.2 ms, replay 4.7 ms. Source-carrier reads and
full-graph fallback were forbidden during the selected read probes.

The catalog and full focus are explicitly not small scene transports. The
compact byte audit attributes 35.04%/27.41% to semantics, 17.66%/24.56% to
shared-v2 HumanForm selection, and only 4.12%/3.52% to scene mappings.
All 84/15 assertion-context wrappers are distinct. Repeated inner `fields`
account for 77,672/12,816 bytes before any factoring overhead; dropping whole
contexts or mandatory qualifications is not a valid optimization. A new scene
profile requires an explicit consumer contract. HumanForm v1/v2 consumption
and the current constructor's live-data adapter remain activation gates.

Reproduction uses the explicit bootstrap above, the returned binding, and the
selected reader/search/exploration APIs; no default reader is switched.
The owner-retained evidence set `full-prepared-search3-r2-*` contains the exact
source digests, limits, build stages, queries, bindings and response digests.
Neither this baseline nor its read canaries prove addressed source-command
maintenance, D1 deployment, live UI consumption or foundation completion.
