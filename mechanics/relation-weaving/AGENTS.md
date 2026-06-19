# AGENTS.md

This card applies to `mechanics/relation-weaving/`.

## Role

Relation Weaving is a ToS-local operation around graph fragments and canonical
relation promotion.

## Boundary Routes

Graph fragments route through `ToS/philosophy/graph-workbench/`; canonical
relations route through `ToS/canon/relations/`. This package owns promotion
operation between those surfaces.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py
```
