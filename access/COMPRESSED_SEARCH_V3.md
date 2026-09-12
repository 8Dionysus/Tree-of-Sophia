# Compressed search-v3 service

`src/tos_access/compressed_search_store.py` is an offline-published SQLite
service slice, not an activated public search mode. Legacy search and
`tos_knowledge_search_indexed_v2` are unchanged. `PublishedSearchService` adds
same-snapshot carrier joins for the explicit local prepared publication. Core,
HTTP/MCP, WebMCP and Cloudflare/D1 activation remain adapter-owner work; this
service alone does not close foundation K3 or establish full-corpus feasibility.

## Producer and reader API

```python
prepared = PreparedSearchDocument.from_item(
    doc_id, "node", exact_normalized_item, source_order_token
)
SearchStore.publish_initial(
    path, binding=owner_snapshot_header, documents=prepared_documents,
    max_mutations=2_000_000, max_bytes=64 * 1024 * 1024,
)
store = SearchStore(path, binding=owner_snapshot_header)
page = store.query_page(
    kind="node", query="a", filters={"kind_id": ["concept"]},
    page_size=50, cursor=None, candidate_budget=256,
    verification_bytes=65536,
    max_metadata_bytes=4 * 1024 * 1024,
    max_response_bytes=4 * 1024 * 1024,
)
SearchStore.apply_delta(
    path, expected_binding=old_header, new_binding=new_header,
    changes=[SearchChange("update", doc_id, replacement)],
    max_mutations=100_000,
)
```

The same algorithms are exposed to the internal prepared-publication owner
through class methods taking an existing `sqlite3.Connection`:

```python
SearchStore.initialize_transaction(
    connection, binding=owner_snapshot_header, documents=prepared_documents,
    max_mutations=2_000_000, max_bytes=64 * 1024 * 1024,
)
SearchStore.apply_delta_transaction(
    connection, expected_binding=old_header, new_binding=new_header,
    changes=typed_changes, max_mutations=100_000,
)
page = SearchStore.query_transaction(
    connection, binding=owner_snapshot_header, kind="node", query="a",
    filters={"type_id": ["concept"]}, page_size=50, cursor=None,
    candidate_budget=256, verification_bytes=65536,
    max_metadata_bytes=4 * 1024 * 1024,
    max_response_bytes=4 * 1024 * 1024,
)
```

All three methods require `connection.in_transaction` to be true. They never
begin, commit, roll back, close, or issue a savepoint on that connection. Schema
creation uses individual DDL statements; `executescript` would implicitly
commit caller work. The owner can write carrier rows and search data in the
same main database and commit them together, or roll back both. Initialization
returns `mutations`, `blocks_written`, `payload_bytes_written`, and whole-file
`database_bytes`; a delta returns the first three counters. These counters
cover search writes, not carrier writes. Query returns the unchanged page
packet and validates its framed header and cursor incarnation in the passed
transaction's snapshot. It can read a coherent publication written earlier
in that same transaction. The owner must perform its carrier/header reads in
that transaction too. The query never opens a second connection or computes
an owner digest.

`max_bytes` is a cap on the **whole main SQLite database**, including carrier,
schema and index pages. It is not a separate search allowance. The effective
cap is the smaller of the requested page count and the connection's current
`max_page_count`; an already oversized database is refused. The stored cap is
reapplied on deltas without increasing a stricter caller cap. A stricter
effective delta cap is persisted in `search_header.max_pages`, so later
standalone search writers also retain it after reopening. Owners reopening
connections must enforce their own whole-file cap before doing carrier writes;
SQLite's pager limit is connection state, not durable quota enforcement for
arbitrary writers. Pager-limit changes are not promised to roll back with SQL
data. Journal/WAL/headroom reservations are separate from the main-file cap.

Any transaction-method failure makes the owner's entire publication attempt
failed; the owner must abort it and never commit partial work. The methods do
not secretly roll back on the owner's behalf. SQLite itself can abort a
transaction on `SQLITE_FULL` or another engine failure, so the owner must also
check `connection.in_transaction` when cleaning up. A connection remains open
on success and failure. Standalone `publish_initial`, `apply_delta` and
`query_page` retain ownership of their own connection lifecycle and use the
same implementation.

Typed exceptions provide an internal adapter mapping, without activating HTTP:

