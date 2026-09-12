# Native-v7 bounded Worker semantics

The shared helpers preserve Python values independently of a runtime. The
Worker's lens/focus route uses them from raw request and D1 row text through
bounded selection, grouping, pagination and the first wire serialization.
Node/relation inspection and temporal comparison separately retain full raw
rows through their first wire serialization. Resumable exploration preserves
native compact packets through checkpoint persistence and replay. Legacy v1 and
indexed v2 search retain native full rows and authority metadata through the
first wire serialization as well.

## Entry and reference API

| Helper | Meaning |
| --- | --- |
| `parseNativeJson(raw, limits?)` | Strict source JSON: retain number lexemes/kinds and object key order; reject duplicate names. Returns `NativeRef`. |
| `parseNativeRequest(raw, limits?)` | Request/cursor JSON: Python's last-member-wins semantics, retaining first insertion order and the final value's numeric context. |
| `nativeChild(ref, key)` | Obtain a child reference including its numeric context; accepts array positions or object keys. |
| `nativeField(ref, dottedPath)` | Python `_field`-style dictionary traversal; absent/null both yield a null value. |
| `nativeScalar(stringOrBooleanOrNull)` | Make an unambiguous literal reference. Numeric/container values require raw decoding. |
| `nativeNumberInfo(ref)` | Inspect integer/float kind and exact lexical number; never infer an integer by `Number.isInteger` alone. |
| `pythonTruthy`, `pythonEquals`, `pythonMember` | Python truthiness, recursive equality and membership, including exact int/float comparisons and false/zero equivalence. |
| `pythonStr`, `pythonRepr`, `nativeSortKey` | Python-style value strings, repr and `str(value or '').lower()` keys using retained numeric kind and object order. |
| `nativeJson(ref)` | Reserialize that preserved reference with original numeric lexemes and object key order. |
| `nativePacketObject(orderedEntries)`, `nativePacketArray(items)` | Construct immutable derived packet fragments with preserved source references. Object entry order is explicit. |
| `nativeInteger(safeIntegerOrBigInt)`, `nativeFloat(finiteNumber)` | Explicit integer/float kinds for newly computed fields only; source fields retain their original reference. |
| `nativePacketJson(packet, limits?)` | Serialize a mixed packet directly, retaining source numeric lexemes/key order and charging aggregate UTF-8 bytes, value visits and depth. |
| `nativeLower`, `nativeCasefold`, `codePointCompare` | Unicode-16 default lowercase with contextual final sigma, group casefold and code-point ordering. |

`NativeRef.value` is the ordinary JS parsed value and may contain rounded
integers, float/int ambiguity, or an overflowing large integer represented by
Infinity. Semantic operations consult the sidecar. **Do not serialize `.value`
as a lossless source record or infer semantics from it.** `nativeJson` preserves
even integer values beyond the Number range within the declared integer budget.
Python integral-float comparisons convert that exact represented float to
BigInt; an arbitrary integer is never rounded to Number to test equality.

Reference context is non-enumerable, and container metadata lives in a private
WeakMap reachable only from its document. Decoding freezes the newly parsed
tree recursively: source values and keys are not normalized or decorated, but
their property writability changes. This enforces the immutable-reference
boundary rather than asking callers to preserve it. Exposed metadata is frozen;
its container key/number maps expose only immutable lookup functions. Losing
reference context throws `NativeContextLost`; it is not a fallback to JS
semantics. Self-bound non-enumerable reference markers reject spread/clone
copies, including primitive references. These are correctness boundaries inside
trusted application code, not a sandbox against hostile JavaScript reflection.

## Legacy and indexed search boundary

`native-search-store.ts` joins ID-only search selection to bounded native row
delivery. Both public v1/v2 schemas remain unchanged. The common v8/v9 published
reader header owns source revision and authority metadata; the route does not
read a full catalog, lens histogram or graph. Selected row bytes must match
their emitted SHA-256 and structural index fields. Selected search documents
must also match Python's sorted/default-spacing JSON/lower document digest,
character count and emitted identity/display ranking fields. Those verification
strings are not substituted for the source-valued response.

