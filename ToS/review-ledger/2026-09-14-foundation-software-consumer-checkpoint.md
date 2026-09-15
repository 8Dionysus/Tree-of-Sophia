# Foundation software consumer checkpoint · 2026-09-14

This is a bounded integration review, not Foundation v1 acceptance, corpus
admission, CI, merge, or deployment evidence.

### Native publication race follow-through · 2026-09-15 UTC

The adjacent concurrent-publication check exposed a second concrete gap: native
Worker descent/dossier could accept an otherwise checksum-valid response across
an A→B→A publication. The regression failed with `Missing expected rejection`
before the fix. Both routes now use the existing revision-plus-epoch guard
already used by search, inspection and lenses; no second clock or request-time
migration is introduced. The guarded routes refuse the interleaved ABA case.
Typecheck and all six native-source tests pass (58.528 s).
Actual Penn descent and dossier still match Python exactly after this change;
their packet hashes are unchanged. The two affected route observations were
repeated because their execution boundary changed, not the completed corpus
mapping, semantic review, private reads or unrelated cost checks.

The human tab also searched and selected the Penn Work after publication,
loaded 11 nodes / 12 relations, then opened its native sources panel. Work,
expression, 1920 edition, DjVu file and item remain distinct; the displayed
notice separates link availability from rights and says rights are unspecified.
Both previously pinned reading items survive. This closes the G6/G7/K2 human
selection→source panel join on the integrity-protected publication, not JGB21's
missing exact text authority. Evidence: `native-integrity-human-source-r1.json`
SHA-256 `d11837d73b9143b77c99b1ee034c674b5207cce5ab095f4cb28ba3be5dbbc311`;
`published-native-navigation-comparison-r3.json`
`7162c50aed53067c84b122824365b7c16b4abd36929b26b74fa50c33b16af90c`.
Fresh corrected-head CI and landing remain required.

## Native navigation integrity and publication seam · 2026-09-15 UTC

The union's initial exact-head CI passed, but review found a real P1: a native
navigation record's full JSON, including rights judgment fields, could drift
while its indexed selection columns stayed unchanged. Both Python and Worker
now require producer-emitted SHA-256 companions for every node, edge and rights
row, checking exact inline or hydrated UTF-8 JSON before use. Missing companions
refuse and require explicit product migration; readers never attest their own
unverified persisted input. The full, initial-product and addressed-delta
producers maintain the same contract. Delta capture rejects missing, wrong and
orphan predecessor companions and retains exact predecessor framing for reversal.
Checksums detect carrier drift, not a malicious publisher controlling both
content and checksums; source, rights and publication authority remain separate.

The bounded migration independently compared all 27,114 native nodes, 39,766
edges and 127 rights rows with the admitted immutable navigation and rights
snapshots. It emitted only checksums and publication metadata: no corpus,
normalized knowledge, native row or prepared-source rewrite, and no database
copy. Capture took 18.186 s, with 344.3 MiB peak and zero swap; both SQL directions
total 26,880,029 bytes, under a reserved 128 MiB write envelope. Forward and
reverse SQL are retained in managed scratch; durable receipts are outside it.

The exact task-owned Worker was stopped for one local SQLite transaction and
relaunched. A wrapper postcheck incorrectly expected the old epoch and reported
failure after the import had committed. Readback established the actual result:
67,007 checksums, unchanged source and normalization identity, new publication
`af09d41589cf74648742e32eeed1b3eb893ad3e5645dfaed68a39ec20df35209`,
and epoch 5→7 from the existing data-revision delete/insert triggers. This is
intentional invalidation of disposable query state, not a source change. The
old snapshot binding refuses; the current catalog and both auxiliary lens-store
bindings are valid. No second import or manufactured success receipt was used.

On real Penn material, source descent still returns 2 nodes / 1 edge and the
work dossier retains its scoped rights. Python and Worker packets are exactly
equal: descent 2.414 / 72.726 ms, dossier 6.176 / 181.085 ms, respectively.
These are bounded observations, not latency percentiles or production claims.
This closes the G6/G7/K1 native-read integrity and publication/auxiliary-store
join. Remaining software gate: reviewed corrected exact-head CI and landing;
remaining K2 exact JGB21 text authority/coverage is not closed by this fix.

Verification: Python native-reader 8 tests, Worker native-source 5 tests and
typecheck passed. The producer/bootstrap/delta suite passed 19/20 initially;
the remaining oracle incorrectly compared physical hashes across semantically
equal JSON with different key order. It now independently verifies each
emitter's exact hashes before comparing semantic rows, and its targeted rerun
passed. The suite includes migration staging/replay/exact reversal and refusal
to sign a rights row differing from its immutable source. A bounded independent
Luna producer review found no remaining deterministic defect; source and
security judgment remain with the master.

Durable evidence: `native-integrity-capture-r1.json` SHA-256
`7bddcfae04af93df42c7c521e6539ea09ec1eed6d0ed9c10fedacb0d8bfa0841`;
`published-native-navigation-comparison-r2.json`
`56495fd7f01d2812c715e73d5709d04a20707f379fc738e7be5dd7720c742d1a`;
`native-integrity-publication-readback-r1.json`
`34053bacf4244736037c37ee58f8c9efcab13d867de2a0c5e15ecb2058259ff3`.
This checkpoint does not grant rights, publish source payloads, or accept the
whole Foundation goal. A checksum-less retained product needs this explicitly
source-paired migration or an already planned compatible full production; a
software merge does not silently upgrade or admit another owner's snapshot.

## Two-Conception reading and cost reconciliation · 2026-09-15 UTC

The actual D1-backed human reader now holds the JGB19 experienced-commanding
and JGB21 metaphysical-self-origination Conceptions side by side in the existing
generic reading shelf. Their Russian substantive names resolve through search;
selection loads 8/7 and 10/9 node/relation carriers respectively. Each retains
its own source-bound description, provisional identity and non-admission
notice. The descriptions distinguish experienced agency from a target of
criticism, without attributing the latter as Nietzsche's endorsed doctrine.
Selecting English on the second card leaves the first Russian, uses the second
card's exact English name and explicitly reports the absent English description
with its available Russian fallback. A screenshot verifies the contextual
two-column shelf over the existing graph; the pinned first item survives the
second focus. This is comparison by reading, not an automatic semantic verdict
that the items are equivalent or competing.

This closes the G5/G6/G9 comparison/language-selection interaction seam.
Known structured scope, unclassified fields and absent form roles remain
visible as such, not fabricated prose or accepted content. The earlier exact
agent/source checks and JGB19 private owner-process text return are reused
separately. This observation neither extracts JGB21 nor transmits a private
TextUnit through HTTP/MCP/UI. The exact-text portion of the JGB21 argument
branch remains open; the existing JGB19 unit is not substituted for that text.
Evidence: `jgb-conceptions-reader-comparison-r1.json`, SHA-256
`5783e40592740df84a4ca3ac25d07f7bca0e54e2946dda1de4317a0c41575bb3`.

The K3 review separates foreground reading from explicitly launched background
publication. Existing observations are sufficient for the bounded v1 cost
claim below; repeated full builds or destructive source deletion would not
strengthen it.

