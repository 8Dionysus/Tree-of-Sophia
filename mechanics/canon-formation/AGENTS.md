# AGENTS.md

This card applies to `mechanics/canon-formation/`.

## Role

Canon Formation is a ToS-local operation around reviewed promotion into canon.

## Boundary Routes

Reviewed promotion routes to `ToS/canon/`. This package owns the promotion
operation around canonical nodes, relations, and registries.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_tree_node_contracts.py
python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py
```
