# AGENTS.md

This file applies to public schema contracts under `ToS/contracts/`.

## Role

`ToS/contracts/` defines public structure for ToS-owned surfaces. A schema edit
is a public contract change and must stay aligned with examples, canon, and
validators.

## Operating Card

| Field | Route |
| --- | --- |
| role | public schema contract surface for ToS structures |
| input | reviewed structural change in canon, compatibility, source-home, or export boundary |
| output | schema contract aligned with examples and validators |
| owner | `ToS/contracts/AGENTS.md` and the specific schema file |
| next route | source/canon/compatibility pressure -> schema update -> examples -> validators |
| tools | JSON Schema, example sync, export validation, review checklist |
| check | schema-aware validators and affected route validators |

## Boundary Routes

- Route semantic model changes through doctrine, canon, contracts, examples,
  and validators together.
- Keep `node_id` grammar stable and ToS-owned.
- Keep multilingual witness support inside one shared authored identity unless
  the node contract itself changes.
- Route runtime, graph UI, MCP, Neo4j, KAG envelope, and service behavior to
  owning runtime or downstream surfaces.

## Validation

Use the validator for the schema consumer: source-home, node contract,
tiny-entry, corpus index, public mirror, or export seam. Use the review
checklist for boundary-sensitive contract changes.