| Work class | Measured evidence already retained | Accepted boundary and residual risk |
| --- | --- | --- |
| Prepared catalog/search/focus/explore/inspect | Fresh-process / repeated core calls: 102.039/86.494, 29.875/11.414, 55.232/28.263, 44.229/23.389, 5.585/6.015 ms; process setup separately 314–459 ms, peak 100.3 MiB, zero swap. | Process-cold, not cold OS cache, HTTP or paint. Real packets and 42,674/62,693 retained node/relation carriers, not a tiny graph. No p95 service promise follows from one pair. |
| Local D1 human/agent reads | Earlier full-store catalog 1,631 ms; exact inspections 192–206 ms. Scoped freedom focus first/repeat 685/606 ms; stored-route lens 6,916/6,062 ms, with exact Python parity. | Accept bounded synchronous reads under the existing 60 s client cancellation ceiling, not a claim that 60 s is desirable latency. Simple observed routes are sub-two-second; complex 1.36 MB route output is slower and remains cancellable. No production/network latency claim. |
| Source correction, assessment/access change, derived deletion and recovery | Exact source/assessment timings and actual withdrawal/restore in the preceding lifecycle table; native positive note read 5.21 s, expired/outside scope refused. | Source history is preserved; derived absence, permission refusal and judgment withdrawal are different operations. No fabricated timing of model reasoning or grant issuance. |
| Addressed background publication | Penn metadata addition 24.588 s; three-Claim batch 61.158 s / 751.6 MiB / zero swap; D1 catch-up 5.172 s; compact reader-header transition 3.427 s. | These are explicit jobs, not UI keystroke response budgets. The 100,000-mutation refusal preceded an explicitly bounded 500,000-mutation run; no hidden full rebuild or unlimited retry. |
| Initial full publication | Prepared build 1,409.173 s / 5,260,079,104 bytes; D1 import 1,320.399 s / 1,008,590 SQL statements. Peak wrapper costs about 6.5/7.6 GiB, with observed swap. | Heavy offline initialization, not a small-change path. Its RAM/disk cost requires the host's reservation and resource route, not parallel duplicate corpus copies. It is not a foreground performance claim. |
| Growth and bounded work | Existing 5,000→10,000 unrelated-row/high-degree query checks, plus 511→1,022 compressed insertion/update/deletion oracle and block-locality checks. | Deterministic row/byte/VM/mutation bounds survive these growth cases. No assertion of universal constant-time SQLite IO or model assessment. |

The enforced budgets are part of the readers, not only this report: prepared
row/metadata/response limits of 1/8/16 MiB, 4,096 returned rows and 200,000 VM
steps; compressed page candidate/verification limits of 256/65,536 with bounded
metadata and body delivery; D1 indexed verification of 16 million characters;
and finite lens/exploration candidate, decode, cache and retained-state limits.
Compact and exact inspection have separate budgets; oversized, malformed,
stale and insufficient-budget requests refuse rather than truncate truth or
silently fall back to corpus construction. TTL, bounded caches, cancellation,
same-snapshot restart and stale-cursor refusal are independently tested.
These limits protect request work and retained storage, not total physical IO
or an unlimited corpus's total size. Larger future workloads must revise an
explicit budget and rerun its affected boundary checks.

K3's required bounded observations are now reconciled, including the real
local/D1 seam and failure/recovery. Their acceptance is conditional on the
reviewed software passing exact-head CI; it does not close the remaining K2
text branch or K1 landing. The first union CI found one failing Worker test
while 355 passed: its old over-budget fixture also corrupted rank JSON and
therefore received a publication-error 503 before the requested-window 413.
The test now changes only document length, leaving rank metadata intact;
the exact 413 assertion is retained. Its targeted real-Miniflare run passed
in 26.94 s. Separate malformed-carrier tests retain fail-closed coverage.
The failed remote run is not counted as success; corrected exact-head CI
must pass before landing.
Cost evidence: `k3-retained-prepared-warm-benchmark-r2.json` SHA-256
`7e897be37d41d1dbde3fb71234bf11d19a206501fcb9fc904cd04b2d4c10ad75`;
`d1-scoped-lens-live-r3.json`
`710c0b9e7286ab1b76e85a3b5956b4a1668b764b5a6210ae373675a53935537d`;
`lens-cursor-real-seam-r1.json`
`5acf8a5f99cb0c7a41d6c84524ff58b1504425a03d32db6ccb27a2af816dcdae`.

## Ordinary word search and bounded verification · 2026-09-15 UTC

The real `Wille` query exposed a second search boundary: rare-gram intersection
alone cannot make a common word's full candidate text fit in one request.
The Worker now verifies a rank-ordered candidate prefix within the unchanged
16-million-character budget, including rank metadata. A materialized prefix
precedes native text joins; continuation advances through verified candidates,
including a prefix with no complete-string match. Global rank/ID/position,
publication binding and exhausted-kind behavior are retained. An individually
oversized next document remains an explicit refusal, not a silently skipped
match. No index, schema, source, grant or D1 data was changed.

Actual human search now shows `Wille` as a lexeme and the separate `Wille` /
`Willen` spellings. Selecting the lexeme loads 7 nodes / 6 relations. Agent
inspection returns the same exact ID and source path. The human relations tab
exposes the separate `lexical_sense_of` Claim linking the contextual Sense,
not a direct assertion that a philosophical conception is a dictionary meaning.
Its bounded JGB19 proposal, uncertain/unreviewed status, source-reading basis
and absence of exact token/paradigm authority remain visible. Two WebMCP search
pages each return 6 nodes / 6 relations without repeated node IDs; both retain
source revision `61e5059bcff97b455f04e4aab92d6edb6a5bc5b33a37e059ec3d80e6f2acba26`.
The first HTTP node window covers 91 candidates / 15,563,210 document characters
plus 90,426 rank characters; the relation side covers 147 / 1,755,003 plus
162,032 rank characters. These are bounded logical spans, not total repeated
SQL visits or physical IO measurements.

This closes the observed ordinary-word search failure and checks the
G6/G9 human/agent lexical-identity/context/continuation seam plus G7 bounded
verification. It is not a complete corpus-search oracle, semantic admission,
a new latency percentile, or completion of K2. Whole-goal acceptance and
exact-head software landing remain separate.
Type checking and all 23 native-search tests pass (28.455 s), including real
D1 isolate restart, Python packet parity, stale/ABA cursor refusal and the
new wide-prefix regressions. The latter use three real in-memory synthetic
source/search carriers, not missing-native placeholders, and compare complete
per-kind result streams across empty progress pages. Window boundary IDs
also retain the existing 1 MiB SQL-side delivery mask.

Evidence: `wille-indexed-human-agent-r1.json`, SHA-256
`e97011057ca3b9ee08f76c9a3208edbedca699bf102f871eac76fac2b77047be`.

The next human constructor check found a harness omission, not a catalog
failure: the local Worker configuration had no `ASSETS` binding, so
`/api/knowledge/contracts` returned 500 while the native catalog returned
200 / 3,409,965 bytes. Only the two normal contract assets were exported
from the selected local Python owner's routes, compared against all 16 exact
current source files, then attached to the existing Worker configuration.
They total 676,739 bytes; no production configuration, data or database copy
was changed. `local-contract-assets-r1/receipt.json` records each source/asset
digest. The repaired contract route returns the 12-contract bundle.

