# AGENTS.md

Root route card for `Tree-of-Sophia`.

## Applies To

This card applies to the whole repository unless a nearer nested `AGENTS.md`
narrows the lane.

## Role

`Tree-of-Sophia` owns the source-first philosophical tree of the AoA ecosystem:
source witnesses, doctrine, nodes, relations, branch growth, canon formation,
public entry seams, and bounded export seams for downstream consumers.

Root `AGENTS.md` routes work. It does not replace `ToS/`, `mechanics/`,
decision records, validators, or neighboring AoA owners.

## Operating Card

| Field | Route |
| --- | --- |
| input | source material, philosophical node pressure, branch growth, route-law change, mechanic pressure, public seam, or graph/export handoff |
| output | source-owned ToS surface, mechanic-local contract, generated companion, decision record, or stronger-owner handoff |
| owner | authored surfaces under `ToS/`, repeatable operations under `mechanics/`, durable rationale under `docs/decisions/` |
| next route | nearest nested `AGENTS.md`, source branch, mechanic package, builder, validator, test, or sibling owner |
| validation | [Verify](#verify), plus the nearest local route card |

## Read Before Editing

For orientation, read only the route you need:

| Work | First route |
| --- | --- |
| overview or repository identity | [README](README.md), [CHARTER](CHARTER.md), [BOUNDARIES](BOUNDARIES.md) |
| current direction | [ROADMAP](ROADMAP.md) |
| system shape | [DESIGN](DESIGN.md) |
| agent-surface shape | [DESIGN.AGENTS](DESIGN.AGENTS.md) |
| ToS source home or branch work | [ToS/AGENTS](ToS/AGENTS.md), then the owning branch card |
| golden Zarathustra route | [TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) |
| mechanics | [mechanics](mechanics/README.md), package `AGENTS.md`, package `ROADMAP.md`, active part route |
| decision rationale | [docs/decisions](docs/decisions/README.md) |
| preserved old root rule | [docs/AGENTS_ROOT_REFERENCE](docs/AGENTS_ROOT_REFERENCE.md) |

For actual edits, read the nearest nested `AGENTS.md`, the owning source file
or manifest, and the narrowest relevant validator before broader gates.

## Boundary Law

- Authored ToS surfaces own ToS meaning.
- Generated, exported, compact, public, graph, runtime, and downstream surfaces
  support or transport meaning.
- Mechanics are organs of repeatable movement, not a second source home.
- Zarathustra is the current golden route, not a generic sample.
- Runtime, proof, memory, stack, KAG substrate, federation, playbook, skill,
  and technique authority route to their owning AoA repositories or layers.

## Change Companions

When a source-backed change moves, update only the smallest matching companions:
root or source-home docs, `ROADMAP.md`, `CHANGELOG.md`, decision records,
generated outputs, builders, validators, or tests. If no durable rationale
moved, say that no new decision record was needed.

Use `docs/AGENTS_ROOT_REFERENCE.md` only to audit preserved root-reference
rules. If a preserved rule still governs current work, move it to the owning
surface instead of re-bloating this card.

## Landing Route

When the user asks for landing, use a branch based on current `origin/main`,
commit the intended diff, push, open a PR, wait for GitHub checks, merge after
green validation, then fast-forward local `main`. `.github/AGENTS.md` owns
GitHub-native file details; `docs/RELEASING.md` owns release publication
guidance.

If GitHub status or merge permissions cannot be observed, report the exact
blocker instead of guessing.

## Route Away When

- a ToS node becomes runtime behavior, memory authority, proof doctrine,
  routing policy, playbook choreography, or KAG substrate;
- a public/export seam is treated as stronger than ToS-authored authority;
- a root doc starts carrying branch inventory, validator sprawl, or generic
  process lore that belongs in a local card;
- a mechanic becomes a hidden replacement for source, doctrine, canon, review,
  or decision authority.

## Verify

Use the narrowest relevant check first. Use the broad gate when the change
crosses owner surfaces or release-visible contracts.

Broad gate:

```bash
python scripts/release_check.py
```

Current public route and export spot checks:

```bash
python scripts/validate_tiny_entry_route.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```

Use [scripts/AGENTS](scripts/AGENTS.md) for script-local owner routes and
targeted generator checks. Use
`mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md` for
interpretive, structural, or boundary-sensitive changes outside validator
coverage.

## Report

Close out with changed surfaces, validation run, skipped checks, decision
review result, remaining risk, and the next owner route.
