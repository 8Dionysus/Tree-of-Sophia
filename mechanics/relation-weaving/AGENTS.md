# AGENTS.md

This card applies to `mechanics/relation-weaving/`.

## Role

Relation Weaving is a ToS-local operation around graph fragments and canonical
relation promotion.

## Boundary

This package does not own graph objects. `ToS/philosophy/graph-workbench/` and
`ToS/canon/relations/` own the material.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_tree_relation_pack.py
```
