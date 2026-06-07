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
| input | era, region, tradition, work, figure, concept, source-corpus, transmission, research packet, and branch-graph material |
| output | tree-shaped philosophical branch surfaces with evidence posture and local graph workbench routes |
| owner | `ToS/philosophy/AGENTS.md`, `ToS/philosophy/README.md`, and `ToS/philosophy/philosophy.manifest.json` |
| next route | source witness or research packet -> local domain branch -> source anchoring -> graph workbench -> review -> `ToS/canon/` promotion when ready |
| tools | branch manifests, research packet manifests, source witness routes, local workbench ledgers, canon route cards |
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

- Capture page labels and workbench nicknames stay in metadata.
  Repository paths describe the philosophical branch they belong to.
- AI-generated Deep Research material routes through
  `ToS/research-packets/deep-research/philosophy/`; it is not a source witness
  and cannot authorize philosophical claims.
- Real source witnesses route through `ToS/source-witnesses/`.
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
