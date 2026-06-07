# Mechanics Operation Topology

## Index Metadata

- Decision ID: TOS-D-0005
- Original date: 2026-06-07
- Surface classes: mechanics/topology, docs/route-law, scripts/validation, legacy/provenance
- ToS layers: docs, mechanics, doctrine, source-witnesses, philosophy, canon, public-compatibility, derived-exports, contracts, review-ledger, scripts
- Tree classes: source, concept, principle, lineage, event, state, support, analogy, synthesis, relation
- Guard families: mechanics symmetry, owner boundary, source-first authority, legacy containment, validation route
- Posture: accepted

## Context

`Tree-of-Sophia` needed mechanics symmetry with already refactored AoA
repositories, but ToS is not a head engineering repo. It is a philosophical
source-home repository. The first mechanics draft moved obvious Agon,
Experience, and Questbook payloads, but that was too narrow: it made ToS look
as if it had only three mechanics and did not explain how shared mechanics,
ToS-local mechanics, `parts/`, and `legacy/` should form.

Sibling repositories show the durable pattern: root `mechanics/` is an atlas;
packages own active operation routes; `PARTS.md` maps functioning parts;
`PROVENANCE.md` is the active bridge to former placement; package-local
`legacy/` exists only when real moved-path or raw-receipt accounting needs to
return historical material to the current active route.

## Decision

Adopt `mechanics/` as the ToS operation atlas with two rosters:

- shared AoA-aligned packages using common parent names:
  `agon`, `antifragility`, `audit`, `boundary-bridge`, `checkpoint`,
  `distillation`, `experience`, `growth-cycle`, `method-growth`, `questbook`,
  `recurrence`, `release-support`, and `rpg`;
- ToS-local packages for operations that belong to this philosophy home:
  `source-witnessing`, `canon-formation`, and `relation-weaving`.

Move mechanics-owned Agon, Experience, and Questbook payloads into named active
parts instead of broad holding buckets. Leave source witnesses, philosophy
branches, canon nodes, review ledgers, and the Zarathustra core inside `ToS/`.

`legacy/` is package-local and conditional. It exists in `agon`, `experience`,
and `questbook` because those packages have real former-path accounting. Other
packages do not get empty `legacy/` directories merely for visual symmetry.

## Options Considered

- Keep only the three payload-bearing packages. This preserved the immediate
  moves but hid the shared mechanics layer and ToS-local operations.
- Create a root roster, root migration ledger, or root `legacy/`. This would
  make history look like an alternate active route.
- Create the shared/local package atlas, split active parts by operation, and
  let `legacy/` form only where active parts have former-path accounting.

## Rationale

This keeps symmetry appropriate rather than forced. Shared mechanics keep the
same parent names used across the AoA ecosystem, while their parts remain
ToS-local. Local mechanics name operations that only ToS can own: witness route
discipline, canon formation, and relation weaving.

The split also protects the `ToS/` home. `ToS/` remains the philosophical body:
source witnesses, philosophy branches, canon, compatibility, exports,
contracts, doctrine, and review evidence. Mechanics route operations around
that body; they do not store the body itself.

The legacy rule prevents future agents from using old paths, temporary names,
or migration history as current topology. Active work starts from the package
and part route. Historical detail is reached only through `PROVENANCE.md` and
only in packages that actually need archive accounting.

## Consequences

`mechanics/topology.json` becomes the checked machine map for active packages,
parts, class, status, and legacy eligibility.

`scripts/validate_mechanics_topology.py` rejects:

- missing shared or ToS-local packages;
- stale broad part routes;
- root `mechanics/legacy/`;
- empty package-local legacy in packages without moved-path accounting;
- old ToS/root payload paths that should now live under active mechanics parts.

Future mechanics growth must either add a part inside an existing package or
justify a new ToS-local package with a source surface, input, output, owner,
next route, tools, and check.

## Source Surfaces

- `mechanics/README.md`
- `mechanics/AGENTS.md`
- `mechanics/topology.json`
- `mechanics/agon/`
- `mechanics/experience/`
- `mechanics/questbook/`
- `ToS/README.md`
- `ToS/AGENTS.md`
- `ToS/source_home.manifest.json`
- `scripts/validate_mechanics_topology.py`
- `scripts/release_check.py`

## Validation

Run:

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
