# Languages, writing and notation: bounded profile review, 2026-09-08

Status: implemented source/profile and local-reader slice of Foundation L01.
Not complete language/text coverage, calibrated linguistic assessment,
research admission, ancient-language competence, UI acceptance or deployment.

## Owner changes and real material

[Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md) and the
[registry explanation](../doctrine/semantic-interchange/README.md#languages-varieties-scripts-and-transliteration-schemes)
own the distinctions. Entity registry 22 declares Language, LinguisticVariety,
Script and TransliterationScheme through the existing semantic metadata reader.
Required content includes the described system, a variety's distinguishing
basis, a script's inventory scope and a scheme's convention and coverage/loss.
The existing atlas `language-script` remains navigation, not their superclass.
Description language/script and an inscription's attributed language/script
are not interchangeable fields. Unknown content remains uninterpreted.

Six subjects under [languages](../source-witnesses/languages/) use the ordinary
`source.create`, source-copy forms and exact-retry contract:

| Stable subject | Profile | Source-description boundary |
| --- | --- | --- |
| `tos.language.akkadian` | Language | described language, not every cuneiform inscription |
| `tos.language.sumerian` | Language | described language, not its writing system |
| `tos.linguistic-variety.old-babylonian` | LinguisticVariety | source-described dialect/period, not automatic museum-period attribution |
| `tos.script.cuneiform-writing-tradition` | Script | broad writing tradition, not one fixed timeless repertoire |
| `tos.script.latin-writing-tradition` | Script | writing tradition, not the Latin language |
| `tos.transliteration-scheme.oracc-atf` | TransliterationScheme | notation convention, not an executed transformation or translation |

[Seven Claims](../source-witnesses/relations/linguistic/source-claims.jsonl)
use six concrete non-transitive predicates: `inscription_language`,
`inscription_script`, `dialect_of`, `historical_language_stage_of`,
`transliteration_source_script`, `transliteration_notation_script`.
Specific domains/ranges, evidence, statement language/script, relation basis
and attestation scope are enforced. Reverse focus exposes the same Claim,
not a newly inferred historical assertion. No maximum language/script count is
imposed on a whole artifact. Scope still requires source-visible assessment.

The [bounded source reading](2026-09-08-linguistic-source-reading.md) identifies
primary pages, exact fields and inspection limits. Penn CBS 07771 keeps its
existing artifact identity and gains only its museum's Akkadian-language
report. No Penn script Claim is inferred. Louvre AO 5473 has separate language
and script reports; its inspected values are French despite the `/en/` URL.
Streck supports the qualified variety relations. The ORACC account comes from
indexed primary documentation after direct-page failures; full current ORACC
availability was not verified. No corpus payload or images were downloaded.

## Review correction and version preservation

Independent helper review found one actionable mismatch: the ATF notation
Claim's existing statement and basis identify our limited inference, while
its original `assertion_layer` said `scholarly_report`. This was not a new
proposition, attribution or judgment. Root source review accepted the narrower
classification and changed only the layer to `linguistic_analysis` using the
separately delegated [Claim correction](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#correction-of-a-declared-source-claim).

`tos_local_claim_layer_revision_owner_v1` permits exact `from/to` pairs and
only the `assertion_layer` field. Old v1/v2/v3 grants remain unchanged. The
transition has no simultaneous content/evidence/maker/visibility edit and no
admission effect. This is an extension of the existing writer boundary;
its rationale and new-Claim exclusion live in that owner contract, not in a
parallel decision or status registry.

Claim `tos.claim.linguistic.atf-notation` advanced from version 1 to 2 with
digest `sha256:bb9a31d0282ab577fdede9f6f23b790f6da621dfaf27c5e3dc55ee0f256c091b`.
Its [retained correction history](../source-witnesses/relations/linguistic/claim-revision-history.json)
contains the exact request, reason, predecessor, dependencies and archive path.
Nineteen predecessor package files were byte-verified through `inspect-version`.
Other six Claim rows and unrelated companions stayed byte-identical; the
selected form advanced with the full corrected context. Exact correction retry
returned the same receipt. Original creation retries for all subjects and the
seven-Claim package retained their original receipts and the corrected head.
The correction plus retry/archive verification took 6.784 seconds locally.

The helper's second read-only implementation review found no actionable issue
in scope, replay, shared-history reconstruction or source-bound assessment.
It did not run tests and does not provide independent historical evidence or
competence certification. Root ran the tests and real source operation below.

## Checks and measured reader behavior

Before the layer-only correction, the source graph module passed 80 tests in
225.147 seconds; the access knowledge module passed 62 in 28.575 seconds.
The existing metadata creation/correction test passed in 247.159 seconds with
all four new profile cases: required content, scope/referent preservation,
unknown fields, exact retry, archived predecessors and rebound forms.

After implementing layer correction, the new targeted test passed in 11.792
seconds and the full Claim command module passed 26 tests in 198.998 seconds.
That suite includes concurrent writes, loss before/after atomic exchange,
revocation, stale inputs, sibling history, retained creation and exact retries.
Three existing assessment boundary tests passed in 2.317 seconds: exact
source/profile scope, refusal of maker/layer shadowing, and source-copy snapshot
invalidation without OCR or a global scan. No new assessment verdict was issued.

The real reader checks both `source-claims` and `source-navigation` carriers
against all six complete source records, Russian/English names and qualified
Russian hover forms. Each declared content property selects its subject by
semantic property ID. All seven full Claims and their scoped Russian statements
remain unreviewed, without canon status or admission. Bidirectional depth-2
focus uses a single scene vertex for each shared subject identity.

The first real probe before correction took 22.003 seconds cold; its 14 focus
calls took 0.225–1.401 seconds. These are local observations, not accepted
latency budgets, an indexed-writer benchmark or UI frame measurements.
The corrected probe used snapshot
`534a80b6b46d2db6bd968798c289f535821e98f7f8ddccb0715c5a3c21a73819`:

| Local operation | First call | Repeated call |
| --- | ---: | ---: |
| Graph load, cold process | 20.691 s | not measured here |
| Catalog after graph load | 5.629 s | <0.001 s |
| Search with language kind filter | 3.992 s | 0.091 s |
| Bounded shared-identity inspection | 1.074 s | <0.001 s |
| 14 forward/reverse focus calls | 0.196–1.307 s | distinct centers/directions, not a repeated-call benchmark |

Focus returned 5–11 nodes and 4–10 relations. Maximum observed process RSS was
2,166,960 KiB with graph, catalog, search and neighborhood indexes retained;
this does not establish a storage or memory budget. Catalog was **not** measured
from a separate cold process. Worker/D1, frontend smoothness and growing-data
budgets are not implied by these measurements. Later note/index regeneration
changes the content snapshot and does not retroactively change this observation.

Reproduce the narrow automated routes from the repository root:

```bash
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_source_claim_commands -v
PYTHONPATH=mechanics/growth-cycle/tests python -m unittest test_source_commands.HistoricalCreationTests.test_semantic_description_creation_and_correction_preserve_referent_and_scope -v
PYTHONPATH=tests python -m unittest test_source_witness_bibliographic_graph -v
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
python scripts/build_tos_corpus_index.py --check
python scripts/validate_tos_corpus_index.py
```

Read-only consumption uses `ToSAccessCore.discover(root)` with
`knowledge_catalog()`, `knowledge_search('Аккадский', kind_ids=['language'])`,
`knowledge_node('tos.language.akkadian')`, and
`knowledge_focus('tos.script.latin-writing-tradition', depth=2)`.
The latter returns the ATF scheme through the exact qualified notation Claim;
inspecting that Claim exposes the full corrected source and form context.
Source paths, catalog declarations and schema bindings remain discoverable in
the ordinary API; consumers need no linguistic-profile-specific screen.

## Checklist, limits and next owner

Review checklist: **yes** for source traceability, source/derived separation,
layer distinctions, exact identity/history, explicit scope, uncertainty,
bounded extension and stronger-owner preservation. **Not applicable** for
practice lineage, counterpart mapping, compost, calibration, canon mirrors,
lived witness and a new golden-kernel pilot. There is no false human reviewer,
competence grant, ancient translation or admission. Source wording does not
become instructions or script execution. Physical artifacts were not retyped;
older metadata was not silently reconciled with new reports.

Russian names/hover and English names are source-copy-ready; that is not full
bilingual long-form assessment. Language families, executed transliteration,
text layers, lexemes/meanings, sign reading, etymology and usage change are
not completed by these four profiles. L01 remains partial in the coverage map.
The next owner is ToS language/text source and assessment, using the existing
growth mechanic; UI and Worker/D1 consumption remain their own checks. No
CI, merge, release or deployment outcome is claimed by this local note.

Rollback of a derived reader or snapshot does not remove these source records,
forms, Claims or archives. Reverting the corrected label requires another
separately delegated source transition, not manual JSONL or receipt editing.
The frozen source-reading note and historical creation files stay unchanged.