The actual constructor then discovers `Тип сущности` / `равно` and the
`tos.entity.lexeme` value. Applying it to the seven-carrier Wille area yields
2 carriers / 1 relation with an explicit delta of −5/−5; an empty intermediate
condition preserves the previous view rather than claiming a result. Returning
to the original view restores its focus and selected Sense-to-lexeme Claim
relation. This verifies G6/G9 condition discovery, bounded filtering and view
return on real data. Counts describe carriers, not two different lexemes.

## Current catalog and owner-separated completion · 2026-09-15 UTC

The source owner's `render_outputs` / `check_outputs` reports no stale catalog
companions: 312 objects and 377 Claims, 689 identities. A subsequent complete
enumeration compared each exact public source record against the selected
prepared reader's indexed subject carriers with the existing
`observe_record` comparison. All fields and source-return references match:
659 direct mappings, 30 native adapters, 411 bound source files. Source
membership, bytes and the prepared publication were rechecked at the end.
The observation took 13.461 s / 318,328 KiB peak process RSS, without graph
construction, a database copy or source writes. It binds source revision
`61e5059bcff97b455f04e4aab92d6edb6a5bc5b33a37e059ec3d80e6f2acba26`
and prepared data revision
`85030b1fb6d05c841cc443e8faa24f33a3b0834fda637c82738f24640437ab2e`.
This refreshes the public catalog part of K4, not private/uncatalogued
inventories, semantic assessment, historical versions or form quality. The
addressed check enumerates exact/entity/native indexed subjects; it is not
a full scan for conflicting identities hidden only in extension attributes.

The corpus migration owner has preserved the earlier Foundation input
`0f017572666e23f3d383f5dbce30e76ef6b21842`. The exact successor handoff through
`303aef986bc8431f47b66f753f70acdbf23219c9` contains 21 changed tracked ToS files,
897,620 successor bytes and no deletions. Before/after Git blobs and SHA-256
values identify every member without copying the corpus. Penn environment,
three Claims, forms and creation evidence remain distinct from catalog
companions; the latter must not overwrite the recipient's union catalog.
The recipient acknowledged this as **queued, not admitted**. Its mass snapshot
and fresh remote restore are separate owner work, not this local D1 result.
The recipient subsequently confirmed that this frozen, not-yet-activated
corpus migration is **not a Foundation K1 gate**. The actual Foundation source
additions are already in current Git `main`; the pending union differs in ToS
only by this checkpoint and the coverage map. Current Git source and our
verified prepared/D1 bindings remain the active Foundation path. The queued
successor does not imply a common currentness claim for its separate snapshot.

K1 follows the current software/corpus/integration release separation. Exact
source-batch admission and snapshot binding still matter; a general KAG or
documentation reseal is not a dependency of every software change. New review
bytes after the handoff retain their own next delta; this note does not claim
to be included in an earlier immutable packet.

Existing real lifecycle measurements were inspected and reused, not rerun:

| Capability and original goal scope | Existing measured result | Evidence boundary |
| --- | --- | --- |
| G6/G7 source correction with stable identity/history | Collection `record.revise` apply/replay 0.61/0.29 s, about 39/38 MiB | [Exact source review](2026-09-09-native-corpus-descriptive-revision-review.md); not a full-corpus latency curve. |
| G4/G7 assessment withdrawal and dependent recovery | Layer withdrawal 1.13 s; renewed Layer/Unit 0.99/1.09 s, 42–43 MiB | [Real quality lifecycle](2026-09-09-real-native-quality-assessment-review.md); source-visible decisions and command time are distinct from model cost. |
| G3/G6/G7 current local access | Renewed project-note human/native-MCP cycle 5.21 s; old expired selections and an outside unit refused | The existing renewal receipt verifies that only mandate/times changed; no material, license or transport expansion. Timing is the positive read cycle, not the act of granting authority. |
| G7/G8 deletion and restoration in the derived publication | Withdrawal stage/commit 572/247 ms, 174 statements; restore 633/224 ms, 187 statements | Actual local Worker, human/native-MCP presence agreement and stale cursors; this removes serving records, not source history or source files. |
| G7/G8 full local reader rollback/recovery | Reverse 3.165 s / 39,714 statements; forward 0.763 s / 62,316 statements | Exact source record and reading-note hashes unchanged; retained local D1 evidence, not production activation. |

These close the named bounded lifecycle observations, not a blanket K3
acceptance. Read-growth tests separately protect 5,000-to-10,000 unrelated
rows/high degree; compressed mutation tests protect block locality and exact
mutation-budget refusal. Synthetic growth checks are not historical events
or measurements of source judgment. Remaining K3 review must reconcile these
costs and full-store publication measurements;
it must not demand destructive source deletion to prove derived deletion.

An additional permanent compressed-store invariant uses 511 and 1,022
synthetic documents and the same insert/update/delete sequence. Exact query
streams agree with the reference after each step. Doubling the unrelated tail
does not increase logical mutations (764/210/692) or affected posting blocks
(339/80/339); payload bytes remain bounded by affected blocks times the codec
block limit. The targeted test passed in 3.187 s. This is a bounded mechanism
test alongside the real full-store publications, not a wall-time complexity
proof or a historical source claim.

Evidence SHA-256: `prepared-catalog-mapping-current-r1.json`
`338ceaccbb9065ca21c7a148b926f07c40c372fc0b8454639863c3f28b80b332`;
`foundation-source-delta-handoff-r2.json`
`e47bfc8ea3a226d3f37ae67f74a7ac039c687c1d899d68444215fec79460fcb6`;
`delta-batch-real-worker-r1.json`
`0db3adc12328af385465940c486833b4b4b4148a3a86c6da7b5d452ee8742d66`;
`full-d1-recovery-r2.json`
`feb0d1125660c5f20f6690b12a9582d273abdfcd88ac5be8b33a2149e071dddc`.

The old catch-up capture receipt's aggregate pairing flag was too broad.
Only successor prepared/source pairing is mechanically verified; predecessor
roots, dependencies and publication token remain externally admitted.
`d1-catchup-pairing-qualification-r1.json` (SHA-256
`29210c82f510cb7c7b4051a3f8efa16993178962171376e07eac920b9c8aa554`)
narrows that historical claim without changing its bytes or reapplying SQL.
The corrected receipt code has separate flags and both delta/catch-up tests.

Manual boundary review: source traceability, stable identities, authored versus
derived separation and source/assessment/rights authority remain intact.
No new textual judgment, canon decision, private publication or abstraction
was introduced. Remaining K1 is exact source/software union and software landing;
K2 still needs its complete exact-text route and outstanding interaction seams.

## Historical carrier/episode and bounded title search · 2026-09-15 UTC

The current D1 scene now connects the already checked Penn source segments in
an actual human/agent interaction: open the modern book, inspect its 1920
Yale edition and DjVu File dossier, select the rediscovery-carrier Claim,
inspect that same Claim through WebMCP, expand its neighborhood, and select
the 1914 access episode. The title, uncertain/unreviewed statement, source
path, and Jastrow attribution survive these transitions. The modern book,
1920 edition, later digital carrier, physical tablet and episode remain
different objects. Shared book evidence does not establish a causal link;
prior access history remains unknown. The source-record/participant/context
checks from earlier bounded segments were reused, not rerun as one large
scenario. Nested context is still sometimes structured JSON, not accepted
complete human prose.