| Exception | Meaning | Suggested HTTP status |
| --- | --- | --- |
| `SearchInvalidRequest` | Invalid request, typed change or missing open transaction | 400 |
| `SearchStaleBinding` | Explicit selected binding or pinned reader incarnation mismatch | 409 |
| `SearchCursorError` | Malformed cursor, wrong query or failed cursor authentication; restart query | 400 |
| `SearchCursorExpired` | Authenticated cursor past its absolute expiry | 410 |
| `SearchUnavailable` | Missing, unreadable or corrupt publication | 503 |
| `SearchBudgetExceeded` | Publication mutation/page/data cap or hard response framing refusal | 413 |

All are `ValueError` subclasses; `SearchCursorExpired` also derives from
`SearchCursorError`. Unavailable and budget errors additionally preserve
`sqlite3.DatabaseError` catches. Continuation under a normal per-page work
budget remains a successful packet, possibly empty, rather than a budget
exception. A failed MAC cannot distinguish tampering from an old publication;
it is a cursor error, not evidence of staleness. A known outer publication
binding mismatch can be rejected separately before querying. The existing
cursor shape and HMAC remain unchanged, including restart survival and ABA
rejection.

The initial publisher creates an absent path exclusively. Failure removes only
that new file. It accepts prepared normalized carriers from the offline owner;
it does not discover a corpus, normalize source meaning, or verify owner truth.
The owner must bind the exact carrier population, content and source order in
the supplied nonempty header. This independent header includes storage version
2, the search algorithm and Python Unicode-data version, not the counter-v9
header. Earlier unversioned service fixtures must be rebuilt; they are not
silently admitted with missing identity constraints or old filter framing.
The complete framed header is capped at 65536 UTF-8 bytes. The separate framed
query/kind/filter input is also capped at 65536 bytes before hashing or reads.

`doc_id` is an integer address in `1..2**53-1`, not source identity or ranking.
Initial addresses and exact `(kind, identifier)` identities must be unique;
case variants remain distinct. Duplicate source identities are refused, never
silently deduplicated or admitted under another address. Each inserted address exceeds
the stored high-water mark; deleted addresses cannot be reused. Updates retain
their address. A delta lists each address once and new addresses in ascending
allocation order. Address kind/source-identity changes must be explicit owner
updates, never guessed correspondence.

`source_order` is a nonnegative safe integer, meaningful only among equal
`str(id or "").lower()` values. The producer assigns sparse tokens from the
actual reference source order. Tokens are not an arbitrary ranking control.
Inserting between ties uses an available token; exhausted token space requires
an explicit bounded group-retokenization delta. No global ordinal renumbering
occurs. Duplicate `(kind, lower-ID, source_order)` is rejected.

## Matching, ordering and continuation

The query follows reference `str(query).strip()`, a 256-codepoint check, then
Python `.lower()`. Expansion by lower is allowed. Searchable text is exactly
`json.dumps(item, ensure_ascii=False, sort_keys=True).lower()` with default
separators. There is no casefold, NFC, punctuation removal or SQL lower.

Four candidate phases preserve exact, prefix, visible, technical rank. Exact
identities have whole-value keys; prefix keys contain the leading 1/2/3
codepoints only; visible and technical planes contain all distinct 1/2/3-grams.
For longer substring queries, the rarest indexed gram selects an ordered
candidate stream. Full reference substring verification and actual minimal
rank discard false positives and cross-phase duplicates. Empty query uses a
rank-3 all-document stream. Python-codepoint byte keys preserve lower-ID order,
including prefix, NUL and astral cases, followed only by the source-order token.

Results contain `schema: tos_knowledge_search_compressed_v3`, `matches`
(`doc_id`, exact typed `id`, `rank`), `returned_count`, `total_matching`,
`has_more`, `next_cursor` and measured `work`. These are addresses for a later
snapshot-bound carrier join, not complete source-return packets.

`has_more` means candidate work remains, not that another match is proved.
An empty page with continuation is valid. Page size is 1..100; the work budget
is 2..4096 operations (candidate checks plus value/chunk checks). Verification
reads are 8192..8388608 bytes, including reread overlap. Metadata has a separate
4194304-byte minimum/default and 67108864-byte maximum. Actual header/key,
document JSON-ID/filter/sort-key and compressed-block bytes are charged before
admission. Narrow length probes precede variable-width fetches. Integer-only
probes and query-control reads have separate counters rather than pretending
their SQLite/Python object overhead is a payload-byte measurement.

