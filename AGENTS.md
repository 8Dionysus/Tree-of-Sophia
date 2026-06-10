# AGENTS.md

Root route card for `Tree-of-Sophia`.

## Applies To

This card applies to the whole repository unless a nearer nested `AGENTS.md`
narrows the lane.

## Role

`Tree-of-Sophia` owns the source-first philosophical tree of the AoA ecosystem:
source witnesses, doctrine, nodes, relations, branch growth, canon formation,
public entry seams, and bounded export seams for downstream consumers.

This card keeps agent work inside that role and routes to `README.md`,
`CHARTER.md`, `DESIGN.md`, `DESIGN.AGENTS.md`, `BOUNDARIES.md`, `ToS/`,
mechanics packages, source files, decisions, builders, validators, and local
owner truth when those surfaces own the answer.

## Operating Map

| Field | Route |
| --- | --- |
| input | source material, philosophical node pressure, branch growth, route-law change, mechanic pressure, public seam, or graph/export handoff |
| output | source-owned ToS surface, mechanic-local contract, generated companion, decision record, or stronger-owner handoff |
| owner | authored surfaces under `ToS/`, repeatable operations under `mechanics/`, durable rationale under `docs/decisions/` |
| next route | nearest nested `AGENTS.md`, source branch, mechanic package, builder, validator, test, or sibling owner |
| validation | [Verify](#verify), plus the nearest local route card |

## Read Before Editing

For first orientation:

1. `README.md`
2. `ROADMAP.md`
3. `CHARTER.md`
4. `DESIGN.md`
5. `DESIGN.AGENTS.md`
6. `BOUNDARIES.md`
7. `ToS/README.md`
8. `mechanics/README.md`

For actual edits:

1. this `AGENTS.md`
2. nearest nested `AGENTS.md` for every touched path
3. the route-mode surface from the table below
4. the nearest source file, branch manifest, doctrine card, mechanic part,
   schema, builder, validator, test, or generated-source owner
5. the narrowest relevant validator before broader gates

Use `docs/AGENTS_ROOT_REFERENCE.md` only to audit preserved root-reference rules.
If a preserved rule still governs current work, move it to the owning surface
instead of re-bloating this card.

## Route Modes

| Route mode | Use when | First surface |
| --- | --- | --- |
| `first-reading` | you need the shortest honest overview | [README](README.md) |
| `authority-boundary` | repository authority, owner split, or source-of-truth pressure changes | [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) |
| `system-design` | tree form, source/generated posture, or repository shape changes | [DESIGN](DESIGN.md) |
| `agent-surface-design` | route-card shape or agent guidance changes | [DESIGN.AGENTS](DESIGN.AGENTS.md) |
| `source-home` | ToS source home, doctrine, philosophy tree, canon, review ledger, or branch topology changes | [ToS/AGENTS](ToS/AGENTS.md) |
| `zarathustra-route` | golden Zarathustra entry, trilingual capsule, public tiny entry, or current root route changes | [ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) |
| `mechanic-change` | repeatable operation topology, package route, part contract, provenance, or validation changes | [mechanics](mechanics/README.md), package `AGENTS.md`, package `ROADMAP.md`, and active part route |
| `derived-export` | generated downstream read models, KAG handoff, corpus index, or export seam changes | source surface -> builder -> generated output -> validator |
| `direction-change` | roadmap, release contour, future trigger, or repo-level direction changes | [ROADMAP](ROADMAP.md) |
| `decision-rationale` | durable route, boundary, workflow, public-contract, or topology rationale changes | [docs/decisions](docs/decisions/README.md) |
| `root-reference-audit` | a preserved old root rule must be inspected | [docs/AGENTS_ROOT_REFERENCE](docs/AGENTS_ROOT_REFERENCE.md) |

## Boundary Law

- Authored ToS surfaces own ToS meaning. Generated, exported, compact, public,
  graph, runtime, and downstream surfaces support or transport meaning.
- Mechanics are organs of operation: package and part-local machines for
  repeatable movement. Keep the root and `ToS/` home clear of scripts, tests,
  validators, and generic process notes.
- Zarathustra is the current golden route. Treat it as a high-pressure source
  branch rather than a generic canon bucket.
- Source witnesses, doctrine, canon, branch manifests, review ledgers,
  decisions, builders, validators, and tests each keep different kinds of
  authority. Keep those layers separated and routed.
- Route runtime, proof, memory, stack, KAG substrate, federation, playbook,
  skill, and technique authority to the owning AoA repository or layer.

## Decision Review

After structural, ownership, workflow, route-law, validator-authority,
public-contract, export, graph, mechanic, or topology changes, check whether
future agents need a decision record to understand why the path was chosen.
Use [docs/decisions](docs/decisions/README.md) for the local rule.

If no new decision is needed, say so in closeout.

## GitHub Landing Workflow

Root `AGENTS.md` owns the repository-wide branch, PR, CI, and merge route.
`.github/AGENTS.md` owns GitHub-native files that support it.

When the user asks to commit, push, and merge in this repository:

1. Start from a branch based on current `origin/main`. If the worktree is dirty,
   inventory it first and carry forward only the intended diff.
2. Commit the intended change with a message that names the changed surface.
3. Push the branch and open a pull request that states changed surfaces,
   validation run, skipped checks, and remaining risk.
4. Wait for GitHub checks. If a check fails, fix the branch and wait for the
   new result.
5. Merge through GitHub after green validation, then fast-forward local `main`
   and confirm the worktree state before closeout.

If GitHub status or merge permissions cannot be observed, report the exact
blocker instead of guessing.

## Update Surfaces

When a source-backed change moves, update the smallest matching companions:

- root or source-home docs when public route or owner posture changes;
- `ROADMAP.md` when current direction, phase, or release contour changes;
- `CHANGELOG.md` when release-visible behavior, validation, public docs, or
  generated surfaces change;
- decision records when future agents need the rationale;
- generated outputs, builders, validators, and tests when their source-backed
  contract actually moved.

Leave scripts, validators, and tests untouched unless the current change moves
a checked contract. Validator topology is its own future refactor.

## Route Away When

- a ToS node becomes runtime behavior, memory authority, proof doctrine,
  routing policy, playbook choreography, or KAG substrate;
- a public/export seam is treated as stronger than ToS-authored authority;
- a root doc starts carrying branch inventory, validator sprawl, or generic
  process lore that belongs in a local card;
- a mechanic becomes a hidden replacement for source, doctrine, canon, review,
  or decision authority.

## Verify

For root docs, route-card, current contour, public route, and bounded export
changes, run the narrowest relevant check first. Use the broad gate only when
the change crosses owner surfaces or release-visible contracts.

Broad gate:

```bash
python scripts/release_check.py
```

Current public route and export spot checks:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
```

Use [scripts/AGENTS](scripts/AGENTS.md) for script-local owner routes and
targeted generator checks. Use [docs/decisions](docs/decisions/README.md) when
the change creates durable route, boundary, validator, export, or source
rationale.

For interpretive, structural, or boundary-sensitive changes outside that
perimeter, use:

`mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`

## Report

Close out with changed surfaces, validation run, skipped checks, decision
review result, remaining risk, and the next owner route.
