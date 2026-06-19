# AGENTS.md

This file applies to canonical authored tree surfaces under `ToS/canon/`.

## Role

`ToS/canon/` is the canonical authored tree layer: source nodes, concept,
principle, lineage, event, state, support, analogy, synthesis nodes, relation
packs, and vocabulary registries.

## Operating Card

| Field | Route |
| --- | --- |
| role | canonical authored tree layer |
| input | reviewed source-grounded authored object, canonical relation pack, or vocabulary registry change |
| output | canonical `node.json`, `edges.csv`, or registry surface with stable identity |
| owner | `ToS/canon/AGENTS.md` and nearest class-specific `AGENTS.md` |
| next route | source witness or philosophy branch -> canon review -> compatibility mirror -> derived export |
| tools | node templates, relation contract, registries, tree validators |
| check | node contract, relation pack, example sync, and export validators |

## Boundary Routes

- Treat canon as authored meaning, not raw witness storage.
- Keep one authored object per directory-scoped `node.json`.
- Keep canonical relation packs under `ToS/canon/relations/`.
- Route raw witness material to `ToS/source-witnesses/`.
- Route compatibility mirrors through `ToS/public-compatibility/`.
- Route generated projections through `ToS/derived-exports/`.

## Validation

Use `scripts/validate_tree_node_contracts.py`,
`mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`,
and the public mirror or export validator when those surfaces are affected.