The starting block can decode at most 256 prefix entries, then locates the
exact predecessor address and skips the prefix without loading its document
metadata. Only the predecessor's key is read once. A missing predecessor in its
bound block is corruption, not permission to scan forward. Fence bytes stay in
SQL; directory probes, blocks decoded and posting entries are reported
separately. The 4-MiB minimum covers the maximal framed header, predecessor
key and one admitted metadata row, ensuring a new singleton can progress.
Term selection costs at most the bounded query's distinct grams per phase and
is separate from verification work. No full posting scan or query-time sort
is introduced.

Response framing has a 4194304-byte default, 1908192-byte minimum and 16777216-byte
maximum, measured as UTF-8 `json.dumps(..., ensure_ascii=False, sort_keys=True)`
including the cursor and accounting fields. Its minimum admits every supported
single ID without narrowing IDs. A result that does not fit after earlier
matches remains unconsumed with an integrity-bound `matched` continuation;
the next page need not reread its already verified long text. A later full
carrier join must account for its own response/row budgets separately.

The cursor is compact even for large IDs: its predecessor address resolves to
the bound stored order key. It preserves phase, within-document value/byte
progress and a 15-minute absolute expiry, and is HMAC-bound to the store's
private publication incarnation and exact query/filter/header combination.
It is reusable across process restarts, not across publications. A publication
rotates the incarnation, so old readers/cursors fail even after external
binding ABA. SQLite read transactions pin the checked header and all page reads.
No request computes a graph digest or loads the corpus.

`returned_count` is always exact. `total_matching` is exact only when an initial
request exhausts the whole kind stream; it is null on every continued page,
including the last. Consumers may sum the returned counts from a full traversal.
Filters are per-kind source/kind/predicate/type value lists (at most 100 each):
node fields are `source_graph`, `kind_id`, `type_id`; relation fields are
`source_graph`, `predicate_id`, `relation_type_id`. There is no `node_type_id`
alias.
Empty lists do not filter. Values remain typed: false, zero, string-zero and
null are not coerced into one source value. Public adapters still own reference
source-name validation and default source selection.

## Storage, deltas and bounded publication

Each term has a B-tree directory of stable range fences and compressed ordered
doc-ID blocks (maximum 256, ZigZag delta-varints). Splits affect one term's
overflowing block. Reverse memberships address deletions/updates without a
whole-store scan. Unchanged memberships are not rewritten for a text-only
change with unchanged term set/order. Text and rank values use 32-KiB chunks.
Empty fences and zero-count terms remain as reusable directory history; a
partial nonempty-block index makes query seeks skip them. Reclaiming that
history is a future bounded owner maintenance route, not implicit compaction.

Delta publication is one SQLite transaction with exact expected/new binding
and fresh incarnation. Failure, allocation conflict, mutation-budget refusal or
disk-page-limit refusal rolls back content, blocks, high-water and header.
The mutation limit counts affected rows (at least one per write statement),
not merely the number of requested documents. The original database-page cap
is retained on deltas. Journal/headroom capacity remains an offline-owner
reservation responsibility; `max_bytes` limits database pages, not journal sum.

Initial limits: 8 MiB per searchable/value stream, 32 MiB aggregate prepared
text, 8192 rank values, 200000 distinct terms per document and 1900000 bytes of
document metadata. Over-limit material is refused at offline publication;
every admitted large value can make query progress within byte budgets. These
are explicit service-slice admission limits, not full-corpus feasibility proof.

`storage_stats()` reports physical page/file bytes, all six data-table row
counts and per-object `dbstat` bytes when that SQLite extension exists. Its
full aggregate scan is an offline diagnostic, never a query preflight. A
270-document synthetic test measured 1204224 total bytes: 802816 reverse
membership bytes, 110592 block bytes and the separately counted dictionary,
fence index, text, value, document and schema pages. Insert-before changed
282/1373 blocks without other address/order-key changes. This is not a real
corpus or D1 measurement and must not be extrapolated as one.

Focused checks live in `tests/test_compressed_search_store.py`: complete paged
reference equality, short/common/absent/Unicode queries, ties, typed filters,
large-value progress and chunk overlap, cursor integrity/restart/ABA, local
delta edits/rollback/nonreuse, physical storage and indexed directory seeks.

## Full-carrier publication search

`PublishedSearchService(reader, limits=PublishedSearchLimits())` in
`src/tos_access/published_search.py` provides the shared internal service for
`tos_local_prepared_read_model_v1`. Its reader must already carry the exact
owner-selected `published_snapshot_binding`. `capability()` checks the selected
reader/search headers and required search/mapping tables and indexes without
reading a graph or full catalog. Edge v8/v9 are explicitly unavailable for this
service; none of these methods select a publication from its own database.

