# Native-v7 bounded Worker semantics

The shared helpers preserve Python values independently of a runtime. The
Worker's lens/focus route uses them from raw request and D1 row text through
bounded selection, grouping, pagination and the first wire serialization.
Exploration, temporal comparison and unrelated inspection routes are unchanged.

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

`native-lens-result.ts` retains source references through compact omissions,
human-form context transport and pagination. Its scene projection has only
explicit structural strings/IDs and display states; it is not a second lossy
source-bearing result. `native-lens-response.ts` writes the plain public
LensResult JSON directly with a 16 MiB ceiling. Internal `{packet, preview}`
never appears on the wire. No producer schema or new scalar index is introduced.

## Work budgets

Default per-document limits: 1 MiB UTF-8 input, depth 64, 300000 value visits,
4300 integer digits (current native Python default). Callers may set explicit
positive limits and must account for their aggregate decoded/callback budget.
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
384 MiB forecast around:

```sh
node --experimental-strip-types --test test/native-semantics.test.mjs test/native-packet.test.mjs test/native-lens.test.mjs
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