Two temporary 15-second requestAnimationFrame probes observed the visible
scene and a book-to-Claim selection: 1,193 / 1,380 callbacks, p95 intervals
16.8 / 16.7 ms, maxima 25.1 / 25.0 ms, zero intervals over 50 ms. Probes were
removed. These are bounded callback observations, not GPU-present latency,
all-scene smoothness or a timed claim about every subsequent route action.
The existing spatial composition was visually inspected without redesign.

The real query `An Old Babylonian Version` initially failed the unchanged
16-million-character verification gate: the rarest trigram selected 166
node candidates with 16,787,765 document characters. Indexed metadata-only
diagnosis showed that three rare trigrams select 39 candidates / 1,656,122
characters. The general Worker planner now intersects up to three postings
using covering-index membership seeks before text verification. Every used
posting closure is checked, and their summed closure count remains within
the existing 50,000-candidate budget. Exact full-text matching, ordering,
snapshot-bound cursors and the character cap remain unchanged; there is no
title-specific exception, new index, database rebuild or legacy fallback.

The same real request now returns HTTP 200 (272,640 bytes, 1,533.62 ms),
including the book. WebMCP search then human selection retains that exact
Work identity; a second cursor page has no repeated node/relation IDs from
the first page and does not clear the selected book. Unknown totals remain
unknown. The result closes this G6/G7/G9 search availability/continuation
seam and the bounded G5/G6/K2 carrier-to-episode human/agent join, not all K2
or global K3 performance acceptance.

Durable local evidence: `penn-carrier-episode-ui-seam-r1.json` SHA-256
`6e44928608ab5e183eb7571ea92cfb1dfb0c424f542aff83d2b3feb4ae8fe4e5`;
`penn-title-intersection-runtime-r1.json` SHA-256
`52cce1158f60784b2065fc1440db42a280ccafe7984619eb9ba35872bbc88ff8`.
Remaining completion work includes complete concept/exact-text and other
K2 interaction coverage, outstanding K3 mutation/growth dimensions, source
companion sealing and exact-head CI/merge. PR 229 is now merged as
`e7b09be2cbfd237b8d8f70c97c795ffefa2903a0`; later PRs remain separate.

## Published native navigation reader · 2026-09-15 UTC

Python now reads the selected SQLite publication's native source-navigation
product instead of silently consulting the legacy corpus index. Shared pure
query functions preserve the existing downward walk, bibliographic lineage,
tree ancestry, Links, scoped rights and fail-closed agent summary. Native
selection validates full JSON against indexed fields, including overflow
payload framing, under the published reader's row/byte/SQLite-work budgets
and post-transaction currentness check. No second database was created.

On the already-running local D1 publication, Python and Worker return equal
complete JSON packets for Penn collation environment descent (2 nodes / 1
edge) and the Jastrow/Clay book dossier. A first Python observation took
38.981 / 76.126 ms; the later paired check took 5.066 / 10.569 ms in Python
and 154.411 / 109.208 ms through Worker HTTP. These are retained-cache bounded
cases, not cold-OS measurements or corpus-wide endpoint parity. The selected
source/data revisions are the same D1 revisions recorded below.

Capability now working: the existing native source product can be consumed
by Python and Worker through the same source-navigation operations, including
new material absent from a stale static index. This closes that G6/G7/G8
consumer seam. It does not add rights or accept a historical inference.
Remaining limit: the separate prepared-normalization SQLite format currently
has no complete native navigation product (especially its header and rights).
That selection must report an unavailable product, not manufacture a dossier
from normalized attributes or silently read the old index. The running Python
prepared service has not been switched to D1 or restarted by this check.
Complete K2 routes, remaining K3 measurements and whole-goal acceptance remain
with the master.

Reproduction: durable local `check_published_native_navigation.py` accepts an
explicit database, expected source/data revisions, loopback peer and bounded
public case IDs. Receipt `published-native-navigation-comparison-r1.json`
SHA-256 `521e4b976ae36ee7fa8892365d9bcde225966613491b1ad30742d4239649e969`.
Existing source/query-store regression checks and all 20 published-reader
tests passed before additional malformed-native-product tests. No source
grant, corpus payload, publication epoch, or service state changed.

Delivery update: PR 228 passed all three required software/Worker/aggregate
checks and was squash-merged as `620e2a59daf27476f92382c5c885a9fb7cc30a17`.
PR 229's subsequent main synchronization has identical tree bytes to its
already-reviewed predecessor; its exact new-head CI is separate and pending.

## D1 catch-up and existing exact-source seam · 2026-09-15 UTC

The lagging local D1 reader now shares prepared source revision
`61e5059bcff97b455f04e4aab92d6edb6a5bc5b33a37e059ec3d80e6f2acba26`.
Explicit offline reconciliation scanned 210,759 admitted digest-manifest rows
in 13.252 s, retained 25 changed normalized rows, and emitted bounded forward
and reverse SQL. Atomic local application took 5.172 s / 574 statements;
all 25 changed native JSON bodies equal the prepared successor. Peak apply
memory was 409.8 MiB with no swap. No second prepared database or full graph
normalization was used. Ordinary per-edit capture still uses two held WAL
read transactions on the same prepared file; this offline scan is not its
latency budget or an automatic fallback.

Worker health confirms data revision
`027f508161807217223234ef62d54c0b63356e154634f4b797995e36292ecaf5` and
42,682 nodes / 62,710 relations. Its indexed discovery carries the shared
source revision. Python retains its own prepared-format data revision, not
the D1 revision; distinct representation identities are expected. Work
navigation/dossier and incoming Penn environment exploration agree on bounded
IDs and topology. Python exact owner-read verifies the environment and three
Claims. D1 explicitly reports that source-owner handle/read is unsupported;
it does not pretend that derived JSON is an owner-record response. A direct
environment descent returns 200 on D1 but 404 on Python: this separate legacy
navigation seam remains under access-owner diagnosis, not full endpoint parity.

Actual human D1 interaction opens the environment, its inverse relation and
the Claim. WebMCP then identifies that exact selected Claim with unreviewed
and derived-export posture. Publication year remains distinct from unknown
collation date. This surfaced an ordinary-inspector defect: the validated
readable-context companion was passed to the reader panel but omitted from
the card. The card now passes the same verified companion to both form and
record-context renderers. Real source labels such as identification status
and provisional identification appear without a UI-owned taxonomy. Unknown
context entries and nested source objects remain explicit; this is not a
claim of complete prose coverage or global interaction smoothness.

The exact JGB19 connection already existed and was not recreated. An existing
private Claim binds the same public commanding-affect Conception version and
digest to an English Expression, with an exact Occurrence and native TextUnit
in its five-record source closure. The currently renewed local selection
returns that source in the owner process; no private request crosses HTTP/MCP.
This checked seam took 1.686 s, with no extraction, source mutation, new grant
or semantic admission. The old creation configuration supplies addresses only,
not current write authority. This evidence does not establish an exact German
JGB21 unit or a public/private browser bridge.

