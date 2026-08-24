# Mechanics Executable Route Map

## Index Metadata

- Decision ID: TOS-D-0028
- Original date: 2026-08-24
- Surface classes: mechanics/topology, docs/route-law, scripts/validation, tests/topology, docs/validation
- ToS layers: mechanics, docs, scripts, tests, validation
- Tree classes: mechanics package, mechanics part, command lane, context boundary
- Guard families: mechanics symmetry, progressive disclosure, command authority, owner boundary, route-law, validator restraint
- Posture: accepted

## Context

The mechanics corpus already had strong package and part contracts. The root
`mechanics/README.md` named the packages, `PARTS.md` named active parts, and
the local route cards named payload and checks. The gap was executable routing:
the root route asked readers to load package `PROVENANCE.md` and `ROADMAP.md`
even when neither was relevant, while `validate_mechanics_topology.py` only
checked that package and part files existed plus moved-path accounting.

The August 2026 inventory covered 16 packages, 28 active part README routes,
202 mechanics files, and 46 repository-level documentation files. Across 121
current mechanics Markdown files, a baseline probe found 194 local Markdown
links and 65 executable script references; they resolved, but no
mechanics-owned guard would detect a later broken package/part link, stale
script reference, or unbounded growth of the always-on mechanics entrypoint.

## Decision

Keep the existing authored package, part, payload, provenance, roadmap, and
decision surfaces as their current owners. Make the root mechanics entrypoint
carry only the human meaning, package map, task-to-owner map, and progressive
disclosure route needed for first selection.

Extend the existing mechanics topology contract with one explicit
`always_on_context_budget` for `mechanics/README.md`, measured as
`whitespace_tokens_v1`. Extend its existing validator to check:

- every topology package remains in the human package map;
- every active part remains in its package `PARTS.md` selection map;
- current mechanics Markdown links resolve, excluding historical `legacy/`
  and route-card `AGENTS.md` content;
- referenced local scripts resolve from the document's package or repository
  root and are present in the descriptive script inventory;
- the always-on entrypoint stays within its declared budget.

The validation lane manifest remains the command authority. The mechanics
validator does not become a release runner, generated-doc author, runtime
status surface, or philosophical source owner. Generated decision indexes
remain lookup read models, and decisions remain weaker than current source or
mechanic contracts.

## Options Considered

- Create a second central executable-architecture inventory. This would add a
  competing root roster beside `mechanics/topology.json` and duplicate the
  package/part source map.
- Rewrite every package and part README into one large runbook. This would
  discard useful local owner contracts and repeat command or schema detail.
- Keep the current docs and rely on broad release checks. This leaves the
  concrete route-drift classes unguarded until a larger gate fails.
- Keep local contracts, make the root route progressive, and extend the
  existing topology validator with narrow currentness checks. This is the
  chosen route.

## Rationale

`TOS-D-0005` makes `mechanics/topology.json` the machine map and keeps
package/part routes local. `TOS-D-0008` keeps executable command authority out
of orientation docs. `TOS-D-0009`, `TOS-D-0011`, `TOS-D-0012`, `TOS-D-0013`,
and `TOS-D-0015` already separate lane authority, script/test inventories,
and package-local machines. This change composes those boundaries rather than
introducing another authority surface.

The guard is deliberately mechanical. It proves route existence and bounded
context, not that a philosophical reading, source interpretation, review,
release, runtime, receipt, or owner acceptance is correct.

## Consequences

New contributors can choose a package and task without preloading historical
or future-only documents. A package or part route can still expand through its
own authored contract, schema, executable, validator, test, provenance, or
decision record. A changed route link, missing inventoried script, or entrypoint
budget regression fails in the mechanics topology lane close to the owner.

The separate `AGENTS.md` documentation line remains unchanged and retains its
own route-card contract, review, and landing evidence.

## Source Surfaces

- `mechanics/README.md`
- `mechanics/topology.json`
- `mechanics/*/PARTS.md`
- `mechanics/*/README.md`
- `mechanics/*/parts/*/README.md`
- `scripts/validate_mechanics_topology.py`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `docs/testing/TEST_TOPOLOGY.md`
- `tests/test_mechanics_route_docs.py`
- `tests/test_test_topology.py`
- `TOS-D-0005-mechanics-operation-topology.md`
- `TOS-D-0008-root-validation-route-unloading.md`
- `TOS-D-0009-validation-lane-command-authority.md`
- `TOS-D-0011-script-topology-coverage.md`
- `TOS-D-0012-agon-threshold-registry-part-local-route.md`
- `TOS-D-0013-mechanics-local-test-homes.md`
- `TOS-D-0015-questbook-package-local-validator.md`

## Validation

Run the mechanics-topology, validation-authority, mechanics-local,
generated-parity, route-doc, decision-index, and release sequences in
`docs/validation/validation_lanes.json` as appropriate to the changed
surfaces. Decision-index regeneration remains owned by
`docs/decisions/AGENTS.md`.