```python
service = PublishedSearchService(reader)
page = service.search(
    query="common", sources=["philosophy"], kind_ids=None,
    predicate_ids=None, cursor=None, limit=40,
)
```

Requests use `normalize_search_query` and validate string lists, known sources,
limits and the 65536-byte query/filter frame before opening SQLite. Empty string
filters are ignored as in native search; absent/empty sources select all known
sources. The limit is an integer in 1..100, per kind. The transport normalizer's
256-character bound includes lower expansion; standalone `SearchStore` keeps
its separately documented query normalization contract.

One `reader._read` operation supplies one checked connection/snapshot for both
`SearchStore.query_transaction` calls and all carrier joins. Every address is
looked up with both `doc_id` and `kind` in `prepared_documents`; a length probe
precedes the exact mapped ID. Its SHA-256 must equal the search match's exact
JSON-ID digest. Body/index-column lengths and checksum metadata chunk lengths
are probed before full rows. `read.items(kind, 'id=?', ...)` then verifies the
existing emitted-byte checksum and exact full-row/index closure. No alias
lookup, corpus digest, graph load, producer or alternative reader is used.

The result schema remains `tos_knowledge_search_compressed_v3`, with complete
unmodified `nodes`/`relations`, `source_revision`, normalized request filters,
`page`, `counts`, `ranks`, `work`, and the checked source authority boundary.
`ranks.nodes` and `ranks.relations` align positionally with their body arrays;
each entry carries `doc_id`, rank 0..3 and a mechanical explanation
(`exact-identity`, `identity-prefix`, `visible-text`, `serialized-text`, or
`all-items` for an empty query). These explanations are search ordering, not
semantic judgments. Returned counts are exact; matching counts are non-null
only when the initial request exhausts that kind and emits all of its pending
bodies. Continued pages always have null matching totals.

Each kind performs at most one inner search page per request. Defaults are
256 candidate/value operations, 65536 verification bytes and 4 MiB search
metadata per kind. If matches exceed the body/fetch allowance, the unconsumed
suffix is retained as `[doc_id, rank, sha256(exact_json_id)]` tuples, at most
100 per kind. Later requests drain that suffix before doing another inner
search. They do not reread already emitted bodies or repeat the saved search
work. Empty candidate-work pages with a changing continuation remain valid.

The outer cursor is base64url JSON authenticated with a separate HMAC domain
using `search_header.cursor_key` from that same checked snapshot. It binds the
complete selected publication, normalized query/filters, both inner cursors,
exhaustion states and pending tuples. A canonical sorted-key hash makes
equivalent binding dictionaries insensitive to JSON member order after restart;
emitted-row framing is unchanged. The cursor expires absolutely after 15 minutes
and is at most 65536 encoded ASCII bytes. Hashed exact IDs keep giant identities
out of the continuation. A different selected binding is stale; a bad MAC alone
is an invalid cursor and cannot establish whether the cause is ABA or tampering.

The default body allowance is 4 MiB, partitioned into 2 MiB per kind. It is
further bounded by the configured reader response limit after reserving 512 KiB
for incoming/outgoing cursors, filters, authority metadata, ranks and top-level
framing. Each kind must be able to return one maximal admitted reader row; an
incompatible budget is refused without inflation. The remaining fetch-byte and
row budgets are split evenly after reserving 66560 bytes and 513 rows for the
reader's final snapshot check. A kind's fetch share must admit its configured
inner metadata/verification allowance and a worst-case singleton carrier join;
otherwise the request fails with `SearchBudgetExceeded`.

Direct search metadata/text bytes are charged into `read.bytes` alongside mapped
IDs, duplicate index columns, full bodies and checksum metadata. The service
stops before fetching a body that would exceed its allowance and retains the
pending match. `work.nodes`/`work.relations` include inner search counters and
body/defer counts; top-level read counters describe the operation before the
reserved final snapshot check. Search-owner row/probe counters remain separate
from `_Read.query` row accounting. The reader validates the entire serialized
response and performs its post-operation publication check, rejecting a result
if publication changed during the read. Current checksums detect byte drift;
they do not defend against a malicious owner rewriting all bound metadata.

Actual-file checks in `tests/test_published_search.py` compare full native
rank/filter/ID/order results, restart and empty-work continuations, long text,
lossless body and fetch-byte deferral, mapping/checksum/index corruption,
concurrent/stale publication, HMAC/expiry and explicit legacy refusal. They use
the publisher owner's bounded synthetic fixture, not a full source corpus.
