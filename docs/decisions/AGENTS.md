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

## Index and validation

Maintain the decision index in [README.md](README.md) by hand. Review each record and validate its changed source through the owning route.