Legacy exact counts, offsets, empty/short queries and source-position tie order
remain available. Ranking now uses the producer's Python-lower carrier instead
of SQLite's ASCII `lower()`, including scalar and multilingual display forms.
Its plan scans the existing kind-local search carrier and looks up source IDs
through their primary keys; it does not introduce a new index or DDL. This is
still a legacy global scan, not a bounded indexed-query guarantee.

Indexed search retains the rarest trigram route, 50000 candidate and 16000000
verification-character caps, global rank and exhausted-kind continuation. A
bounded selected posting-window check rejects missing document/stat closure,
including a false zero-stat result, before rank/source matching. Only this
window and selected source carriers are checked, not every hidden index entry.
Opaque private D1 cursors now use `tos_knowledge_search_indexed_cursor_v3`;
recognized v2 tokens return 409 and require restarting the query. Public indexed
search is still v2, not the separate local prepared compressed-search v3 ABI.
Cursor counters require JSON integer kinds and stay bound to query, filters,
source revision and publication epoch. Repeating a valid continuation does not
restart a kind already exhausted.
Generated inner and outer tokens must fit the existing 8 KiB private decode
limit before a page is emitted; otherwise 413 replaces an unusable continuation.

Python Unicode strip/lower and code-point lengths/order replace JavaScript
locale/UTF-16 approximations. Legacy checks the stripped query's 256-character
limit; indexed also checks the original and lowercased query. No NFC or casefold
normalization is introduced. HTTP filter lists preserve their literal values
(Python `_list` does not strip them); empty/repeated fields follow Python's
`parse_qs` then first-value selection. Numeric URL parameters use Python's
whole-integer/default/clamp behavior, including Unicode decimal digits.

Rows are capped at 1 MiB, digests at 1 KiB, emitted headers at 64 KiB, native
aggregate delivery/response at 16 MiB. ID selection and source/header delivery
are masked in SQL before oversized strings cross the boundary; node/relation
delivery is serialized against one remaining-byte budget. Selected search
document verification has a separate 16000000-character cap. Size/work refusals
are 413, damaged/missing publication carriers 503, invalid requests 400, and
crossed publication/stale tokens 409. No silent legacy/static fallback occurs.
Retained lone-surrogate strings or keys fail 503 at the Python UTF-8 boundary;
escaping them into otherwise valid JSON does not make a returned source valid.
Legacy count/selection is not charged against the native selected-payload
200000 rows-read quota, and neither path claims SQLite VM-step preemption.

Tests compare complete native legacy packets and every indexed page against
`search_knowledge_graph` and actual `ToSAccessCore.knowledge_search_indexed` with
`SQLiteKnowledgeSearchReadModel`, preserving ordered source members, integer/
float kinds and float repr. Runtime-specific opaque tokens and work counters
are excluded from that indexed comparison. Real D1 HTTP restart/replay, query
plans, budgets and negative admission are separate checks. Lens header/status
differences and compressed local-prepared search are outside this correction.

## Resumable exploration boundary

`native-exploration-store.ts` reads bounded identity/adjacency windows and verifies
selected full rows against emitted digests and index identity before producing
compact `nativeCarrier` packets. Arbitrary retained source fields, including
unknown semantics, numeric kinds, negative zero, unsafe integers and source
member order, remain native references. The existing compact projection still
omits attributes, source records, readable context and the Claim canonical-JSON
companion; it does not synthesize missing input. Scenes use the same bounded
structural projection as lenses, without executing a lens.

The exploration adapter serializes the native page once, before the atomic D1
checkpoint batch, with its existing 1 MiB response cap. That same JSON text is
returned on first delivery, persisted replay and concurrent CAS-winner delivery;
replay validation never round-trips it through an ordinary JavaScript packet.
Private state contains only normalized request options, string identities,
bounded integer counters, queue/depth pairs and structural origin descriptors.
Explicit key/type/range/closure guards precede cloning this private state.
No source-valued carrier enters traversal state.

The public v1/v2 schemas, `tos-exploration-d1-execution-v6`, traversal scheduling,
24 adjacency windows/512 work-unit bounds, TTL and cache capacities are unchanged.
The private checkpoint `version` now uses
`tos-exploration-d1-execution-v6/native-json-v1`. Old v6 cache records may already
contain rounded numbers: they return 409 and require a fresh start, without
rewriting/migrating/deleting source data or changing the cache-table schema.
Oversized state/replay cells are masked in SQL before delivery. Expiry, successor
admission and eviction share the batch's publication-epoch guard, so a crossed
publication does not even prune unrelated expired checkpoint rows.

