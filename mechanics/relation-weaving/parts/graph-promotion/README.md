# Graph Promotion

## Operating Card

| Field | Route |
| --- | --- |
| role | route graph fragments toward reviewed relation packs |
| input | proposed node, proposed relation, branch fragment |
| output | relation-pack promotion route or return-to-review |
| owner | `mechanics/relation-weaving/parts/graph-promotion/` |
| next route | `ToS/philosophy/graph-workbench/` or `ToS/canon/relations/` |
| tools | `ToS/doctrine/RELATION_PACK_CONTRACT.md`, `tos-ops-mechanics-plan --repo-root ROOT --relation-pack-validate` |
| check | `tos-ops-mechanics-plan --repo-root . --relation-pack-validate` |

## Payload

- `tos-ops-mechanics-plan --repo-root ROOT --relation-pack-validate`

The script is local because relation-pack promotion is the repeatable
operation. The canonical relation carrier stays in `ToS/canon/relations/`, and
the candidate ledger stays in `ToS/candidate-intake/`.