Goal closure at this checkpoint: G7/G8 addressed local derived publication
and checked retention; G5/G6/G9 human/agent selected-identity and context
continuity; G3/G6 and part of K2 exact existing JGB19 source connection.
Remaining: direct Python environment descent, complete K2 routes, remaining
K3 mutation/growth measurements, UI acceptance, exact-head CI/merge and final
whole-goal review. Seven catch-up fixture tests, six existing prepared delta
tests and the same-file WAL test pass. Inspector/context regressions pass
52 existing and four new cases; typecheck passes. Existing replay/reverse
coverage was reused, not relabeled as a new real reverse application.

Durable local evidence (basename, SHA-256):

- `d1-catchup-capture-r2.json`: `b6daa4e972a6cd3d698c0436ec7a9ea7932a6f2e95634609671c2f37c41fad51`.
- `d1-catchup-apply-r1.json`: `5ec2c8bc2464eebfc83f1e62e8e5c2b3ef9a440cede58f1dd75af0f280b45c3d`.
- `d1-catchup-consumer-evidence-r1.json`: `92ca0bf6f6244b40380ebf9e942642ebfea694de1cbf1020b1f4fada16d2a349`.
- `d1-penn-browser-seam-r1.json`: `cf007ba1980f900b2f347f36f565a5130b9a95fca1eb5bdf4c13eec8fd779b57`.
- `jgb19-private-public-seam-r1.json`: `77680f8ce0d039d0a396657281c099a5417734ae09a5cd8413365f329581bdad`.

## Penn source-creation continuation · 2026-09-15 UTC

Committed creation evidence and a current write delegation now have distinct
checks in the initial metadata/identity-Claim publication routes. Exact receipt
bytes, original configuration, principal, authority, source path and pre-expiry
creation instant remain bound. Natural expiry does not invalidate an already
created package; new source commands still reject expired delegation. Changed
scope, tampered/future receipts and post-stage revocation refuse. This does not
renew grants or admit source content.

On the retained full prepared store, the existing Penn research-environment
package added 3 nodes/2 relations in 24.588 s and 48,441 SQL mutations. Its three
existing participant/environment Claims then added 5 nodes/15 relations in
61.158 s and 215,858 mutations, with 751.6 MiB peak memory and no swap. The first
Claim attempts did not commit: the task harness first obscured a budget refusal
with an invalid rollback method, then exposed the 100,000-mutation search-index
limit. Contract-correct rollback retained the predecessor; the successful run
used an explicit 500,000-mutation bound. No database copy, source command,
automatic admission or D1 write occurred. These are measured offline costs,
not accepted small-edit latency budgets or global K3 completion.

At source revision `61e5059bcff97b455f04e4aab92d6edb6a5bc5b33a37e059ec3d80e6f2acba26`,
native MCP reads nine exact related records and traverses from environment back
to its Claim. HTTP and the production web source client return identical Claim
and environment records. Actual browser selection -> Sources -> original record
preserves identity, uncertainty, unreviewed status, single-source limits and the
distinction between publication year and unknown collation date. The scene also
retains the reception process, environment and evidence neighbors. Existing
built assets were reused; this is not a global smoothness measurement.

This closes the checked G3/G5/G6/G9 historical-environment consumer seam, not all
K2. Local D1 catch-up, other K3 mutation classes, final CI/merge and whole-goal
acceptance remain separate. Checklist: yes for source traceability, identity,
qualified historical context, language authority, uncertainty and authored versus
derived boundaries; not applicable for canon, consent, calibration, counterpart
or new assessment admission. Completion ownership remains with the master.

Durable local evidence (basename, SHA-256):

- `penn-metadata-publication-r1.json`: `a95822554dbabfbc1e49e62ec79c020e824470b69c893d82cf4821c3874d0b6a`.
- `penn-claims-publication-r3.json`: `b1e98de8a088161172b6cf478ce00dac59084ca41fdc0e009fd3387bfa61bc24`.
- `penn-context-agent-r1.json`: `a0b44461f26386335768f8a82bcecce5293070d6211f45421c19bfa0c49ef63e`.
- `penn-context-paired-r1.json`: `310df0066ee61ffe9f43365c9b4016f16970b6bd34acf94642cc1cd935f5fff4`.
- `penn-context-browser-r1.json`: `44148850218b6c8e867709e6679ab382fa25eb14fe20519153a9ecfb9d95b8da`.

Reviewed worktree: `codex/tos-foundation-final-20260914`, with first parent
`7f59dc9147690f767e332a41dcd1503c996bbed0` and incoming parent
`36de25a5018aa277f64cec547e6dd9941358e697`. The observations below were made
before the merge commit, against its edited software. Session:
`01a06cc7-0452-77f2-b89a-fb77fb86c3bf`. Exact commit checkpoint review follows
separately; neither parent alone contains the tested integration.

## Working capabilities and their limits

| Capability actually checked | Goal coverage | Remaining boundary |
| --- | --- | --- |
| On the real canonical departure event and Zarathustra support record, agent indexed search → human result selection → agent inspection preserves the selected identity and readable inspector. The same bounded dataset also passes HTTP/native MCP inspection, compact lens and continuation, with request-time full graph construction forbidden. | G5/G6/G7/G9: shared human/agent reading, stable identity, bounded query delivery. | Two real records and a real JGB date Claim are integration seams, not the complete two mandatory research routes or full-corpus UI acceptance. |
| The compiled query store directly serves indexed search with the same four-rank ordering and native records as the legacy route. Node and relation pages reach EOF without duplicates; malformed, expired, changed-query/filter/snapshot cursors fail closed. Scan-only stores and insufficient candidate/verification budgets do not silently rebuild an index or graph. | G6/G7: explicit query compilation, bounded work, snapshot-bound continuation. | The isolated installable candidate and CI remain separate checks. |
| Oversized native D1 rows retain complete source JSON and searchable content in versioned metadata chunks; digest verification and the ordinary read budget still apply. Full/delta/replay equivalence and missing/corrupt/over-budget refusal are checked. A failed large read does not block another node. | G7/G8: no silent field loss, atomic derived publication, bounded reads. | Producer retention up to 8 MiB does not grant delivery above the 1 MiB native read budget. The corrected complete D1 candidate has not yet been reimported and health-verified. |
| Optional knowledge-search pagination no longer prevents browser initialization when its control is absent. All four existing WebMCP browser scenarios pass after the fix. | G6/G9: selection, source-gap and agent browser operations remain connected. | This is not a redesign or a new global smoothness claim. |

The real-record browser run reported no page errors and zero full-graph calls.
It does not read private payloads or perform semantic admission. Public metadata
test snapshots retain their source paths and digests; they do not become new
canonical records or rights grants.

## Reproducible evidence

Durable task artifacts are identified by basename and SHA-256 below. They are
local observations, not publicly activated runtime receipts.

| Artifact | SHA-256 |
| --- | --- |
| `final-real-browser-cycle-r2.json` | `98637d648767135025660a488c8a5af459c117d738351de7006f6480ebc5d70a` |
| `indexed-query-store-focused-r1.log` | `03ec00144a15294c998a749f79fe2c978791b5f618cb6530c0db4363b4121b7d` |
| `final-native-overflow-seams-r2.log` | `699e7a749a2f111040b3d82d5a47136c60faf89790fb58f8122e93be8412e8f3` |
| `final-overflow-delta-r1.log` | `e9332dec30b0592eaa280d219e5480cd533c1af5c84c191b4c8f3d09f9b8a8ee` |

