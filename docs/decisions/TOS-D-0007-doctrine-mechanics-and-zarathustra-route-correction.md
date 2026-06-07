# Doctrine Mechanics And Zarathustra Route Correction

## Index Metadata

- Decision ID: TOS-D-0007
- Original date: 2026-06-07
- Surface classes: source-home, mechanics/topology, docs/route-law, scripts/validation, legacy/provenance
- ToS layers: doctrine, zarathustra, source-witnesses, canon, public-compatibility, derived-exports, mechanics, scripts
- Tree classes: source, concept, relation
- Guard families: owner boundary, source-first authority, mechanics placement, validator restraint, legacy containment
- Posture: accepted

## Context

The first `ToS/` landing kept too much operation material in
`ToS/doctrine/`. That made the source home look like a storehouse for process
docs, review checks, export-seam notes, and growth mechanics.

The same pass also left the current Zarathustra public route capsule under
`ToS/doctrine/`, which made the golden route look like one doctrine note among
many rather than the current axis-bearing entry route.

## Decision

Keep `ToS/doctrine/` for authored knowledge law: knowledge model, node
contract, templates, interpretation ladder, and route doctrine that belongs to
meaning.

Move repeatable operation docs to the mechanic or part that owns the operation:

- review checklist to `mechanics/audit/parts/review-ledger-route/docs/`;
- context compost to `mechanics/distillation/parts/source-compost/docs/`;
- growth and curation process notes to `mechanics/growth-cycle/parts/branch-growth-cycle/docs/`;
- manual corpus gate to `mechanics/source-witnessing/parts/witness-route/docs/`;
- KAG export posture to `mechanics/boundary-bridge/parts/derived-kag-seam/docs/`;
- rejection/branching restraint to `mechanics/agon/parts/canon-restraint/docs/`.

Create `ToS/zarathustra/` as the golden route branch for the current bounded
*Thus Spoke Zarathustra* prologue entry. The branch holds the public-entry route
surface and route capsule while source text remains in `ToS/source-witnesses/`
and canonical authored nodes remain in `ToS/canon/`.

## Rationale

The source home should be tree-shaped, not a warehouse. Mechanics are
repeatable operations and belong in `mechanics/<package>/parts/<part>/`.
Zarathustra is neither a generic mechanics surface nor merely another canon
directory; it is the current bounded route axis that orients the first public
entry.

Validator changes should protect this owner split. They should verify branch
ownership, route refs, generated parity, and authority boundaries. They should
not treat the old `ToS/doctrine/` pile as a truth source.

## Consequences

`scripts/validate_tos_source_home.py` now recognizes `ToS/zarathustra/` as a
source-home branch.

`scripts/validate_tiny_entry_route.py`, `scripts/generate_kag_export.py`,
`scripts/validate_kag_export.py`, and root-entry map generation now point to
the Zarathustra route branch instead of `ToS/doctrine/`.

Mechanic part route cards now point to their part-owned docs instead of using
`ToS/doctrine/` as an operation store.

Historical review notes keep their old path evidence. Current route cards and
validators use the new owner surfaces.

## Source Surfaces

- `ToS/doctrine/`
- `ToS/zarathustra/`
- `ToS/zarathustra/AGENTS.md`
- `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
- `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`
- `mechanics/distillation/parts/source-compost/docs/CONTEXT_COMPOST.md`
- `mechanics/growth-cycle/parts/branch-growth-cycle/docs/`
- `mechanics/source-witnessing/parts/witness-route/docs/MANUAL_CORPUS_ENTRY_GATE.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
- `scripts/validate_tos_source_home.py`
- `scripts/validate_tiny_entry_route.py`
- `scripts/validate_kag_export.py`
- `scripts/validate_nested_agents.py`

## Validation

Run:

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_tos_source_home.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
