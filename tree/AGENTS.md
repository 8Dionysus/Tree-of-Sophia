# AGENTS.md

This file applies to canonical authored tree surfaces under `tree/`.

## Read first

Before editing anything here, read:
1. the repository root `AGENTS.md`
2. `docs/KNOWLEDGE_MODEL.md`
3. `docs/NODE_CONTRACT.md`
4. the relevant `*_NODE_TEMPLATE.md` file
5. the exact `node.json`, `edges.csv`, or registry surface you plan to change

## Local role

`tree/` is the canonical authored tree layer for Tree of Sophia.

It holds:
- canonical authored source nodes
- concept, principle, lineage, event, state, support, analogy, and synthesis nodes
- canonical relation packs
- vocabulary governance registries
- future authored context-node surfaces

The current route uses directory-scoped `node.json` files as canonical payloads and route-local `edges.csv` files as canonical relation packs.

## Editing posture

Treat `tree/` as authored meaning, not as raw witness storage and not as a derived export layer.

Keep these rules sharp:
- one authored object per directory-scoped `node.json`
- explicit relation packs under `tree/relations/`
- stable node identity
- explicit, bounded relation names
- authored interpretation kept visibly distinct from raw witness text

`examples/*.example.json` are compatibility mirrors of this layer, not a second canon.
`generated/` is downstream-facing derivative output, not a rival authority surface.

Quest or RPG reflection vocabulary must remain adjunct-only. It may annotate routes elsewhere, but it must not rename node classes, relation law, or authorship posture inside the canonical tree.

## Hard no

Do not:
- move raw witness storage into `tree/`
- flatten multiple authored objects into one node because they feel related
- let compatibility mirrors drift and then treat the drift as a new canon
- smuggle AoA routing or runtime control semantics into authored node law

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_intake_pack.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```

If the change touches the current bounded public route, also run:

```bash
python scripts/validate_tiny_entry_route.py
```
