# Native witness forms and assessment-input review

Date: 2026-09-07. Integration reviewer: `agent:codex-tos-foundation`.
This records a mechanical adapter and boundary review, not linguistic
assessment, historical admission, publication or Foundation v1 completion.

## Changed owner surfaces

The [human-form contract](../doctrine/HUMAN_FORMS.md#native-material-witnesses-and-scholarly-composites)
now reaches all 18 existing physical artifacts and ten native scholarly
composites. Each has an adjacent `artifact-witness.human-forms.json` or
`composite-witness.human-forms.json`, created through the ordinary protected
source-form command, not by changing its native record. Their actual
`artifact_id`/`composite_id`, source version and complete payload bind the
forms. The 28 original witness files are byte-unchanged relative to
`807332d06657042f2a165768dba89875c6d1e994`.

There are 56 current forms: one inventory/preferred name and one source-note
per subject. Current names are version 2; each original version 1 remains in
`prior_forms`. Hover forms remain version 1. The second name version replaces
whole-record context with its required custody/identity, layer, authority and
rights context. The hover still exposes the whole original record. Every
change retains the preceding package and form history through growth receipts.
No inventory number is recast as an assessed object title.

The same selected source records and current forms are available to the
source-bound assessment command. Native schemas join the exact consumed
dependency snapshot; schemas, owner paths and public visibility are checked.
Native subjects may be explicit typed Claim endpoints without injecting a
fake `record_type` or `record_id` into their actual payload. Missing endpoint
selection fails rather than causing an implicit corpus scan. Selection alone
neither performs assessment nor grants authority.

Python and Worker selectors now recognize these three exact native schema
versions, validate their actual identity field and reject shadow identities.
Both graph carriers retain the same forms and complete source bytes. No UI
files or visual heuristics changed.

## Real execution and measurements

Creation of the 56 forms and exact replay took 1.907 s; the 28 qualified name
revisions and exact replay took 1.965 s. A subsequent inspection resolved all
28 sources and 56 current forms as 84 exact assessment inputs in 0.670 s.
These are local command observations, not content-generation measurements.

The final shared-reader probe used snapshot
`a2c26d7481d4328e8c8becb37dd5b11d0bda4436c70cb1c103b8d82d95aa76d7`.
Cold construction took 28.927 s; peak RSS was 1,280,424 KiB. All 28 focused
subjects preserved exact source payload and current forms in both carriers;
focus times ranged from 0.330 to 3.839 s. The largest selected form delivery
was 15,254 bytes, below the reader's 16 KiB limit.

The first real probe exposed invalid native identity binding in the access
selector. A later probe exposed a genuine delivery-budget failure for the
Shuruppag composite with its actual long form IDs. Tests first reproduced
both failures. Repeated record-level dependencies are now deduplicated by
their complete canonical ref only: field bindings and context stay separate,
and different identities, versions or digests never merge. The output budget
and required semantic context were not weakened. The final probe passed for
every subject; the earlier failing probes are not acceptance evidence.

## Verification

Completed focused and full checks:

```bash
PYTHONPATH=tests python -m unittest test_source_witness_bibliographic_graph
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_source_commands
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_knowledge_assessment
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_human_forms
PYTHONPATH=access/src python -m unittest discover -s access/tests -p test_knowledge_contract.py
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_catalog.py --check
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python scripts/build_tos_corpus_index.py --check
python scripts/validate_tos_corpus_index.py
```

Results: final full graph suite 77 tests / 357.147 s; final full source-command
suite 38 / 309.221 s; assessment suite 49 / 7.865 s; human-form suite 22 /
0.307 s; access suite 60 / 45.958 s. The direct 28-native-record matrix passed
in 0.909 s, including all required name-context omission negatives and actual
identity-based form IDs. Worker knowledge suite passed (11 tests / 32.093 s),
the dedicated native selector parity check passed (0.854 s), and TypeScript
typechecking passed. Generated graph/corpus checks and validators passed.

One intervening full graph run failed parity while a concurrent companion
rebuild changed its snapshot. The final full run above began after the rebuild
and used stable source/graph inputs; no parity assertion was disabled.
Tests also constrain schema changes, wrong owner homes, stale source/form
versions, restricted visibility, forbidden language inference, exact retry,
omitted selected endpoints, unknown schema values and prototype-key values.

## Boundary review and remaining work

Yes: source return, persistent identity, authored/derived separation, full
editorial/material context, rights/authority boundaries and retained history.
Yes: source-copy readiness remains separate from semantic admission. Native
schemas declare no field languages; language and script remain null even when
the copied wording looks English. Territory, ancient source language and UI
locale do not create a language declaration. No former review was rewritten.
No: this is not completed bilingual coverage or an independently assessed
historical corpus. Counterpart, compost, lived-witness and canon promotion are
not applicable to this slice.

The next owner work is ToS source-visible linguistic assessment and supported
translation/freeform materialization, then the remaining corpus mappings.
The assessment-input adapter removes a format barrier, not the need for
actual competence and content review. Old source-copy forms remain inspectable
if a derived reader is rolled back. CI, merge, deployment, UI interaction,
Cloudflare/D1 runtime behavior and whole-foundation performance acceptance
were not established by these local checks.
