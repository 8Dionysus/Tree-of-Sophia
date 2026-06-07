# AGENTS.md

This file applies to the `ToS/philosophy/` domain tree.

## Role

`ToS/philosophy/` is the growing domain branch for philosophy inside Tree of
Sophia. It is where trunk law, era branches, region branches, tradition
subtrees, works, figures, concepts, transmissions, sources, and local graph
workbenches can expand without flattening into a warehouse.

This branch is not `candidate-intake/`. Candidate intake remains a quarantine
surface for provisional extraction. `ToS/philosophy/` is the authored domain
body that can grow toward canonical nodes and relation packs.

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

## Stop Lines

- Do not create `zagotovki`, `world-written-philosophy`, `raw-pages`, or other
  flat source-label paths inside this branch.
- Do not make `ToS/philosophy/` a database dump, a Notion mirror, or a hidden
  replacement for `ToS/canon/`.
- Do not promote a work, figure, concept, or relation into canon until the
  branch has enough source anchors and review posture to support it.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
```
