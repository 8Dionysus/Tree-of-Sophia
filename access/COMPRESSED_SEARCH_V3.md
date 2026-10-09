# Compressed search-v3: native access route and retired Python design

The current runtime owner is the installed Rust `tos-query` and `tos-access`
product. Use its CLI, HTTP, native MCP, or the thin `ToSAccessCore` SDK adapter
for prepared compressed search. The former Python `SearchStore` and
`PublishedSearchService` implementation is no longer shipped. This document keeps
its implementation history below; those details do not describe the current
runtime or supported Python APIs. The native operation and packet contracts
remain in the Rust access/query sources and `access/contracts/knowledge-api.v1.json`.

## Historical Python producer and reader API (retired)

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

Fresh queries additionally bound exact/prefix/visible candidates by the rarest
full-text term when it has fewer members. Every admitted result must contain
the complete query in serialized text, so this necessary-condition driver
cannot remove a valid hit. Actual minimal-rank and full-substring verification
still own admission; a rare gram alone proves neither. Once rank excludes a
candidate from the current phase, verification skips its irrelevant full body.
This prevents a long identifier beginning with a common three-character prefix
from walking every identity in that prefix family. No new corpus index or
publication is required, and the sorted candidate stream is never materialized.

The query hash binds `full-text-bound-v1` for fresh streams. Previously issued,
authenticated query hashes remain recognized and retain their original term
driver and verifier for the rest of their existing 15-minute lifetime. Their
predecessor is not silently moved into a different posting term. Header,
incarnation, query/filter and MAC checks remain mandatory; this is bounded
in-flight cursor compatibility, not acceptance across publications or a
fallback from a damaged new stream. Counts, page boundaries and work may differ
between equivalent plans; complete ranked results must agree.

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

Storage version **3** replaces the former `(doc_id, term_id)` reverse pair
table with one `search_document_terms(doc_id, term_count, payload, digest)` row
per document. `search_reverse_codec.py` encodes strictly increasing dictionary
IDs as canonical unsigned delta-varints, independently of document ordering.
IDs are `1..2**63-1`; at most 200000 IDs and 1800000 payload bytes are admitted.
The selected row's count, SQLite value types and byte lengths are probed before
fetching its payload. Decode rejects noncanonical/truncated varints, zero
deltas, overflow, extra terms and count mismatch. A domain-separated SHA-256
frame binds document address, kind, count and exact bytes. Every document has
at least its all-document sentinel term, so an absent/empty reverse row is an
error, not an empty old set.

Addressed updates/deletes validate old selected IDs against their dictionary
kind and sentinel in batches of at most 512. They decode only the selected
frame, retain the existing forward set differences and sort-key relocation,
and write/delete the reverse frame once. An unchanged term set and kind keeps
its frame even when order changes. The fresh private bulk producer can omit
dictionary revalidation of IDs it just resolved in the same owned transaction;
it still validates frame bounds, canonical bytes and digest. Request queries
never read this reverse table. These checks do not certify arbitrary hidden
cohort corruption or a malicious co-owner's recomputed frames.

Storage 2 and 3 are deliberately incompatible at the exact search-header
gate. This change leaves `tos_knowledge_search_compressed_v3`,
`python-lower-json-default-order-v1`, forward blocks and query/cursor shape
unchanged. Migration is an explicit offline bootstrap into a new file and a
fresh cursor incarnation; neither requests nor delta writers rewrite an older
file. Prepared-publication descriptors independently name the search storage
version through that owner's publication contract.

Delta publication is one SQLite transaction with exact expected/new binding
and fresh incarnation. Failure, allocation conflict, mutation-budget refusal or
disk-page-limit refusal rolls back content, blocks, high-water and header.
The mutation limit counts affected rows (at least one per write statement),
not merely the number of requested documents. The original database-page cap
is retained on deltas. Journal/headroom capacity remains an offline-owner
reservation responsibility; `max_bytes` limits database pages, not journal sum.