Exploration alone adopts the same v8/v9 published header/index and typed
row/digest/metadata size admission as inspection/temporal comparison. Its small
exploration header must agree with the published authority/source revision;
bounded split metadata reads have clock checks on both sides, including replay.
Ordinary traversal retains the existing absent-endpoint exclusion; mandatory
origin or delivered-packet closure still refuses missing data. A completely
hidden corrupt index entry is not proven absent by positive selected-row checks.
Raw request integer fields use Python JSON integer kinds, not integral floats;
legacy focus whitespace and predicate ordering use Python Unicode semantics.

Focused tests use the actual `PublishedExplorationService` over the same SQLite
publication, comparing full-traversal selection unions, complete retained source
values/member order/numeric kinds, and each page's scene against Python. Real D1
raw HTTP restart/concurrent replay and ABA tests cover the storage boundary.
Runtime-specific page scheduling, work counts, snapshot hashes and opaque tokens
are deliberately not asserted equal. D1's 1 MiB replay cap can refuse a packet
accepted by Python's larger cache; D1 rows-read accounting is not SQLite VM
preemption. These are bounded preservation tests, not universal runtime parity.

## Temporal comparison boundary

`native-temporal-store.ts` uses the same bounded published header, emitted-row
digest and identity checks as inspection, but executes no inspection or lens.
Only the exact selected Claim IDs and their declared value/Document-subject IDs
are read. Four historical or six documentary lookup calls are sufficient; a
request-local cache avoids fetching a repeated full operand. No schema, generated
scalar index, historical assertion or inferred Claim is introduced.

`temporal-comparison.ts` carries original `NativeRef`s into `NativePacket` result
fragments. Its `_same_json` equivalent adds the temporal contract's boolean/type
distinction to exact native numeric equality. Documentary canonical digests sort
keys by code point and use Python numeric representations, not original token
spelling or rounded `.value` numbers. Returned source references keep original
numeric kinds/lexemes and member order independently of that canonical digest.
Canonicalization has a separate 8 MiB character-work allowance: a short float
token can expand in Python's representation. The accepted canonical source
companion still has the owner's 262144 UTF-8-byte limit; exceeding that limit
is an inconsistent binding, not a hidden smaller input-row budget.
The existing source profile, role, source-line kind, exact raw binding and
date-envelope rules remain the Python owner's computation.

HTTP request decoding is bounded to 64 KiB and rejects invalid UTF-8/BOM; request
selection member order and Python whitespace semantics survive normalization.
Source JSON is strict (duplicate/nonfinite/over-complex rows fail 503), including
Python's refusal to encode returned escaped lone surrogates as UTF-8. Explicit
native execution/response budgets are 413, missing selected Claims 404 and
stale source/content/publication revisions 409. Binding inconsistencies remain
ordinary `undetermined`/`unsupported` comparison packets rather than errors.

The actual published Python reader and raw Worker HTTP differential cover the
same selected SQLite rows. The D1 plan's post-statement rows-read guard, native
aggregate writer depth/visit limits and SQL pre-delivery byte checks remain
explicit bounds, not a universal equivalence claim for Python VM exhaustion or
arbitrarily large/corrupt publications. Local prepared runtime, corpus readiness,
production deployment and other query-family parity are separate claims.

## Explicit packet composition

```ts
const row = parseNativeJson(rawD1Row);
const packet = nativePacketObject([
  ['record', row],
  ['selected', nativePacketArray([nativeChild(row, 'attributes')])],
  ['count', nativeInteger(1)],
  ['score', nativeFloat(1)],
  ['ok', true],
]);
const wire = nativePacketJson(packet, {maxBytes: 1_048_576, maxVisits: 300_000, maxDepth: 64});
```

Fragments accept strings, booleans, null, original `NativeRef`s and other
explicit fragments. They reject plain objects/arrays, implicit JS numbers,
undefined, sparse arrays, malformed entries and duplicate member names.
Reserved names such as `__proto__` are ordinary names; numeric-like names keep
the caller's ordered-entry sequence. Constructors snapshot/freeze their owned
entry/item lists, not the caller's input lists, with a local 300000-member limit
that can be explicitly overridden. New derived integers reject unsafe Numbers
and negative zero; use BigInt for exact large integers, or `nativeFloat(-0)` to
retain float negative zero. Do not rebuild a source number from its lossy `.value`.

