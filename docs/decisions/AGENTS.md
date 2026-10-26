# AGENTS.md

This card applies to durable rationale under `docs/decisions/`.

## Role

Records preserve durable decision rationale for a route, topology, owner
boundary, validator, export posture or rejected alternative. Current meaning and behavior stay with
the source the record describes; dated review evidence stays in its review home.

## Record shape

Use the canonical `TOS-D-####` ID and filename
`docs/decisions/TOS-D-####-short-slug.md`. Each note has an
`## Index Metadata` block with Decision ID, Original date, Surface classes,
ToS layers, Tree classes, Guard families and Posture. Start from
[TEMPLATE.md](TEMPLATE.md).

Preserve existing IDs. A changed choice gets an explicit superseding record
that names the affected scope; historical rationale stays readable. Ordinary
documentation cleanup alone does not require a new record.

## Boundary Routes

Generated indexes are lookup views, weaker than current source authority. Keep `modeled_surfaces` in
`docs/decisions/indexes/index_contract.yaml` as a top-level list of normalized
repo-relative paths under `docs/decisions/`; root non-record Markdown does not
belong in that list.

## Validation

Use [docs/VALIDATION.md](../VALIDATION.md) to regenerate indexes and check
record shape, then validate the changed source through its owner. Index parity
does not accept a decision or prove its implementation.
