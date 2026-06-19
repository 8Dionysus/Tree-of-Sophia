# AGENTS.md

This file applies to derived export artifacts under `ToS/derived-exports/`.

## Role

`ToS/derived-exports/` publishes generated downstream-facing read models for
Tree of Sophia. These files are consumer companions that return to authored
ToS authority.

## Operating Card

| Field | Route |
| --- | --- |
| role | generated downstream-facing read model surface |
| input | owned canon, compatibility examples, contracts, and generator logic |
| output | generated export payloads and compact read models |
| owner | `ToS/derived-exports/AGENTS.md` for route law; generator scripts for payload construction |
| next route | source-owned input or generator -> regenerate -> validate export |
| tools | KAG export generator/validator, corpus-index builder/validator, source validators |
| check | generated parity and export validation |

## Boundary Routes

- Change source-owned inputs or generator logic before changing exports.
- Keep generated fields pointing back to the source that gives them meaning.
- Keep runtime graph, UI, MCP, Neo4j, and service behavior in `abyss-stack`.
- Keep KAG, routing, and control-plane authority with owning downstream
  repositories.
- Let the corpus index cover `ToS/` as an index that returns to authored
  meaning.

## Validation

Use the relevant builder `--check` and validator for the export touched:
KAG export, root-entry map, corpus index, public mirror sync, or tiny-entry
route.