Valid immutable construction cannot create cycles. Unmarked cyclic data is
rejected at entry, mutation of fragments/source trees is rejected by freezing,
and the writer also tracks active containers. Repeated references are allowed;
each serialized occurrence pays the complete visit/byte cost. Failure returns
no partial string. No global mutable registry associates ordinary JS objects
with semantics, and no intermediate plain packet is sent through JSON.stringify.
JSON.stringify is used only to escape individual string values/keys.

## Worker integration boundary

`native-lens.ts` carries original filter references through normalization and
property binding. Matching, eager filter groups, native set-intersection
failures, Python value strings, casefold grouping and v7 float64 fingerprints
operate on references. Native int equality is exact even where v7 fingerprint
int/float coercion intentionally aliases numeric values. Pagination requires
integer JSON kinds, not merely integral JavaScript values.

`native-lens-store.ts` uses the existing v9 publication header, per-row emitted
byte digests, dimensional histograms and four ordered indexes. It rejects
incompatible Unicode/schema, damaged row/order closure and changing publication
clocks. SQL narrows identities, dimensions and incidence only; general matching
and sort/count execute before bounded selection. Fixed-length path walks allow
revisits; exhausted negative-path work fails rather than asserting absence.
Metadata streams one chunk per query with SQL type/UTF-8-length guards against
both local and remaining aggregate allowances. Selected payload pages use SQL
length/type and cumulative page-byte guards before JSON or identity text is
delivered to the Worker. Relation index endpoints must match authoritative row
headers before use, even when no relation payload will be returned.
All identity/order/header projections use SQL string-type/1 MiB-cell and
cumulative remaining-byte guards too. Delivery admission is serialized within
one request, so concurrently merged streams cannot overbook an allowance.

`native-lens-result.ts` retains source references through compact omissions,
human-form context transport and pagination. Its scene projection has only
explicit structural strings/IDs and display states; it is not a second lossy
source-bearing result. `native-lens-response.ts` writes the plain public
LensResult JSON directly with a 16 MiB ceiling. Internal `{packet, preview}`
never appears on the wire. No producer schema or new scalar index is introduced.
The stored catalog asset is byte-bounded while streaming before strict UTF-8
decoding/JSON parsing (8 MiB; absent Content-Length does not bypass it). Invalid
or oversized publication assets return 503. Native execution/response budgets
return 413 for compilation and stored-lens/focus GET/HEAD; malformed compile
input remains 400 and damaged source metadata remains 503.

`native-d1-read.ts` is the internal bounded text/payload/digest reader shared
with `native-inspection-store.ts`. Inspection is an independent v8/v9 plan:
exact normalized ID, then node entity ID, then native aliases; code-point ID
ordering; full selected rows; exact incident counts; bounded related relations
or complete unique endpoint closure. It never executes a lens, reads the catalog
or lens histograms, or rereads selected full rows to rebuild a packet. Its
`NativePacket` contains raw row/header refs and explicitly derived counts/flags;
`nativePacketResponse` is its first serialization, including for HEAD admission.

Inspection adopts the published Python reader's explicit compatibility
corrections: nonempty string `source_refs` only, incomplete endpoint closure
503, 128 alias matches, 4096-code-point IDs, Python Unicode stripping, duplicate
source-member refusal, and declared Python compact header framing. Header
framing verification never canonicalizes source rows. Row/digest/header size
budgets return 413 in inspection; the prior lens source-size statuses and
header-framing checks remain unchanged pending separate shared-owner review.

## Work budgets