Bootstrap and delta have distinct mutation admission. `initialize_transaction`
and `publish_initial` keep their default of 2000000 mutations; an offline owner
may explicitly select a positive integer through `MAX_ADDRESS` (`2**53 - 1`).
The two delta methods keep their 100000 default and 20000000 ceiling. Invalid
bootstrap budgets are rejected before search DDL or new-file creation; invalid
delta budgets do not change the caller's transaction or page cap. An admitted
budget is still checked against actual writes, with the same rollback duty.

A full cohort accumulates term/document memberships across every carrier;
the buffered initializer still updates term counts per membership, while its
reverse frame is written once per document and each dirty block eventually
needs a block write. The delta ceiling is
therefore not a portable full-corpus size limit. A larger bootstrap budget
changes neither stored wire format nor
query, row, term, block, chunk or database-page limits. Prepared publication
also enforces its whole-publication mutation cap, including carrier writes;
the offline bootstrap receipt records the selected `PublicationLimits`.
Tiny-document tests of a larger admitted budget prove admission and rollback,
not full-corpus scalability, latency, disk capacity or memory feasibility.

Initial publication coalesces repeated writes to a bounded cache of posting
blocks. At most 8192 entries and 32 MiB of accounted retained payload are kept;
entry overhead and the current bounded insertion/sort/split workspace are not
an RSS measurement. Oversized entries bypass retention. Each term retains at
most one range, with explicit lower/upper fences. Eviction and range changes
flush dirty data; overflowing blocks use the same 128/129 split and publish
both directory ranges immediately inside the still-uncommitted transaction.
The final flush precedes `search_header`. Flush writes count against the same
mutation limit, and failure still requires whole-transaction rollback.

On a monotonic insertion into a retained range, no block read/decode, complete
key reload/sort, or per-membership payload rewrite is needed. Nonmonotonic keys
use the original exact ordering; no pre-sorted corpus, locale sort, source-order
reinterpretation, unbounded cohort map, external sort, or temporary full index
is assumed. Dictionary lookup and term-count writes remain per membership;
reverse storage is the sealed per-document frame above. Delta operations keep
the unbuffered forward writer. Fence contents/splits, query semantics and
cursor ABI are unchanged by posting coalescing.
`test_compressed_search_bootstrap.py` compares complete logical tables (except
fresh random cursor secrets), paged results, flush failures and write counters
against that original writer. Its synthetic reduction in block rewrites is
not a full-corpus timing or storage claim.

### Explicit scratch-backed bulk bootstrap

`initialize_transaction` remains the buffered initializer above. The separate
`SearchStore.initialize_bulk_transaction` is an explicit empty-store alternative,
never an automatic fallback or retry after refusal:

```python
from tos_access.compressed_search_bootstrap import BulkBootstrapLimits

# The owner already holds connection's transaction and reserves both stores.
report = SearchStore.initialize_bulk_transaction(
    connection, binding=final_binding, documents=exact_documents,
    scratch_path=exclusive_scratch_path,
    scratch_limits=BulkBootstrapLimits(
        max_bytes=32 * 1024 * 1024, max_mutations=2_000_000),
    max_bytes=64 * 1024 * 1024, max_mutations=2_000_000,
)
```

These example caps are explicit refusal bounds, not a full-corpus sizing claim.
The bulk loader reuses exact document preparation, all serialized-object terms,
identity constraints, chunking and ordering. First it writes documents, values,
the term dictionary and compressed reverse frames without incremental posting-block
maintenance. A bounded dictionary cache has at most 8192 entries and 8 MiB of
accounted retained payload by default; this is not an RSS bound. Existing
per-document term/value/text bounds still apply. The dictionary cache is released
before staging, retaining its reported high-water measurements.