Durable regressions live in `access/tests/test_query_store.py`,
`test_published_read_model.py`, `test_incremental_runtime.py` and Worker
`test/knowledge.test.ts`. The browser scenarios are in `access/e2e/test_webmcp.py`.

## Source-first review and next owner

Source traceability, native-field retention, identifier stability, language
forms, uncertainty and authored/derived separation remain explicit: yes.
The changed readers do not accept Claims, manufacture human signatures, grant
private access, merge identities or publish canon: yes. No new interpretation
or assessment competence is claimed; those checklist items are not applicable
to this software checkpoint. Access remains read-only.

The master retains completion ownership. Remaining K1–K4 acceptance must use
their exact source/corpus and consumer evidence, not the number of green tests.
In particular, exact-text permission, complete corrected D1 health, the
installable software candidate, required CI and merge are not closed here.
The pre-existing complete D1 candidate with a header-count defect is retained
as diagnostic evidence, not presented as healthy. Its replacement is a
separate exact-target storage action, not implied cleanup permission.

## Subsequent full-corpus consumer checkpoint

The retained source at `a3144d4bea084d38c8fd9b822b9b30072188ebbc`
has source revision
`46f6f4b830ab3d5b419462907f94044b53910cdee916924cf0de7265829f8fb4`.
Its prepared snapshot was reused without re-normalizing the corpus. Python
and Worker run the integrated software from `79cf1f15d2ad1847ef5bdf65ac6413a58b9dfc15`;
the browser additionally includes the capability-selection change reviewed
in this continuation. The exact successor commit is bound by checkpoint review.

| Capability actually checked | Goal coverage | Remaining boundary |
| --- | --- | --- |
| The full prepared backend advertises compressed search, not indexed search. The page now selects an advertised engine, retains its native schema and opaque cursor, and rejects unavailable explicit modes without a legacy retry. On real freedom and letter 705 records, agent search → human selection → agent reinspection succeeds with no page errors. | G6/G9, K2: one shared searchable corpus and selected identity across human/agent adapters. | This is not the complete concept/text or historical transmission route, nor a global smoothness measurement. |
| A persistent browser regression verifies compressed continuation, no duplicate page identities, and selection preservation after an unavailable-engine refusal. All five browser scenarios and 549 web unit tests pass; typecheck and production build pass. | G6/G7/G9: engine-aware continuation and no hidden request-time graph fallback. | These checks do not grant content access or accept historical meaning. |
| The corrected full D1 import completes with 1,008,590 executed SQL statements. Worker health is HTTP 200, `ok: true`, with data revision `3ee4420d1a5120be42addca42e207042b26be420288f1a1e21a9d152ce5c0853`. Catalog, canonical departure, Penn reception Claim and freedom Concept have equal native JSON values to the Python backend at the same source revision. | G7/G8, K3: healthy full-corpus D1 delivery and checked native field retention. | Local processes only; not production deployment or exhaustive endpoint parity. The catalog and three inspections took 1,631/206/206/192 ms on Worker and 153/10/17/25 ms on Python with retained filesystem cache, not cold OS. |

The import took about 22 minutes, with 7.6 GiB reported peak memory and
11.1 MiB swap. Its cost is a full data-release/bootstrap cost, not a small-edit
budget. No second prepared corpus or database copy was created for these
consumer checks. The obsolete SQL generation may be removed only through
the separately authorized exact-target storage route; the current SQL,
database, prepared snapshot and durable receipts remain distinct.

| Artifact | SHA-256 |
| --- | --- |
| `final-live-browser-r2-resource.json` | `18484dd4b3b33b07238096b0d3013f3e2fbf894159c25d24db3474f5827393b0` |
| `final-browser-regression-resource.json` | `147eda4750ed6c69d332ac0f10f6b812d353e695c1bea5f7a500129d0c745a79` |
| `search-mode-web-final-r2-resource.json` | `e6f9fb56af795e745ca9f0854eb30bba28e3b8b33dee38f3e3b608cede4c8183` |
| `final-d1-native-parity-r2.json` | `2a1ffd84877e80827d98c409873d9518a743c0b4be843e6af363b462de428145` |

Source/derived separation, exact identity, current access authority and
read-only delivery remain preserved. No source interpretation, rights grant,
canon decision or human signature is created by this change. Remaining
completion ownership stays with the master: complete K2 routes, outstanding
K3 measurements, exact updated software package, CI and merge are not inferred
from this checkpoint.

The same checkpoint subsequently exposed a separate legacy-offset search in
the default observatory shell. It now shares the advertised engine selector,
keeps native cursors and unknown counts, and offers the knowledge-search tool
without changing scene rendering. A real full-corpus browser search for
`freedom` returned both source carriers and philosophical relations; human
selection and agent inspection retained the exact freedom ID. A persistent
prepared-browser test verifies human and agent search, cursor continuation,
and previous-page return. The other five browser cases remain passed; the new
case passed after correcting its asynchronous test wait. All 552 web tests,
typecheck and production build passed. Source-panel exact-record consumption
and the full historical/text routes are still separate from these search checks.

`observatory-search-build-r1-resource.json` has SHA-256
`dc166079ba2970f8024e2dc7a0cbe58c49f1f2bb1f640b003fdda3202bafcc97`;
`observatory-search-browser-r2-resource.json` has SHA-256
`6bcc2fd8c3dd42ee25584653a10a7b9a023e4cda68a9347dcfe6d314d89cf0e6`.

Worker CI's missing identity-proposal fixture is repaired by selecting the
three actual mechanics test modules and replacing a root-test dependency with
the existing shared frozen source-assembly fixture. No authored corpus is
added to software CI. The affected Python module passes 16 tests / 112
subtests; the exact Worker case passes. The required aggregate is unchanged:
it still requires every software job to succeed, and the full CI run remains
the separate landing check.

## Default-shell exact source return

The default observatory Sources panel now consumes the existing exact source
reader for arbitrary selected nodes and relations. It takes the target from
version-bound backend inspection; it neither derives paths from IDs nor needs
a record-specific screen. Metadata, native text and bibliographic navigation
remain distinct. The selected owner's capabilities advertise public/local
text choices; each requested representation still validates its own binding
and rights. A refused text request preserves the available metadata record.
Local returns keep complete selected notices next to their unchanged spans.
No permission is created or extended by this UI change.

On the retained full corpus, the real project-authored Occurrence
`tos.occurrence.sid-0405e33f53d54b8f9c0d5325dd9858d6` opened its exact public
record and original English notes. The public-text request correctly returned
`access-restricted/native-unit-public-rights-not-satisfied`: its separate output
rights are conditional, and this reader has no current local selection.
The metadata remained available, no local-reading button was advertised,
and native WebMCP reinspection retained the same occurrence and source path.
Browser error collection was empty. This note is not a Nietzsche witness;
these observations close the tested G6/G9 exact-record seam, not positive
native-text delivery or the complete K2 historical/concept routes.

The live observation is `observatory-source-return-live-r2.json`; it binds
source revision `46f6f4b830ab3d5b419462907f94044b53910cdee916924cf0de7265829f8fb4`,
the dirty successor of software `18cda4ccdbacc9d68b2b74405a66f8925ee0e129`,
and exact source/browser byte hashes. Its browser asset has SHA-256
`5571e5ed551dd40534a96bebe03436f6e07b5640b48eab6969d966ee93e0cb97`.
Typecheck, 17 targeted exact-reader/localization tests and production build
passed; `observatory-source-return-build-r2-resource.json` has SHA-256
`f8ca66ad11a5f0e8fa8b4380c45cf218404da8085217a63df0987a77c5425dd4`.
Earlier adequate full-corpus and search checks were not repeated.