Default per-document limits: 1 MiB UTF-8 input, depth 64, 300000 value visits,
4300 integer digits (current native Python default). Callers may set explicit
positive limits and must account for their aggregate decoded/callback budget.
Inspection additionally limits aliases to 128 matches, relation endpoints to
256 unique nodes, related relations to 0..1000 (default 200), reader headers to
64 KiB and row digests to 1 KiB. Per-row input is 1 MiB and per-metadata-chunk
input is 128 KiB. It shares 16 MiB aggregate delivery/response, 4096 returned
rows, 2000 statements and post-statement 200000 rows-read accounting. This is
not SQLite VM/hard-limit equivalence: at a tested 1048577-byte valid source row,
both inspectors return 413; at 2 MiB Python's earlier hard SQLite string limit
returns 503 while D1's explicit pre-delivery size guard returns 413. Both refuse
and D1 does not deliver the oversized text. No full-graph scan verifies indices.
Equality/membership have shared visit limits; repr/string/reference-only JSON
have output character limits. Mixed packet JSON has aggregate UTF-8 byte (1 MiB),
value-visit (300000) and depth (64) limits, counting the output root as a visit
at depth zero. Every child occurrence costs one additional visit; keys cost
bytes but no separate value visit. The D1 plan separately bounds 2048 generic
candidates, 32768 callbacks, 16 MiB decoded/delivered strings, 4 MiB plan sort
keys and a separate 4 MiB final-closure sort-key budget, a 64-entry/2 MiB payload cache, 100000 path steps, 4096 returned rows and
2000 D1 statements. Pages contain 16 rows. D1 `rows_read` is checked against
200000 after each statement; this is not Python's SQLite VM-step preemption
and cannot abort a statement already running. Unicode functions operate over
their supplied bounded string; final-sigma processing is linear.

## Unicode data and regeneration

`native-unicode.generated.ts` uses algorithm
`tos-python-native-unicode-v1`, Unicode `16.0.0`. The exact official source URLs
and SHA-256 values are embedded in the generated file. The Unicode license is
in `UNICODE-LICENSE.txt`. Case-sensitive authored strings are not normalized.

Download UnicodeData, SpecialCasing, DerivedCoreProperties and CaseFolding from the exact
Unicode 16.0.0 UCD directory into an owner-approved temporary directory, naming
them `UnicodeData-16.0.0.txt`, `SpecialCasing-16.0.0.txt` and
`DerivedCoreProperties-16.0.0.txt` and `CaseFolding-16.0.0.txt`. Then use:

```sh
python3 -B access/shared/build_native_unicode.py --ucd-dir /absolute/ucd-directory
python3 -B access/shared/build_native_unicode.py --ucd-dir /absolute/ucd-directory --check
```

The generator rejects changed source digests/Unicode versions or a new unhandled
context rule. It checks every code point's lower/printability/casefold/decimal against actual
Python before emitting. It never downloads data, loads a graph, normalizes
source records or publishes. No corpus-scale field index is generated.

## Focused verification

From `access/deploy/cloudflare-worker`, run the host resource route with a
768 MiB forecast around:

```sh
node --experimental-strip-types --test test/native-semantics.test.mjs test/native-packet.test.mjs test/native-lens.test.mjs test/native-inspection.test.mjs
```

Thirteen tests compare actual Python str/repr/truthiness on fixed edge cases and
5000 deterministic random finite-float attempts, mixed int/float/bool and nested
equality, full Unicode scalar lower/printability digests, contextual final
sigma, source-key/number preservation, composed Python-emitted wire values and
decoded number kinds, immutable boundaries, malformed/cyclic data, context loss
and work budgets. A separate 1471-case deterministic corpus covers exact dyadic
ties with sign/scale variants and exponent/subnormal boundaries. No shortest
float mismatch was reproduced on the tested Node/V8 runtime, so the formatter
was not changed. The current [ECMAScript shortest exponential conversion](https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-number.prototype.toexponential)
specifies an even significand when shortest alternatives tie. These are bounded
assertions, not a proof over all IEEE values, every JS runtime or whole lenses.
The lens tests use tiny Python-normalized and producer-serialized rows in a
real in-memory SQLite D1 facade. They compare complete Python-decoded wire
packets, numeric kinds, source member order and fingerprints; the actual Worker
HTTP handler is bundled for raw-body and refusal checks. Legacy fixtures use an
explicit test-only v9 publisher. No test here imports the complete corpus or
proves production Cloudflare runtime acceptance.
Inspection tests compare against the actual `PublishedKnowledgeReadModel` on
the same tiny SQLite publication, not a reconstructed lens or JS oracle. They
cover raw HTTP, source number kinds/unsafe integers/member order, exact/entity/
native aliases, refusal boundaries and publication/ABA guards, plus an actual
Miniflare D1 smoke test. Temporary test databases stay under the host TMPDIR.
