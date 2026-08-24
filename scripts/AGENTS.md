# AGENTS.md

This card applies to deterministic generators, validators, lane loaders, and
small command-plane helpers under `scripts/`.

## Read before editing

Read the root `AGENTS.md`, then the owner surfaces that carry the action:

- [`docs/validation/validation_lanes.json`](../docs/validation/validation_lanes.json)
  is command authority; [`docs/validation/SCRIPT_TOPOLOGY.md`](../docs/validation/SCRIPT_TOPOLOGY.md)
  and [`docs/validation/script_inventory.json`](../docs/validation/script_inventory.json)
  describe script coverage and side effects.
- Read the nearest source, contract, mechanics, decision, public-entry, or
  review card for the route being changed. Common entrypoints are
  `ToS/source-witnesses/README.md`, `ToS/doctrine/CORPUS_FOUNDATION.md`,
  `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`,
  `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`, and
  `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`.
- For decision indexes, read `docs/decisions/AGENTS.md`; for agent-route
  topology, read `DESIGN.AGENTS.md` and
  `docs/validation/agents_route_inventory.json`.

## Local role

Scripts are a bounded command and evidence plane. They build generated
companions, check structural contracts, load executable validation lanes, and
produce deterministic diagnostics. They do not author ToS meaning, accept
semantic or rights judgments, become runtime policy, or replace proof, memory,
KAG, stats, eval, or deployment owners.

The source/derived split is part of every script contract: a builder may write
an explicitly owned generated companion; a validator proves its declared
mechanics only. Passing a source-witness, segmentation, semantic, translation,
graph, public-entry, or export check never proves human truth, review,
translation, rights, canon, or runtime acceptance.

## Operating Card

| Field | Route |
| --- | --- |
| role | deterministic generator, validator, or lane helper |
| input | source, contract, manifest, decision, topology, or generated companion |
| output | bounded artifact, parity result, route diagnostic, or validation signal |
| owner | exact source surface plus the script's inventory entry |
| next route | source owner -> script -> generated companion/validator -> review or release lane |
| tools | local Python, schema, manifest, unittest, and generated-parity checks |
| check | nearest lane first; use `scripts/release_check.py` for release-visible changes |

## Route by pressure

| Pressure | Stronger owner and script route |
| --- | --- |
| corpus identity, provenance, rights, or source witness | `ToS/source-witnesses/` and `ToS/doctrine/CORPUS_FOUNDATION.md`; use the source-witness builder/validator pair named by the source-foundation lane |
| text units, segmentation, translation alignment, or semantic proposals | `ToS/contracts/`, research packets, candidate intake, review ledger, and canon; use the corresponding contract validator or laboratory route |
| philosophy tree or graph workbench | `ToS/philosophy/`; use the topology, graph-view, projection, and review routes in the validation manifest |
| Zarathustra public entry or golden kernel | `ToS/zarathustra/`; use tiny-entry, source-home, lived-witness, and KAG-export checks as applicable |
| mechanics package or part | nearest `mechanics/` card and package validator; scripts only carry repeatable operation law |
| decision rationale or indexes | `docs/decisions/`; generate and validate indexes after the authored record is reviewed |
| agent route cards and disclosure | `DESIGN.AGENTS.md`, `docs/validation/agents_route_inventory.json`, and the route currentness/harness scripts |
| eval, stats, KAG, or memo port | local `evals/`, `stats/`, `kag/`, or `memo/` card, then the named sibling owner; a local receipt is not central acceptance |

## Editing posture

Keep each script narrow to its declared source route. Prefer explicit file
dependencies, stable input order, fail-closed negative controls, reproducible
transforms, and visible exit conditions. Keep private or payload-bearing data
in the owner-controlled storage route; tracked companions must state their
fixity, provenance, rights posture, uncertainty, and semantic ceiling when
those facts affect admission.

Do not hide command order in Python. `scripts/release_check.py` runs the
`release_check` sequence from `docs/validation/validation_lanes.json`.
`docs/validation/script_inventory.json` must cover every active script and
describes owner, source truth, reads, writes, side effects, lane, CI posture,
and focused test target; it is not command authority.

Generated indexes, catalogs, graph packets, public mirrors, and route
currentness are read models subordinate to their authored source. A green
validator is evidence for its named contract only. Manual source-visible
review owns bibliographic, textual, translation, semantic, rights, canon, and
public-meaning judgments.

## Boundary routes

- Authored meaning returns to `ToS/source-witnesses/`, `ToS/doctrine/`,
  `ToS/candidate-intake/`, `ToS/review-ledger/`, `ToS/canon/`, or the exact
  branch owner; compatibility and graph exports remain derived.
- Mechanics operation law returns to the nearest package/part owner.
- Runtime, MCP, Neo4j, KAG composition, stats grammar, eval verdicts, memo
  authority, memory, deployment, and proof doctrine route to their owning
  repositories or layers.
- Scope widening requires the source/contract/decision surface and its
  validator or review route to move together. Never make a validator the
  hidden source of truth.

## Validation

Use the nearest command sequence in `docs/validation/validation_lanes.json`.
The repository gate is:

```bash
python scripts/release_check.py
```

For route-card work, the focused lane is the inventory -> generated
currentness -> inheritance validator -> deterministic task harness sequence:

```bash
python scripts/build_agents_route_currentness.py --check
python scripts/validate_nested_agents.py
python scripts/agents_route_harness.py --check
```

Run affected source, mechanics, public-entry, graph, decision, test, or port
checks named by the same lane. Do not infer human, semantic, runtime, CI, or
owner acceptance from a local green result. Report changed surfaces, selected
validation, skipped checks, generated parity, boundary limits, residual risk,
and the next owner route.
