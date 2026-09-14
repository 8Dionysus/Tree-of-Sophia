# Knowledge coverage: explicit snapshot observations

Date: 2026-09-09 UTC. Source implementation base:
`948b365b8105c673cd7ff273119c1ef5e6ecd58f`.
This record covers a read-only access diagnostic, not semantic assessment,
Foundation acceptance or complete source-corpus migration.

## Scope and implementation

`access/src/tos_access/coverage.py` enumerates every node and relation in one
ordinary normalized snapshot. Each row retains its carrier ID, subject ID,
revision, source routes, declared type/predicate mapping, retained-record
pointers, selected display language/provenance, materialized form role states,
candidate states and next-action categories. It does not infer rights,
payload availability, language, applicability of an absent role, or truth.
Multiple carriers of one subject remain multiple carriers, not extra subjects.

Source-marked wording describes the projection's declaration, not verified
live-source authorship. Generated endpoint statements remain navigation.
Missing title/summary/explanation provenance is honored even when the display
contains an identifier or notice. Ready source-copy delivery is not an
assessment. Form admission and a parent's separate assessment are not merged
or recalculated. Ambiguity, restriction, absent adapter and over-budget
delivery remain gaps with source-return routes; there is no total quality score.

The diagnostic streams rows and accumulates bounded per-source/per-role
counters rather than retaining a second per-object corpus. It does require the
existing full normalized graph in memory. No scan was added to normalization,
hover, focus, HTTP, MCP or Worker request paths. Existing `display_coverage`
field-presence ABI totals are unchanged. No source records, source forms,
assessment journals, grants, rights or runtime settings changed.

## Observation, not migration acceptance

The adjacent [machine-readable observation](2026-09-09-knowledge-coverage-observation.json)
binds the selected projection revision and language. It covers 41,801 node
carriers and 61,721 relation carriers. All have a declared semantic mapping;
that is not substantive or all-profile completeness. The source-claims layer
has 458 carriers with form collections and 588 without; source-navigation has
297 with and 26,749 without. The two layers overlap in subject identity and
must not be summed as unique philosophical objects. Each contains 71 ambiguous
name selections under the requested English preference; this is not 71 proven
incorrect names, and no language or preferred form was invented to erase them.
The ordinary display has 40,274 missing node descriptions and 47,465 missing
relation explanations. A separate ready HumanForm may still exist for such a
carrier; these counts are not a content verdict. The final scan completed in
an owner-admitted process with 1.2 GiB peak memory and no swap. No latency
budget or benchmark improvement is claimed.

The graph can omit source objects or private payloads. Those absences are not
enumerated by a projection scan and require the source owner's catalog-to-
projection mapping. Pointers to retained records do not prove current source
byte equality. This observation deliberately does not claim currentness of
the generated input, migration of absent families, full HumanForm coverage,
historical truth, CI, merge, D1 import or UI acceptance.

## Reproduce and checks

```bash
PYTHONPATH=access/src python -m tos_access.coverage --root . --language en
PYTHONPATH=access/src python -m tos_access.coverage --root . --language en --rows
python -m pytest access/tests/test_knowledge_contract.py -q
```

The `--rows` stream ends with a summary; interruption before that summary is
not completed enumeration. A report over changed projections has a different
source revision and is a new observation, not an in-place historical update.
Outputs inherit the input's visibility; omission of wording alone is not
authorization to publish a report over restricted inputs.

Four focused tests passed. The complete knowledge-contract module passed
83 tests and 405 subtests in 38.75 seconds, with 1.3 GiB peak memory and no swap
in its owner-admitted process. Tests protect missing versus nonempty notices,
unknown mapping, duplicate subject carriers, restricted form versus available
display, invalid language, source immutability, full enumeration and incomplete
output after interruption. The first focused run failed two new fixture
assumptions (a guessed resource carrier ID and an implicit extra root node);
the fixtures were corrected to use actual normalized IDs and an explicit
selected carrier set. No production behavior was weakened to satisfy them.
Documentation-family rebuild/check and source-home validation also passed;
the two affected documentation test modules passed 43 tests and 23 subtests.

Manual review applied the source-first checklist: traceability, authored/
derived distinction, stable identity, plurality, language provenance and
assessment boundaries remain intact. Source, canon and rights changes are
not applicable. Full standalone/repository validation and generated corpus/
KAG rebuild are left to integration; this source-only diagnostic does not
claim those gates. Next owners are the exact source routes for wording,
adapter and applicability gaps, and source-witness migration for objects not
represented in the current projection. V03/M01 remain partial.
