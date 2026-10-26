# Published Worker read semantics

This is the human compatibility route for bounded Cloudflare projection reads.
The Worker retains and verifies original D1 row bytes; Rust/WASM owns shared
temporal, inspection, lens and exploration semantics. Legacy and indexed search
retain their declared compatibility behavior. These readers expose selected
derived data; none grants source, rights, review, canon or publication
authority.

The [Worker deployment guide](../deploy/cloudflare-worker/README.md) owns build,
import and deployment operations. Versioned schemas and request shapes are
indexed in [`../contracts/README.md`](../contracts/README.md). This page records
cross-operation compatibility rules and bounds; it does not describe Python as
a current runtime.

## Preserved JSON and text behavior

Published source JSON is strict: duplicate object names, non-finite numbers,
invalid UTF-8, lone surrogates at the Python-compatible UTF-8 boundary, and
over-complex records are refused. Parsed source references retain number
lexemes and kinds, object-member order, and unknown fields through bounded
packet construction. Do not serialize a normal JavaScript value as a lossless
source record: large integers and integer/float distinctions may already be
lost. Request and cursor JSON retain the declared Python last-member-wins rule.

Text compatibility uses pinned Unicode 16.0.0 lowercase/strip and code-point
length/order behavior. It does not add NFC or case-fold normalization. Source
strings and filters remain literal unless the operation declares normalization.
These rules keep selected reads reproducible; they do not alter authored
wording or provide translation.

## Search

Legacy search retains its v1/v2 response schemas, exact counts, offsets,
short/empty query behavior and source-position tie order. Its ranking uses the
published Python-lower carrier, not SQLite ASCII `lower()`. It remains a legacy
global scan and does not promise bounded indexed-query work.

Indexed search uses the rarest trigram route, with limits of 50,000 candidates
and 16,000,000 verification characters. It returns public indexed-search v2;
private D1 continuation uses `tos_knowledge_search_indexed_cursor_v3`. A
recognized older private cursor returns 409 and requires restarting the query.
Cursors bind the query, filters, source revision and publication epoch. A
bounded selected posting window is checked for document/stat closure, but the
reader does not audit every hidden index entry.

Legacy queries enforce a 256-code-point limit after Python-compatible strip;
indexed queries check original and lowercase forms. HTTP filters preserve their
declared literal values. Numeric query parameters follow the documented
whole-integer/default/clamp behavior. No reader silently switches between
legacy and indexed mode or translates offsets into compressed cursors. The
separate native prepared compressed mode is described in
[`../COMPRESSED_SEARCH_V3.md`](../COMPRESSED_SEARCH_V3.md).

## Inspection, lenses and temporal comparison

Inspection, lens compilation/focus/stored execution and temporal comparison
share Rust/WASM planning over selected, digest-verified carriers. D1 performs
bounded identity, dimension, incidence and row lookups; the Worker does not
materialize the full graph or run a duplicate TypeScript query engine. Request
shape is checked before D1 access where its contract requires it. Publication
clock and `data_revision` checks surround the read and the final body handoff.

Lens execution v7 requires the declared v9 publication metadata, row digests,
Unicode table version and ordered indexes. Older or damaged publications fail
closed. Results preserve source-number kinds and member order through compact
JSON emission. Temporal comparison uses the published `source-claims` profile;
source binding, dates, assessment and visibility follow the selected source
contract. A missing or inconsistent carrier produces the operation's declared
unavailable/undetermined result, not invented evidence.

Current response/error limits are operation-specific. Common published-reader
limits include 1 MiB per source row, 1 KiB digests, 64 KiB emitted headers and
16 MiB aggregate delivery/response. Lens metadata chunks are at most 128 KiB,
up to 256 chunks. Temporal requests are bounded to 64 KiB; their selected
publication reads and output retain their own limits. A route refuses a
selected response that exceeds its contract instead of returning a partial
packet.

## Resumable exploration

The shared Rust exploration engine owns request validation, BFS and identity
semantics, inclusion, compact scenes, state validation and replay shape. D1
serves concrete bounded SQL needs and verifies raw selected carriers before
passing their lexical bytes. Opaque tokens, cache framing, TTL, admission,
eviction and atomic continuation writes remain Worker-owned. A Worker cursor
cannot be transferred to a local native reader.

Each page has at most 24 adjacency windows and 512 graph work units, in addition
to bounded metadata/admission queries. Checkpoints expire after 15 minutes and
the cache is capped at 128 records, 32 MiB total, and 1 MiB per state or replay
response. The 1 MiB source-row and request-delivery bounds also apply. Expired or
evicted state requires a new exploration. The cache stores query continuation,
not source history or a saved workspace; cleanup removes checkpoint rows only.

The publication clock guards checkpoint admission, replay and eviction. A
changed epoch returns 409 even if the final digest matches the prior value;
expired or evicted state returns 410; oversized state returns 413; missing
migration/metadata or damaged required carriers return 503. Ordinary traversal
excludes edges with absent endpoints, while required origin/result closure
fails closed. Positive row checks do not prove a completely hidden index entry
is absent or establish whole-corpus completeness.

## Bounds and refusal behavior

Per-document parser defaults are 1 MiB input, depth 64, 300,000 value visits
and 4,300 integer digits. Search, lens, inspection and exploration add their
own candidate, statement, returned-row, cache and output bounds. The D1
`rows_read` guard runs after each SQL statement; it is not SQLite VM-step
preemption and cannot interrupt a statement already running. A byte cap is not
an aggregate host-memory, disk or CPU guarantee.

For these published routes, malformed requests generally return 400; a missing
selected resource can return 404 where the contract declares it; changed
publication or stale continuation returns 409; expired/evicted exploration
returns 410; exceeded work/byte bounds return 413; and missing or damaged
required publication data returns 503. Consult the operation schema for its
exact cases. No partial packet or alternate source is used after refusal.

## Unicode table maintenance

`native-unicode.generated.ts` is generated from the pinned Unicode 16.0.0 UCD
files. The generated source records official URLs and digests; case-sensitive
authored strings are not normalized. The active `tos-unicode-tables` generator
is native, does not download inputs, and emits the maintained TypeScript and
Rust carriers from an explicitly supplied directory:

```sh
tos-unicode-tables --root "$PWD" --ucd-dir /absolute/ucd-directory
tos-unicode-tables --root "$PWD" --ucd-dir /absolute/ucd-directory --check
```

Use the repository's validation route after changing this contract or its
generated tables. Regeneration does not read a corpus or publish data.
