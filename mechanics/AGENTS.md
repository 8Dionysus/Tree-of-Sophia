# AGENTS.md

This card applies to `mechanics/` and every nested path until a nearer
`AGENTS.md` narrows the lane.

## Role

`mechanics/` is the operation topology layer for Tree of Sophia. It routes
shared AoA-aligned mechanics and ToS-local operation pressure without turning
the `ToS/` philosophy home into a warehouse for schemas, examples, generated
companions, guard packets, or process notes.

Mechanics are operations, not topic buckets. Use `ToS/` when authored
philosophical meaning, source witnesses, canon, Zarathustra, or the philosophy
domain tree changes.

## Read Before Editing

1. root `AGENTS.md`
2. `mechanics/README.md`
3. `mechanics/topology.json`
4. target package `AGENTS.md`, `README.md`, `PARTS.md`, `PROVENANCE.md`, and
   `ROADMAP.md`
5. target part `README.md`
6. the stronger source, canon, quest, generated, schema, or decision surface
   named by the part

## Boundaries

- Top-level shared packages follow the AoA mechanics vocabulary when the
  operation belongs to that shared family.
- A ToS-local top-level mechanic needs evidence that it cannot be a part of an
  existing shared parent.
- Shared mechanics become ToS-local only when ToS has its own operation, owner
  split, boundary, and validation route.
- `parts/` holds active functioning operation contracts, not source-file
  inventories.
- Package `PROVENANCE.md` is the active bridge to former placement or source
  lineage. It is not the archive itself.
- Package-local `legacy/` exists only for real moved-path, raw-receipt, or old
  naming accounting that returns to an active part.
- Do not create root `mechanics/legacy/`.
- Do not move source witnesses, philosophy branches, canon nodes, review
  ledgers, or the Zarathustra core into mechanics.
- Do not claim runtime activation, proof verdicts, memory truth, SDK
  authority, AoA federation law, KAG substrate authority, or ToS canon
  mutation from this lane.

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