The `18cda4ccdb` Worker CI job passed. Its software job stopped before browser
execution because the independent page-command expectation lacked the newly
registered `tos.page.knowledge-search`. That exact expectation is updated;
strict equality and required job aggregation remain intact. Local standalone
software validation passed. This repairs contract-test drift, not a relaxation
of the command surface. The successor still needs its exact package and CI.

Review: source return, version/identity retention, source-language authority,
read-only ownership and access boundaries are preserved. Historical meaning,
canon, assessment and translation admission are unchanged and not inferred
from these tests. There is no scene/camera/gesture redesign or new corpus
normalization. Outstanding K2 source completeness, K3 measurements and the
authorized landing remain with the master; the goal stays active.

The same consumer review found legacy indexed-only limits in the advertised
WebMCP search input: a three-character minimum and an 8 KiB cursor ceiling.
The adapter now admits nonempty queries and up to the compressed reader's
64 KiB opaque cursor; the chosen engine still validates the actual query.
The existing round-trip test now checks a one-character query and a complete
64 KiB cursor against both the published tool schema and the returned token.
All 17 WebMCP tests, typecheck and the updated production build passed;
`observatory-source-return-build-r4-resource.json` has SHA-256
`0d9a5832b21e862044b0961ec598e55c40becf29146809af3c5e4ee6b38a77d0`.
This later build changes WebMCP declarations, not the source-panel logic
observed above. The full D1 corpus was not rebuilt for either adapter change.

On the updated real browser, native `tos.page.knowledge-search` with query
`道`, compressed mode and limit 2 returned `dao 道` and
`Daoxue 道學 / Learning of the Way`, plus two relation records, in the shared
human search panel. Matching totals remained unknown and continuation remained
available. Candidate posture was not promoted. The selected occurrence stayed
unchanged. `observatory-short-search-live-r1.json` records this G5/G6/G9 check
and the exact final browser asset digest; it is not semantic acceptance of
these corpus candidates. New source-panel actions also reuse the existing
button container styling after a visual review, with no layout redesign.

## Retained full-corpus query cost

Five independent fresh Python processes each constructed one reader and ran
the same operation twice on that same core. The exact normalized freedom ID
was used for focus, exploration and inspection; inspection returned one match.
No source or database copy, normalization, cache dropping, service restart or
fallback was used. The selected source/data revisions remain the retained
`46f6f4b8…` / `18f51b1d…` publication, not a new data release.

| Operation | First / repeated query, ms | Returned bytes |
| --- | --- | --- |
| Catalog | 102.039 / 86.494 | 3,407,484 |
| Compressed search, `freedom`, limit 4 | 29.875 / 11.414 | 129,069 |
| Focus, depth 1, 16/32 node/relation limits | 55.232 / 28.263 | 586,381 |
| Exploration, depth 1, page 8/16 | 44.229 / 23.389 | 141,301 |
| Exact inspection, relation limit 16 | 5.585 / 6.015 | 177,462 |

These are core-call timings, not HTTP transfer, serialization, browser paint
or cold-OS latency. Fresh-process setup took 314–459 ms separately; the launch
reported 100.3 MiB peak memory and no swap. The receipt records limits from
the actual reader/search/lens/exploration instances rather than a copied list.
Owner APIs enforce those bounds; this measurement is not an independent
proof of every limit. Search used 66 rows, 243,777 read bytes and 13,500 SQLite
VM steps in the retained fresh-process observation. The 3.4 MB catalog remains
an initial vocabulary carrier, not the size of each graph-scene change.

`k3-retained-prepared-warm-benchmark-r2.json` has SHA-256
`7e897be37d41d1dbde3fb71234bf11d19a206501fcb9fc904cd04b2d4c10ad75`.
Its original r1 receipt remains separate: that run repeated fresh processes,
not warm calls, and used an entity alias returning two distinct carriers.
This closes the named current-publication query-cost measurement portion of
G7/K3. It does not close remaining mutation, growth-cost, access-transition or
full K3 acceptance requirements, nor replace existing recovery evidence.

The persistent Sources browser regression also passes on the final built
shell: native page selection, backend exact target, human Sources action,
record read, and expansion of exact record/provenance details. It reuses one
existing frozen Work record with a test-local metadata owner, not production
corpus or native text. The successful run took 5.01 seconds with 679.4 MiB peak
memory and no swap. Its log SHA-256 is
`33729b4e995b961003215f4ddece859ae1a0466b50f0a2df4e2fc46e18e2cda9`;
`sources-panel-e2e-r6-resource.json` has SHA-256
`ea8ce7745918e6142d6b50b37c465b211419774a5efdbcb6a423bde0074d8519`.
Earlier attempts exposed fixture/navigation selector errors, not source
permission failures; their failed receipts are retained rather than counted
as passes. Existing source-reader unit tests separately protect stale,
corrupt, unsupported, conditional and bounded-text cases.

## Native module loading parity

CI run `34877144680` exposed a real packaging-independent seam: the
observatory imported the shared search selector without its `.ts` extension.
Vite resolved it, but the direct Node contract reader could not load the
module. The source import now names the existing module exactly; no loader
fallback, test skip or duplicate selector was added. The already existing
node/relation origin parity test passes, as do TypeScript checking and the
browser build (`source-import-parity-r1-resource.json`, 6.530 seconds,
658.8 MiB peak, zero swap). Browser asset hashes are unchanged. This closes
the observed direct-runtime import defect, not the still-pending new CI run
or the broader Foundation acceptance.

## Native D1 navigation and advertised search · 2026-09-15

PR #226 landed as `7f610a78d3743258cbae7df9e020f4888078479a` after
successful required CI run `34926570346`. This is software landing, not a
production deployment or a publication of newer authored corpus records.

The retained full D1 now includes its native navigation product. An addressed
bootstrap installed 27,112 nodes, 39,765 edges and 127 rights records, preserving
the normalized corpus and source revision `46f6f4b8…`. Data revision changed
from `3ee4420d…` to `73b8ba6b23c56983d97a5487a105f0637e55bf5793754ef5cb936027007a0d23`.
The official atomic local SQL importer executed 1,244 statements in 4.056 s;
the resource launch reported 627.1 MiB peak memory and zero swap. Read-only
full product readback checked every native row against the exact retained
navigation and rights inputs in 8.838 s, with 60.5 MiB peak memory and zero
swap. No database copy or corpus normalization was performed. Exact reverse
SQL is retained; it was not applied to the real full D1. Small-fixture reverse
tests are separate evidence, not proof of a real full-database rollback.

