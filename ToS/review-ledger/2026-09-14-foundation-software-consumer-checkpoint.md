# Foundation software consumer checkpoint · 2026-09-14

This is a bounded integration review, not Foundation v1 acceptance, corpus
admission, CI, merge, or deployment evidence.

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
