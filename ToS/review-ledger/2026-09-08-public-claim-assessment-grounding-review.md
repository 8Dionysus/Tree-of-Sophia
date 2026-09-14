# Public Claim grounding and current assessment snapshots

Date: 2026-09-08. Reviewer: source-owner agent under the continuing Foundation
v1 mandate. This is a source/assessment boundary review, not a historical
judgment, competence attestation, Sign promotion or publication decision.

## Owned change

The public source adapter in
`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py`
now carries the declared Claim's exact typed endpoint references into the
shared engine's required-source closure. A selected human form adds its exact
Claim and inherits the same grounding. Matching native TextUnit views and
layers extend that closure through the existing full native binding; public
metadata alone does not establish text access. Origins are retained, not
multiplied by counting records or copied sources.

`describe.command_context.required_sources` exposes exact references. Every
required record must appear as support, challenge or context in the assessment
evidence. Loading all records into the engine is not equivalent to citing them.
Missing native reading permits inspection but refuses usable admission and
freeform materialization. Unrelated selected records and metadata-only native
units do not contaminate a Claim's own grounding or invalidate its assessment.

Claim forms are checked independently of whether a Claim profile resolver was
initialized. The subject must be the exact current source-selected Claim with
an understood declared profile; an inline shadow or unsupported family cannot
supply it. References are compared canonically, preserving the distinction
between integer version `1` and boolean `true`. The selected Claim identity set
is frozen before adding form dependencies: a malformed form using a Claim ID
prefix cannot bootstrap a second form's source authority.

All source-bound v2/v3/v4 commands recheck selected sources, exact file fixity,
native bindings, profile inputs and protected owner configuration after the
journal lock, before publication, on replay and before returning a current
read. Concurrent journal-head drift also refuses that view. An unpublished
blob is not committed history. A corrected endpoint can withdraw current use
without rewriting the unchanged Claim, old assessment or commit-time receipt.
Inline-only v1 and the separate private source-owner adapter retain their
existing boundaries.

This enforces the existing source-grounding doctrine; no new registry, policy,
authority, per-record human gate or source record was introduced. The closure
contains declared endpoints and their selected native returns, not arbitrary
URLs, free prose, opaque extensions or inferred references. Issuers still own
the explicit source selection, grants, competence and stable multi-file input.
The rechecks do not create a distributed transaction or protect against a
hostile same-account writer after the final observation.

## Legacy Sign gate found during profile review

The semantic-annotation-v2 validator previously counted any `sign_promotion`
review as a promotion review, including a rejected or deferred one. It now
requires `accept` or `accept_with_limits`, as the existing accepted status
already claims. Twelve schema-valid synthetic combinations of two admitted
statuses and six decisions exposed eight bypasses before the fix and passed
after it. No real reviewer was invented or relabeled, no historical review or
ID was changed, and the old schema's human-only promotion route was not
declared migrated. The future agent-capable Sign operation remains separate
unfinished work under the current corpus and knowledge-assessment doctrine.

## Verification

All new assessments, authority/competence entries, native packets and mutation
controls are synthetic temporary fixtures. The public fixture copies an
existing Letter 705 Claim and its real endpoint records, but does not assess
or admit their historical content. No retained private payload or real journal
was changed by these tests.

Tests exercise complete versus individually omitted endpoint/native evidence,
form inheritance and materialization, matching full native binding rather
than unit ID alone, public metadata-only refusal, unrelated native isolation,
retained history after relevant drift, and five concurrent source/configuration
edges. The missing-selected, inline-only and boolean-version controls each
failed before the final guard and passed after it; a wrong digest was already
refused. A further form-as-Claim control reproduced and closed the map-expansion
bypass. The final core assessment module passed 67 tests in 37.574 s. Source
foundation passed 99 tests in 124.111 s, with one declared skip for unavailable
optional private payload bytes (98 passed). Independent full regressions:
Occurrence/public-v3 8 tests / 136.749 s; private Claim v4 11 / 76.164 s;
general private v4 21 / 70.639 s. Root's native assessment regression passed
18 / 28.437 s. Source-foundation and source-home validators passed. Corpus,
documentation and agent-route companions were regenerated and validated;
56 nested cards passed. Documentation/corpus-index and script/test-topology
regressions passed 60 tests in 28.115 s. The companions are regenerated again
after recording these results, before committing their final source snapshot.

Reproduce through the owner-local modules:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests -p test_knowledge_assessment.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_occurrence_assessment_guard.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_claim_assessment.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_assessment.py
python -m unittest discover -s mechanics/growth-cycle/tests -p test_native_text_assessment.py
PYTHONPATH=tests python -m unittest test_source_witness_foundation
python scripts/validate_source_witness_foundation.py
```

## Checklist and remaining work

Source traceability, authored/derived separation, exact identity, uncertainty,
history, scoped authority and ToS/AoA boundaries: yes for this inspected change.
Canonical admission, lived witness, gold packets and translation authority:
not applicable. The independent helper reviewed the shared adapter and added
native-bound controls; root inspected its full diff, reproduced the uncovered
form bypasses and corrected them before acceptance of the change.

Actual source-visible competent assessment, arbitrary larger motif member
sets, agent-capable Sign promotion and legacy schema migration remain with
the ToS source/assessment owner. These mechanical checks establish none of
those semantic outcomes. UI integration, performance acceptance, CI, merge,
deployment and published-runtime health are not claimed here.
