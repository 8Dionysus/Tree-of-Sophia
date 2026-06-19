# AGENTS.md

This file applies to compatibility and scaffold example surfaces under
`ToS/public-compatibility/`.

## Role

`ToS/public-compatibility/` holds current public compatibility examples,
mirrors, and tiny-entry surfaces for Tree of Sophia. These surfaces remain
subordinate to canon and contracts.

## Operating Card

| Field | Route |
| --- | --- |
| role | public compatibility and tiny-entry example surface |
| input | reviewed canon object, contract change, public-safe route example, or bounded compatibility artifact |
| output | example payload aligned with canon, contract, and export validators |
| owner | `ToS/public-compatibility/AGENTS.md`; review archive owner for `review/` |
| next route | canon or contract -> public example -> generated export when included |
| tools | public mirror sync, KAG export generator, schema validators |
| check | public mirror sync and export validation when relevant |

## Boundary Routes

- Change canon or contracts first when an example needs new source-owned
  meaning.
- Keep `node_id` values stable, readable, and scoped to one authored object.
- Keep one shared `node_id` across multilingual witnesses unless the contract
  changes.
- Route superseded material to `ToS/public-compatibility/review/`.
- Widen public payloads only after source, canon, contract, and review surfaces
  support the wider route.

## Validation

Use public mirror sync for current examples. Use KAG export and tiny-entry
validators when the current public/export route depends on the changed surface.
