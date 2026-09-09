# Textual fragment and quotation passage: implementation review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, against the change
following `2735ebffe7186a07f4d737477137ea9557d7bf02`.
This is integration and boundary review, not independent source assessment,
translation acceptance, scoped admission or Foundation v1 completion.

## Executable distinction

The two declared metadata profiles distinguish an intellectual textual portion
from a particular passage transmitting it. Neither is a physical fragment,
editorial number, quotation act, reconstruction or exact text unit. Required
content describes the portion and its boundary, or the quotation and its
reported location. Common source, identity-continuity and language rules are
reused rather than copied into another reader.

Three separately identified Claims express fragment membership, the quoting
passage's containing object and limited transmission of the fragment. Their
domain/range, evidence, scope, forward and inverse readings are executable.
Competing Claims may coexist; none is transitive or establishes exact wording,
complete coverage or a physical exemplar. Registry version 17 extends the
existing reader without a Python product-code or UI-specific branch.

## Real source growth

The [frozen source-reading note](2026-09-07-burnet-fragment-quotation-source-reading.md)
binds the inspected 1908 second-edition Gutenberg representation. Ordinary
source commands created John Burnet, his *Early Greek Philosophy*, the textual
portion designated (8) there and that book's particular English quoting
passage. The existing Parmenides poem ID was reused. Four Claims connect
author, book, fragment, containing work and transmission; no Simplicius
witness, edition instance or exact ancient text was fabricated.

Preparation, creation and exact retry took 7.396, 6.012, 6.753 and 6.986
seconds for the four subjects; the four-Claim batch took 7.800 seconds.
Twelve subject forms and four Russian Claim statements materialized as ready
source copies. All remain unverified, unreviewed and unadmitted. The source
reading is one origin, not independent corroboration from each derived form.
Russian descriptions are authored paraphrases, not accepted translations.

## Common reader

The real `ToSAccessCore` retained exact source records in both source carriers,
source-language/Russian names, Russian hover context, semantic scope/content,
and complete Claim statements. Depth-2 focus bounded to 80 nodes/150 relations
returned Burnet 5/4, book 7/7, fragment 7/7 and quotation 7/7 nodes/relations,
with one selected scene vertex each. Observed times were 0.256, 0.259, 0.260
and 1.190 seconds. Cold construction took 22.271 seconds; peak RSS was
1,280,724 KiB. Probe snapshot:
`88723f378888bbac1131f860e48fe02f3c81f70ebb7f7998c3c891af15a16f98`.
Later coverage-map and review-note rebuilding changes that snapshot. These are
single local measurements, not p95 or UI performance acceptance.

The four content properties were discovered through the semantic catalog and
queried by exact property IDs. Each selected its expected source record. Nine
assessment inputs resolved exact payloads: four Claims, four new subjects and
the existing poem. Eight share the Burnet origin; the poem's prior Palmer
origin does not independently corroborate the new Burnet Claims. No assessment
was invoked.

Reproduce discovery through `ToSAccessCore.knowledge_catalog()` with
`access/src` on `PYTHONPATH`. Focus either
`tos.textual-fragment.parmenides.poem-eight` or
`tos.quotation-passage.burnet.early-greek-philosophy.parmenides-eight` using
`core.knowledge_focus`; inspect the returned Claim context before interpreting
the connection. The discovered property IDs are
`tos.property.fragment-account`, `tos.property.fragment-boundary-basis`,
`tos.property.quotation-account` and `tos.property.quotation-location`.

## Verification and limits

The complete bibliographic graph module passed 73 tests in 209.567 seconds;
the access knowledge-contract module passed 59 tests in 32.058 seconds.
The full source-command module passed 37 tests in 163.707 seconds.
The expanded common source-create/correction matrix passed in 137.141 seconds,
including these two profiles, exact retry, prior-version inspection, unknown
content retention, forms and property queries. The focused profile test passed
in 1.426 seconds and checks both source carriers, three relation flows and
reverse focus. Negative cases reject missing scope/content, blank continuity
criteria, wrong identity kinds, reversed roles, absent evidence and missing
Claim scope. Synthetic fixtures are not historical evidence.

Source catalog and graph parity, graph validation, source foundation and
source-home checks passed. Manual checklist: yes to source return, separate
identity/Claim/text layers, qualified transmission, language-neutral IDs,
explicit unknowns and unchanged authority. Canon, lived-witness consent,
calibration and public payload publication are not applicable to this diff.

B03 remains partial: exact ancient text and the Simplicius quotation route,
textual reconstruction through the scholarly-composite owner, and competent
independent assessment remain open. The physical-member composite contract
must not be filled with invented artifacts to represent textual transmission.
No actual UI interaction, Worker/D1 verification, CI, merge or deployment is
claimed here. Reader rollback preserves source packages, receipts and form
history; corrections use owner commands and do not edit frozen input notes.
