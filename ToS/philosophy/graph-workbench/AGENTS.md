# AGENTS.md

This card applies to `ToS/philosophy/graph-workbench/`.

## Role

`graph-workbench/` owns source-side graph preparation for the philosophy
branch: view lenses, cluster contracts, review-packet contracts, proposed nodes,
proposed relations, language packets, branch fragments, and promotion ledgers before canon.

It does not own runtime rendering, Neo4j storage, MCP service behavior, or UI
state. Those routes belong to `abyss-stack` after ToS exports the generated
projection.

## Operating Card

| Field | Route |
| --- | --- |
| input | branch review pressure, atlas rows, dossier graph-shape material, proposed node/relation pressure, graph lens pressure |
| output | source-owned graph lens, cluster, review-packet, proposed graph, or promotion surface |
| owner | nearest graph-workbench branch card and `ToS/philosophy/philosophy.manifest.json` |
| next route | view contract -> generated projection -> abyss-stack API/UI/MCP/Neo4j projection |
| check | philosophy topology, graph view catalog, graph projection, release check |

## Branch Routes

| Branch | Owns |
| --- | --- |
| `views/` | source-owned graph lenses and switching contracts |
| `clusters/` | source-owned collapse and overview contracts |
| `review-packets/` | source-owned review packet shape for agents and operator review |
| `proposed-nodes/` | pre-canon node pressure |
| `proposed-relations/` | pre-canon relation pressure |
| `language-packets/` | pre-canon text-bearing original/ru/en language packets |
| `branch-fragments/` | local branch graph fragments before promotion |
| `promotion-ledger/` | route from workbench pressure toward canon relation packs |
| `PLANTING_INTERFACE.md` | source-owned packet route from prepared atlas/dossier material into graph-workbench review |

## Boundary

- Keep lens, cluster, and review-packet meaning in ToS.
- Keep runtime filtering, layout, browser rendering, MCP serving, and Neo4j
  materialization in `abyss-stack`.
- Do not turn generated projection caches into canon.
- Do not invent source facts to satisfy a cluster contract; emit unresolved
  diagnostics when current atlas data is not mature enough.

## Verify

Use the narrowest graph route first:

```bash
python scripts/build_philosophy_graph_views.py --check
python scripts/validate_philosophy_graph_views.py
python scripts/build_philosophy_graph_projection.py --check
python scripts/validate_philosophy_graph_projection.py
python scripts/validate_philosophy_topology.py
```
