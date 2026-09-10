# Compressed search-v3 service

`src/tos_access/compressed_search_store.py` is an offline-published SQLite
service slice, not an activated public search mode. Legacy search and
`tos_knowledge_search_indexed_v2` are unchanged. Core, carrier joins, HTTP/MCP,
WebMCP, Cloudflare/D1 publication and a full-corpus producer are not integrated
here. This service alone does not close foundation K3.

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
)
SearchStore.apply_delta(
    path, expected_binding=old_header, new_binding=new_header,
    changes=[SearchChange("update", doc_id, replacement)],
    max_mutations=100_000,
)
```

The initial publisher creates an absent path exclusively. Failure removes only
that new file. It accepts prepared normalized carriers from the offline owner;
it does not discover a corpus, normalize source meaning, or verify owner truth.
The owner must bind the exact carrier population, content and source order in
the supplied nonempty header. This independent header includes the search
algorithm and Python Unicode-data version, not the counter-v9 header.

`doc_id` is an integer address in `1..2**53-1`, not source identity or ranking.
Initial addresses must be unique. Each subsequently inserted address exceeds
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
reads are 8192..8388608 bytes, including reread overlap. Posting directory
lookups/decode overhead is separately reported as blocks/entries read: at most
256 prefix entries can be replayed at each phase's starting block, without a
full posting scan or query-time sort. Term selection costs at most the bounded
query's distinct grams per phase and is separate from verification work.

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
Filters are per-kind source/kind/predicate/type value lists (at most 100 each).
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
270-document synthetic test measured 1196032 total bytes: 802816 reverse
membership bytes, 110592 block bytes and the separately counted dictionary,
fence index, text, value, document and schema pages. Insert-before changed
282/1373 blocks without other address/order-key changes. This is not a real
corpus or D1 measurement and must not be extrapolated as one.

Focused checks live in `tests/test_compressed_search_store.py`: complete paged
reference equality, short/common/absent/Unicode queries, ties, typed filters,
large-value progress and chunk overlap, cursor integrity/restart/ABA, local
delta edits/rollback/nonreuse, physical storage and indexed directory seeks.
