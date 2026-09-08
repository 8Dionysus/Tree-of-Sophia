# Biography and historical context: implementation review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, against the change
following `b36f889fc47f5f8e5ae5fecaf132512b4860eb46`.
This is a bounded implementation/source-boundary review, not calibrated
historical assessment, an admission decision, a completed biography lens,
UI acceptance, release, or Foundation v1 completion.

## Changed owner surfaces

The semantic interchange registries, version 15, declare six substantive
historical profiles, 19 account properties, eight scoped relationships and
declared reader routes for the existing participant/place predicates.
`historical-context-record.schema.json` and
`historical-context-claim.schema.json` compose the shared source contracts.
No new per-kind Python branch or parallel type registry was needed.

The shared historical date grammar now accepts ordinary ToS identity syntax
for a relative anchor; the consuming reader must resolve HistoricalSituation
ancestry. The legacy carrier independently retains its old three-kind
restriction. The previous exact schema is retained as
`ToS/contracts/history/c64323ceb75498e98b58b661b602cbebad4a4e61b72ed8f5f9973cad75d13236.json`;
its byte hash was verified. Earlier provenance and source packages were not
restamped, rewritten or silently migrated.

Six real subjects were created through independently scoped ordinary owner
commands: a teaching phase, a discharge-request episode, a life circumstance,
an appointment environment, a local chair-turnover periodization, and Basel
as Place. The five historical records live under
`ToS/source-witnesses/history/basel-research/`; the Place is under
`ToS/source-witnesses/places/basel/`. Fifteen Claims live in
`ToS/source-witnesses/relations/basel-biography-research/`, including four
temporal values. Adjacent creation requests, receipts, serialization provenance,
environment captures and human-form sets retain the exact published inputs.
Immediate exact retries returned identical receipts. All identities remain
provisional, Claims unreviewed and forms without admission.

The [source reading](2026-09-07-basel-biography-source-reading.md) names the
accounts, read sections and limitations. It is immutable evidence input,
separate from this implementation review. The contradictory citizenship
reports were not resolved through names, place or institutional association.
The petition's date is not its delivery or approval date. No bodily account
was promoted to diagnosis or explanation of philosophical content.

## Review of the boundaries

Yes: source return, authored/derived separation, distinct identity/record/
Claim/value/form layers, language-bound forms, preserved uncertainty,
historical scope, no implicit admission, and unchanged external owner/UI
authority. Dates remain separate values. A historical period is an explicit
research periodization with a substantive basis, not merely an interval.
A generation is not automatically an Organization or a temporal anchor.
Environment domains are content properties, not combinatorial subclasses.
Context association does not imply influence, chronological containment,
continuous participation or causality. Missing domains are not negative claims.

Not applicable: canon or tiny-entry change, lived-witness intake, calibration,
counterpart mapping, operational AoA runtime, and release/publication mutation.
No new ADR is needed for reuse of the existing declared-profile seam; the
historical distinctions and anchor restriction are explained in the current
registry owner README. This review does not classify the new research as
accepted knowledge.

## Validation observed

- `python -m unittest tests.test_source_witness_bibliographic_graph`:
  70 tests passed, 209.044 seconds. New negative controls cover required
  content and language, type/ID mismatch, generation/Organization separation,
  context Claim fields, participation role and invalid endpoints.
- The new relative-date integration test passes both source readers, exact
  payload and unknown-field preservation, language-selected name/hover forms,
  reverse anchor navigation, no invented time keys, generation/person/missing
  anchor refusal, and the legacy carrier's refusal of new anchor vocabulary.
- `python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_claim_commands.py`:
  23 tests passed, 163.413 seconds.
- `python -m unittest discover -s access/tests -p test_knowledge_contract.py`:
  58 tests passed, 30.100 seconds.
- Source catalog, source-claim graph and corpus-index builders completed.
  Source-foundation, source-claim graph and source-home validators passed
  after the real source creation.

The initial new contract test failed because the profile did not exist, then
passed after its schema/registry implementation. An integration assertion
initially queried the legacy display `ru` slot instead of the language-selected
form contract; the test was corrected to inspect `select_human_forms`, and
no product fallback or source wording was weakened. Initial creation refused
a missing parent directory before any subject write; after preparing the
bounded parent, the ordinary transaction succeeded.

## Real reader observation

The unchanged `ToSAccessCore` loaded the rebuilt corpus. Every new subject was
found in both `source-claims` and `source-navigation` with its complete exact
source record, Russian name/hover and English name. All fifteen Claim source
payloads and Russian statements survived, with the entire Claim as mandatory
form context and no admission. The four temporal values kept distinct value
and Claim identities. Undeclared calendar/year numbering produced no numeric
sort bounds; the relative date had no absolute bounds.

Bounded `knowledge_focus` calls at depth 2, limits 500 nodes/1000 relations,
verified episode ↔ person and phase ↔ person ↔ existing JGB Work. This is a
two-step route through authorship, not a claim that JGB was written in Basel.
The episode focus returned 9 nodes/10 relations in 0.307 seconds; the phase
19/26 in 0.353; the Work 130/193 in 0.743. The three person calls returned
286/641 in 0.654, 1.717 and 0.843 seconds.

Cold graph construction was 37.148 seconds, peak RSS 1,272,160 KiB, on
Python 3.14.7 / Linux 7.1.13-200.fc44.x86_64. The exact observed snapshot was
`9527de0e1825e3fbdfabdfb892a832e9caf4a0ee62e6a677b40dc0c0948a6ebb`.
Other local validation ran concurrently. These are single observations, not
isolated p95 measurements, latency-budget acceptance or an incremental writer.
Later documentation rebuilds can change the corpus snapshot without changing
what this observation measured.

## Remaining work and rollback

HistoricalGeneration has a tested contract but no new real cohort here.
Real cultural, religious and scientific-technological environment accounts,
the remaining historical/space/migration relationships, biography/time-scale
lenses and independent substantive quality assessment remain open under the
ToS source and research owners. The coverage map keeps H02/H04/H05 partial.
Source creation still scans metadata and cold reading remains expensive.
Worker/D1, actual UI interaction, broader corpus growth cost, CI, merge and
deployment are not verified by this change.

Rollback of a derived reader must preserve these new source packages and
their histories. A predecessor that cannot understand the new profiles must
report that incompatibility, not coerce or delete their sources. Source
correction continues through the separately scoped versioned owner commands;
identity criterion changes and later admission remain distinct operations.
