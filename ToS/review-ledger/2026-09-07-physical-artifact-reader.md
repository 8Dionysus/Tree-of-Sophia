# Physical artifact metadata reader — 2026-09-07

Scope: native physical-source metadata in the existing catalog, claim graph,
corpus-index source-navigation and shared access reader. No artifact source,
rights record, payload, semantic claim or review decision is rewritten.

## Owner changes and boundaries

- `CORPUS_FOUNDATION.md` and semantic interchange define the adapter;
  entity registry version 6 maps both carriers to `tos.entity.artifact`.
  Relation registry version 6 includes Artifact only in the existing planting
  relation range already authorized by `philosophy-source-planting.schema.json`.
  A wrong Place endpoint remains a failing control.
- Catalog and graph schemas allow this explicit optional family. The native
  artifact v1/v2 schemas stay unchanged. Corpus identity assessment is absent
  (`null`) for this family only; legacy metadata review remains a separate
  source field, not automatic identity verification.
- Catalog/graph/index builders share exact field copies. The first custody
  inventory number is a source-attributed navigation label, the whole
  `path_identity.note` is the identity-boundary description, and pointers
  disclose their origin. No language is inferred from the inscription, ID or
  UI. Complete assessed human forms remain a gap.
- Both source-return readers retain every native source field and stable ID.
  Native rights and authority flags stay false where the source says false.
  No new custody, location, dating, carrier/text, same-as or influence edge is
  inferred from these fields. Existing planting edges remain navigation.
- Source-navigation formerly exposed some artifacts as ID-only placeholders.
  Its catalog-driven replacement retains the same node ID and authored edges.
The two carriers have one persistent entity ID; their existing `projects`
  relation is representation, not a newly accepted historical equivalence.

Review checklist: source return, source/derived separation, identity,
layer distinctions, explicit uncertainty and owner boundaries — yes.
Canon changes, counterpart, calibration and lived-witness transitions — not
applicable. This review does not assess historical artifact descriptions,
reopen media rights, admit text, or attest upstream research/model execution.

## Verification so far

The first test failed with a missing artifact catalog family. The two-carrier
test then failed on an unmapped navigation type, exposing an actual consumer
gap. Both are now specified through the real builders and shared access code.

Focused controls pass for v1/v2 native IDs/full fields, null identity assessment
without relaxing Corpus entries, refusal of private metadata, false authority,
unknown schema, wrong identity kind, altered catalog fields, escaped source
path, duplicates, symlinks and the 1 MiB metadata bound. The existing complete
claim-graph suite passed 51 tests before the final additional schema control;
the final focused artifact set passes 3 tests. Source catalog source-return and
native artifact-layer checks each pass; topology checks pass 26 tests.

The live catalog reads all 18 existing physical records (8 v1 and 10 v2),
validates every source against its declared schema, and validates all generated
catalog entries and the manifest. Catalog and claim-graph rebuild/currentness
checks pass. Full final consumer verification is recorded below when complete.

## Actual integration findings and bounded measurement

A live reader first exposed two carriers for some artifact IDs: existing
planting placeholders plus the new complete record. The common corpus reader
now replaces the placeholder in place. Real planting records then exposed the
missing explicit Artifact range in the relation registry; this is corrected
against the existing planting schema, with a wrong-Place endpoint negative.
Refreshing the corpus also exposed a previously unmapped historical navigation
carrier that omitted adjacent forms. Its exact historical schema/digest/public
visibility checks, type mapping and shared form materializer now preserve the
prior source-bound Russian form through default focus. A test first failed on
the unmapped historical type and now compares the entire delivered form packet
across both carriers.

One premature graph-test run overlapped the rebuild and failed on exact
projection currentness (17 errors and 2 failures). The first corpus rebuild
also preceded the graph it indexes and was correctly rejected as stale.
After graph → corpus-index ordering and terminal builders, all 52 graph tests
and 9 corpus-index tests passed at that checkpoint. Final follow-up checks
below cover the additional planting and historical dual-carrier controls.

Independent comparison to the pre-slice commit found all 697 old graph nodes,
1327 edges and 196 claim traces unchanged as JSON values; exactly 18 physical
identity nodes were added. All 18 native artifact files were independently
byte-compared with Git and are unchanged. The generated graph contains 715
nodes, including 190 identity nodes; these counts do not constitute admission.

A fresh local `ToSAccessCore` process inspected all 18 physical subjects and
verified both carriers against every native source field, inventory label,
complete identity note, type and review posture. Default focus retained its
authored planting and representation routes (3 nodes, 2 relations for the
selected coffin record). No inspected artifact acquired a ready assessed
human form or normalized historical date from its metadata.

Observed pre-closeout source revision:
`e1fe9907fab25c6537bb0a8496cb345a7081e9443cdd01088708c860e2905851`.
One CPython process, local source checkout, no concurrent checks from this
task: cold first inspection **17.837208 s**, remaining 17 warm inspections
**0.001570 s total**, default focus **0.205807 s**; user CPU **17.398664 s**,
system CPU **0.621558 s**, maximum RSS **1,200,984 KiB**. This is neither a
latency distribution nor a scaling/deployed-runtime benchmark. Cold startup
and whole-corpus rebuild cost remain substantial open requirements.

## Final focused checks

- `python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py`:
  **52 passed**, 48.946 s, including real planting endpoints and complete
  historical form parity through the default navigation focus.
- `python -m unittest discover -s tests -p test_tos_corpus_index.py`:
  **9 passed**, 40.422 s.
- `python -m unittest discover -s access/tests -p test_knowledge_contract.py`:
  **51 passed**, 22.879 s.
- `python -m unittest discover -s mechanics/growth-cycle/tests`:
  **101 passed**, 17.442 s.
- Source catalog exact-source-return and native artifact layer controls pass;
  topology **26 passed**, source-home and `git diff --check` pass.
- Catalog, graph and corpus-index currentness pass through the actual builders
  and validators. The required artifact-bundle wrapper reports ToS/AbyssOS
  admission **paused**; its successful wrapper exit is not artifact admission.

The final fresh Core functional check, at pre-closeout revision
`27894d2275f9679baea06dda0014efb9a83ddfd077416e3c6da46892dcfb644e`,
again preserved all 18 artifacts in both carriers. It also returned the exact
Jenseits historical record version 2 and identical ready Russian name/hover
packets in both carriers and default focus, all with `admission=null`. This
functional check ran alongside unit verification and is not a new latency
measurement. Adding this review evidence changes the subsequent whole-corpus
snapshot revision, not the inspected source subjects or forms.

## Recovery and remaining work

The native records were not migrated in place: rollback means regenerating a
chosen compatible derived reader, never deleting artifact sources or newer
research. New types/schemas are explicit; unknown versions fail rather than
being normalized into an invented known shape. The full source-foundation
route has an earlier private-payload blocker and was not rerun or bypassed in
this slice. CI, merge, deployment, D1, UI interaction and semantic acceptance
are not established here. Document/letter identity, carrier relationships,
artifact growth and assessed multilingual forms remain owner work, not a
completed subject profile.
