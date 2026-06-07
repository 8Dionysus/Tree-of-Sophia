# AGENTS.md

This file applies to the `ToS/philosophy/` domain tree.

## Role

`ToS/philosophy/` is the growing domain branch for philosophy inside Tree of
Sophia. It is where trunk law, era branches, region branches, tradition
subtrees, works, figures, concepts, transmissions, sources, and local graph
workbenches can expand without flattening into a warehouse.

`ToS/philosophy/` owns authored domain growth. `ToS/candidate-intake/` owns
provisional extraction before reviewed material can grow toward canonical nodes
and relation packs.

## Operating Card

| Field | Route |
| --- | --- |
| role | growing domain branch for philosophy |
| input | era, region, tradition, work, figure, concept, source-corpus, transmission, and branch-graph material |
| output | tree-shaped philosophical branch surfaces with evidence posture and local graph workbench routes |
| owner | `ToS/philosophy/AGENTS.md`, `ToS/philosophy/README.md`, and `ToS/philosophy/philosophy.manifest.json` |
| next route | branch witness -> local domain branch -> graph workbench -> review -> `ToS/canon/` promotion when ready |
| tools | branch manifests, source witness manifests, local workbench ledgers, canon route cards |
| check | `python scripts/validate_philosophy_topology.py` |

## Read First

1. Root `AGENTS.md`
2. `ToS/AGENTS.md`
3. `ToS/philosophy/README.md`
4. `ToS/philosophy/philosophy.manifest.json`
5. `ToS/doctrine/KNOWLEDGE_MODEL.md`
6. `ToS/canon/AGENTS.md` before promoting any graph object into canon

## Branch Law

- Name branches by what they are in the philosophical tree, not by UI labels,
  temporary project names, or source-page nicknames.
- Keep the main shape tree-first: trunk -> eras -> regions -> traditions ->
  works, figures, concepts, transmissions, sources, and graph fragments.
- Keep cross-thread surfaces navigational. They may help find figures, works,
  concepts, source corpora, and transmission routes, but they do not replace
  the branch where the object belongs.
- Keep local graph workbenches local to the branch they explain. Generated
  projections can summarize the whole tree later, but generated summaries do
  not own meaning.
- Preserve evidence status and uncertainty in branch manifests instead of
  smoothing it into authoritative prose.

## Boundary Routes

- Source UI labels and workbench nicknames stay in source-witness metadata.
  Repository paths describe the philosophical branch they belong to.
- Notion pages route through `ToS/source-witnesses/notion/philosophy/`; their
  branch-shaped material routes into `ToS/philosophy/`.
- Provisional extraction routes to `ToS/candidate-intake/`; reviewed authored
  nodes and relation packs route to `ToS/canon/`.
- Canon promotion needs source anchors, evidence posture, and the owning canon
  validator.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
```
