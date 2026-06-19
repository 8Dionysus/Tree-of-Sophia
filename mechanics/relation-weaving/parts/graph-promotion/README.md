# Graph Promotion

## Operating Card

| Field | Route |
| --- | --- |
| role | route graph fragments toward reviewed relation packs |
| input | proposed node, proposed relation, branch fragment |
| output | relation-pack promotion route or return-to-review |
| owner | `mechanics/relation-weaving/parts/graph-promotion/` |
| next route | `ToS/philosophy/graph-workbench/` or `ToS/canon/relations/` |
| tools | `ToS/doctrine/RELATION_PACK_CONTRACT.md`, `mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py` |
| check | `python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py` |

## Payload

- `mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`

The script is local because relation-pack promotion is the repeatable
operation. The canonical relation carrier stays in `ToS/canon/relations/`, and
the candidate ledger stays in `ToS/candidate-intake/`.