It then traverses the existing `(kind, sort_key)` index and decodes one reverse
frame at a time. A temporary integer rank expresses that exact order, without
replacing source-order tokens or persistent addresses. A temporary-sort query
plan is refused before its scan. Per-term tails retain at most 255 addresses;
appending the 256th writes a complete forward block immediately. A separate
LRU permits at most `max_cached_tails` (default 8192) and `max_tail_bytes`
(default 8 MiB) of accounted retained tails. An oversized entry bypasses the
cache. Eviction writes one compressed, sealed scratch `tails` row per term,
not a row per membership. Reload validates byte bounds, digest, canonical
forward codec, total modulo 256 and strictly increasing temporary rank.
The fixed frame plus numeric addresses is accounted payload, not measured RSS.

Staging flushes retained tails; final packing visits scratch term IDs through
their primary key, writes remaining partial blocks and each term's final count
once. Packed term and membership totals must match the fresh dictionary and
producer totals before the header is written. Later block fences use the first
document's original sort key. No source
text or long sort key is duplicated in scratch. The main header follows all
stages and contains the exact algorithm, Unicode binding and storage version.
The retained `batch_size` bound (1..1024) controls scratch page-measurement
checkpoints; `max_page_count` enforces every allocation. This private transaction
never shrinks its page count, and the final flush measures its actual high-water.

Tail compression removes per-membership SQLite rows, not all per-membership
work. A very small or churn-heavy cache can still perform a scratch read/write
for almost every membership. Cache hits/misses/evictions, reads/writes, stage
duration and scratch high-water must be measured; no cache-locality or
full-corpus speed claim follows from boundedness alone.

Logical memberships/counts and the concatenated query result stream match the
buffered initializer. SQLite bytes, block/fence boundaries, write counts, work
measurements and budget-limited page boundaries need not match. The first block
uses the empty fence, subsequent blocks their first key; existing bounded delta
split/deletion rules and cursor ABI apply unchanged. The tiny bulk tests compare
full exact-object search and later deltas, not a byte-identical block history.

Scratch is a separate connection and an exclusively created 0600 file at the
explicit path. No directories are created, existing files are never adopted,
and symlink parents or existing SQLite sidecars are refused. Only this
disposable database uses
`journal_mode=OFF`; it is never a publication, recovery input or reusable cache.
It is removed on success and failure after checking its original inode. A
replacement inode is left intact and causes refusal. No `ATTACH`, main commit,
main rollback, main close or process-global temporary-directory change occurs.
After **any** exception the owner must abort the main publication; SQLite may
itself abort a failed transaction. An interrupted process may leave only an
owner-disposable scratch candidate, not a resumable or admitted store.

The scratch page cap is independent of the whole-main-database cap; its write
cap is also explicit. The overall `max_mutations` additionally charges both
main and scratch DML (affected rows, with at least one per write statement).
The report separates `main_mutations`, `scratch_mutations`, their sum
`mutations`, batch/write-call counts, cache peaks, stage durations, main page
bytes and peak scratch page bytes. It also reports `reverse_memberships`, tail
hits/misses/evictions, scratch read calls and tail-cache entry/byte peaks.
DDL/index maintenance is not a DML row
count; page caps cover their allocated storage. Disk reservation must cover
main plus scratch and the main owner's rollback journal/headroom, not just
the final file. Main `max_page_count` alone cannot cap SQLite TEMP spills.

An offline prepared owner may explicitly call this initializer at its existing
search insertion seam, passing the same final binding and document generator.
That owner must include `report["scratch_mutations"]` alongside its main
connection's `total_changes` in the whole-publication budget (do not add
`report["main_mutations"]` again). Existing prepared/offline entry points are
not switched by this API introduction. Full-corpus feasibility and activation
remain separate owner decisions after measurement and review.

Initial limits: 8 MiB per searchable/value stream, 32 MiB aggregate prepared
text, 8192 rank values, 200000 distinct terms per document and 1900000 bytes of
document metadata. Over-limit material is refused at offline publication;
every admitted large value can make query progress within byte budgets. These
are explicit service-slice admission limits, not full-corpus feasibility proof.

