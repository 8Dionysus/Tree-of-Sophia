# AGENTS.md

This card applies to `mechanics/` and every nested path until a nearer
`AGENTS.md` narrows the lane.

## Role

`mechanics/` is the operation topology layer for Tree of Sophia. It routes
repeatable movement around the `ToS/` source home: intake, review,
distillation, boundary handoff, obligation routing, release support, generated
companion checks, and graph/export promotion.

Mechanics are organs of operation. Authored philosophical meaning, source
witnesses, canon, Zarathustra, and the philosophy domain tree route through
`ToS/`.

## Operating Card

| Field | Route |
| --- | --- |
| input | repeatable ToS operation pressure, package route change, part contract, provenance bridge, or validation movement |
| output | mechanics package, part-local contract, provenance bridge, generated companion, or validator route |
| owner | `mechanics/AGENTS.md`, `mechanics/README.md`, `mechanics/topology.json`, and the nearest package or part route |
| next route | package `AGENTS.md` -> `README.md` -> `PARTS.md` -> `PROVENANCE.md` -> active part `README.md` |
| validation | mechanics topology, nested route-card check, package validator, or part validator |

## Boundary Routes

- Top-level shared packages follow the AoA mechanics vocabulary when the
  operation belongs to that shared family.
- A ToS-local top-level mechanic needs evidence that its operation fits no
  existing shared parent.
- Shared mechanics become ToS-local only when ToS has its own operation, owner
  split, boundary, and validation route.
- `parts/` holds active functioning operation contracts, not source-file
  inventories.
- Package `ROADMAP.md` holds future pressure and growth conditions; backlogs,
  validation inventories, script/test refactor plans, and landing ledgers route
  to their owning surfaces.
- Package `PROVENANCE.md` bridges active routes to former placement or source
  lineage.
- Package-local `legacy/` exists only for real moved-path, raw-receipt, or old
  naming accounting that returns to an active part.
- Source witnesses, philosophy branches, canon nodes, review ledgers, and the
  Zarathustra core route to `ToS/`.
- Runtime activation, proof verdicts, memory truth, SDK authority, AoA
  federation law, KAG substrate authority, and ToS canon mutation route to
  their stronger owners.

## Validation

For mechanics topology changes, run:

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
```

For package-local payload changes, run the package or part validator named by
the nearest route card, then the repository release gate when the change is
release-facing.

## Closeout

Report which mechanic package changed, whether pressure was shared or local,
which part owns the payload, whether `PROVENANCE.md` or `legacy/` was needed,
which stronger owner stayed stronger, and which validator proved the topology.