| Capability actually checked | Goal coverage | Remaining boundary |
| --- | --- | --- |
| Native D1 navigation/rights product matches its complete retained inputs and serves the real JGB Work dossier in the default Sources panel. | G7/G8, K3: addressed derived publication and source-preserving readback. | Retained inputs are not current-main corpus publication; bootstrap cost is not a small-edit growth budget. |
| Worker search capability discovery now validates publication and schema without reading corpus rows. Human Jenseits search and native WebMCP search use indexed mode, then human Work selection and fresh agent inspection retain the same ID and source path. The scene loads 40 nodes / 39 relations. | G6/G9, K1: shared human/agent discovery, selection and source-dossier access. | Worker exact source handles/read are still absent: opening the original record fails while the dossier remains available. Exact text and the complete K2 routes remain open. |
| Python and Worker health responses retain aggregate node/relation counts and the five validated display coverage counters, excluding diagnostic/catalog-sized details. Incomplete coverage still fails readiness. | G7: bounded readiness delivery without changing source diagnostics. | Health does not prove source semantics, permissions, corpus currentness or every query operation. |

The live browser observation uses the dirty runtime-capabilities successor of
the merge above; it records exact hashes of changed runtime files. Its receipt
is `d1-browser-search-dossier-r3.json`, SHA-256
`d8b9ec54ab7fea3103c451a437a24e18b6bf00c3290a2618ea5169be94114be3`.
Native publication receipts are `native-bootstrap-apply-r2-resource.json`
(`ba7313576a3718cf1b7cb397dbaeb52d1b111a6d4c7539c4fa94bab5c4632127`)
and `native-bootstrap-readback-r2-resource.json`
(`0efc2f68b0b4c169d13a05fc69ad758c88abf08272811bf74b7e5ef561ef87d4`).
All are durable local task artifacts, not public runtime receipts.

The existing indexed HTTP fixture now checks capability discovery before
publication, after publication, and after removal of its fixture-only gram
statistics table: 503, 200, 503. Its real indexed/cursor cycle remains intact.
That test, the Worker health HTTP test, TypeScript checks and the three focused
Python health cases pass. Previously sufficient corpus/search checks were not
rerun. Exact successor CI and software artifact remain the next landing gate.

Review: source traces, native values, identity, uncertainty and authored/derived
separation remain preserved; yes. These adapters neither grant rights nor
accept Claims, signatures, interpretations or canon. No source assessment or
translation admission is made; not applicable. The master retains open exact
source delivery, K2/K3 completion and final Foundation acceptance. Scratch G16
remains open; no destructive cleanup was performed.

## Search eligibility and explicit source-owner boundary · 2026-09-15

The successor review identified an indexed-search boundary: its trigram engine
requires three normalized Unicode code points. Both Python and D1 now advertise
that minimum, and the shared browser selector applies the same native lower/strip
rules before submitting a search. Automatic selection may choose an available
compressed engine for a shorter query; an explicitly requested indexed engine
is refused, never silently substituted. Three focused web files pass 52 tests,
including no-search-request and Unicode boundary cases; web type checking and
the focused Worker indexed-search and health tests also pass.

The prior browser failure above is now classified more precisely. The source
read contract assigns exact owner reads to the explicitly selected Python
adapter, not D1. D1 serves derived navigation and now advertises source-reader
unavailability. The Sources client checks that capability before asking for a
handle. On the retained JGB Work, the real human action now returns
`unsupported / source-owner-reader-not-configured` while preserving the
selection and dossier. Native WebMCP's short `道` request is refused by engine
eligibility rather than sent into an incompatible backend. These observations
close the G6/G9 and K1 discovery/selection boundary defects, not exact source
delivery through D1 or completion of K2.

The operator renewed only the two existing local-reading scopes. New selections
preserve every material and transport restriction; old expired selections are
still rejected. Current software reads the exact private English JGB19 unit
in its owner process (5,186 UTF-8 bytes, complete notices), and the exact
project-authored note unit (6 bytes, two notices). A different unit is refused;
source bytes are unchanged. JGB19 remains unavailable through HTTP/MCP and
does not supply the missing German source anchors. Receipts are
`jgb19-private-return-r3.json` (SHA-256
`8ab60fa952d9d575f2e395b43a3da1b62dee101d882b962ca6376171f137d811`)
and `renewed-local-reading-r1.json` (SHA-256
`f359fc1eff5aedfef101285591656bee5998f6e6bd1d5a24df11939ffcb35bb7`).
This verifies the renewed local access portion of G3/G6, not public rights or
full source-reading acceptance.

The retained Python catalog still pins an older execution profile. A reviewed
header-only candidate replaces only the `projection_diff.py` binding: complete
AST reconstruction verifies the unchanged legacy program after removal of the
new snapshot-only entry points. Its source collections, publication token and
all other profiles are unchanged; no parts or database copies were written.
Current source-owner code can read the note through that candidate. The
prepared database and running consumer have not yet adopted it; that paired
transition remains the next integration step. Review and staging evidence are
`catalog-reader-profile-review-r1.json` and
`catalog-reader-profile-staging-r1.json`, not a general compatibility grant.

Review: source traces, language authority, rights, explicit unavailable states,
and authored/derived separation are preserved. No source or canon admission is
made. K2, the remaining K3 publication/growth checks, and final acceptance remain
with the master. G16 stays open. The storage owner separately removed the
already imported SQL export; source inputs, prepared snapshot, live D1 and
addressed forward/reverse SQL remain. Recovery of that disposable full export
is regeneration, not undelete; byte-identical regeneration was not tested.

### Paired local source return

The reviewed catalog transition subsequently committed on the retained prepared
database: 224 SQL changes, 3.427 seconds, 177.3 MiB peak memory and zero swap.
No normalized row bodies, normalization binding, context membership or source
bytes changed. Source revision is `d3ce2a61cb1f6b43a458cce84d88a90d10c84251d488d93f024339314d028b01`;
prepared data revision is `cb01ff7611388ea1b7b2e544d75685c904621c66154fab867df10c46d0f66129`,
epoch 2. The old selection is rejected after commit. The first attempted
transaction rolled back on normalization-profile drift; the successful attempt
explicitly selected the retained publisher owning that unchanged normalization
profile. Current source-owner reader code remains separately selected.
`catalog-reader-profile-publication-r2.json` has SHA-256
`4001ee23979733d90ddf9f679ac3f6af06ec87371cabfa9c95a4b954e2917ee1`.

On this real full prepared corpus, native MCP and the current production browser
client over HTTP return identical exact record, handle and local native unit for
the authored note (5.210 seconds for the combined route). The actual browser
then follows selection → Sources → original record → local text and displays
`corpus`, both complete notices and the prohibition on external publication.
The browser used existing built assets; the current source client was checked
separately. This closes the positive local source-return seam of G6/G7/K2, not
the remaining historical routes or a new smoothness benchmark. D1 remains
independently selected at its earlier revision; no cross-backend switch or
currentness claim is made.

Receipts: `renewed-note-human-mcp-cycle-r1.json`, SHA-256
`1f24be34ab7761f354ba4a6905d710fcfe2b4983fbf65f34f8c5518d31c549c9`;
`renewed-note-browser-observation-r1.json`, SHA-256
`ff31a6e3466cd8d109f373da4793acfc0b686325ab21cf5efb62f761340b2ed7`.

Successor review also caught two delivery details: capability preflight now
honors a client response budget below 64 KiB, and the packaged operation map
advertises the implemented D1 indexed-search discovery route. The smaller-budget
test covers both representation discovery and a complete exact record read.
These changes do not widen a caller's resource limits or add source-reading
authority to D1. Required CI `34930035706` passed at `0e7c0ce371`; the two later
review fixes require their own successor CI before landing.