`storage_stats()` reports physical page/file bytes, all six data-table row
counts and per-object `dbstat` bytes when that SQLite extension exists. Its
full aggregate scan is an offline diagnostic, never a query preflight. A
separate `reverse_memberships` sum distinguishes logical memberships from the
new one-row-per-document table. The former storage-2 270-document synthetic
fixture measured 1204224 total bytes, including 802816 reverse membership bytes
and 110592 block bytes. A bounded in-memory design measurement encoded its
76866 reverse memberships losslessly into 77189 payload bytes and 98304 SQLite
bytes including per-document counts and 32-byte seals (8.167 times smaller for
that table only). The production storage-3 buffered fixture measures 499712
total bytes and the same 98304 reverse bytes. This is not a full-corpus forecast.
Insert-before changed
282/1373 blocks without other address/order-key changes. This is not a real
corpus or D1 measurement and must not be extrapolated as one.

The old Python producer/reader test suites were retired with those classes.
Current tests live in the native `tos-query` search modules and the
installed Access integration routes under
`rust/crates/tos-access/tests/selected_wires.rs`. The historical
synthetic measurements above are not a full-corpus forecast.

## Current native full-carrier prepared search

The sections above document the retired Python implementation for historical
context. Its `SearchStore` and `PublishedSearchService` classes are not installed
entrypoints.

The installed native access product owns full-carrier prepared search for
`tos_local_prepared_read_model_v1`. Its selected native query owner checks
the publication binding, reads through one verified snapshot, and returns
complete graph carriers with the `tos_knowledge_search_compressed_v3`
packet contract. Search normalization, filtering, continuation, work and
response budgets, and body verification execute in Rust. The Python SDK
only frames native `tos_knowledge_search` calls; it does not ship a
`PublishedSearchService` or another Python reader.

Select `tos knowledge search --mode compressed`, native MCP
`tos_knowledge_search(..., mode="compressed")`, or
`ToSAccessCore.knowledge_search_compressed(...)` with an explicit prepared
reader selection. Discover the selected engine through
`knowledge_search_capabilities()`, HTTP
`GET /api/knowledge/search/capabilities`, CLI
`tos knowledge search-capabilities`, or native MCP
`tos_knowledge_search_capabilities`. Compatibility mode availability
describes engine selection only; compressed availability checks its
selected publication/search headers and required objects, not every
corpus row. The default mode remains `legacy`; a prepared reader requires
an explicit supported mode and never silently creates or falls back to a
compatibility graph.

## Local adapters

On an explicitly selected prepared core, use
`knowledge_search_compressed(query, sources=..., kind_ids=...,
predicate_ids=..., cursor=..., limit=...)`. Discover engine selection with
`knowledge_search_capabilities()`, `GET /api/knowledge/search/capabilities`,
`tos knowledge search-capabilities` or native MCP
`tos_knowledge_search_capabilities`. Compatibility mode availability describes
engine selection only; compressed availability checks its selected small
publication/search headers and required objects, not every corpus row.

Select `GET /api/knowledge/search?mode=compressed`, CLI
`tos knowledge search --mode compressed`, or native MCP
`tos_knowledge_search(..., mode="compressed")`. CLI publication/binding flags
are documented in [README](README.md#explicit-prepared-local-reader). The
default mode remains `legacy`; a prepared reader requires an explicit supported
mode and never silently creates or falls back to a compatibility graph.
Offsets are not accepted in compressed mode. Preserve query/filters and pass
the returned cursor until `has_more` is false; an empty page is not exhaustion.

HTTP maps invalid requests/cursors to 400, known stale bindings to 409,
authenticated expiry to 410, hard budget refusal to 413 and unavailable/corrupt
publications to 503. CLI and native MCP preserve their normal exception/error
envelopes. HTTP and CLI encode compressed results in compact UTF-8 JSON;
native MCP uses the same compact text plus structured content rather than a
pretty-printed full carrier. Reader limits describe the logical compact result,
not the extra MCP protocol envelope containing both representations. GET request
framing remains subject to the HTTP server's request-line bound.
