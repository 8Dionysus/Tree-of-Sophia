# AGENTS.md

This file applies to canonical authored tree surfaces under `ToS/canon/`.

## Read first

Before editing anything here, read:
1. the repository root `AGENTS.md`
2. `ToS/doctrine/KNOWLEDGE_MODEL.md`
3. `ToS/doctrine/NODE_CONTRACT.md`
4. the relevant `*_NODE_TEMPLATE.md` file
5. the exact `node.json`, `edges.csv`, or registry surface you plan to change

## Local role

`ToS/canon/` is the canonical authored tree layer for Tree of Sophia.

It holds:
- canonical authored source nodes
- concept, principle, lineage, event, state, support, analogy, and synthesis nodes
- canonical relation packs
- vocabulary governance registries
- future authored context-node surfaces

The current route uses directory-scoped `node.json` files as canonical payloads and route-local `edges.csv` files as canonical relation packs.

## Operating Card

| Field | Route |
| --- | --- |
| role | canonical authored tree layer |
| input | reviewed source-grounded authored object, canonical relation pack, or vocabulary registry change |
| output | canonical `node.json`, `edges.csv`, or registry surface with stable identity |
| owner | `ToS/canon/AGENTS.md` and nearest class-specific `AGENTS.md` |
| next route | source witness or philosophy branch -> canon review -> compatibility mirror -> derived export |
| tools | node templates, relation contract, registries, tree validators |
| check | node contract, relation pack, example sync, and export validators |

## Editing posture

Treat `ToS/canon/` as authored meaning. Raw witness storage routes to
`ToS/source-witnesses/`; generated projections route to `ToS/derived-exports/`.

Keep these rules sharp:
- one authored object per directory-scoped `node.json`
- explicit relation packs under `ToS/canon/relations/`
- stable node identity
- explicit, bounded relation names
- authored interpretation kept visibly distinct from raw witness text

`ToS/public-compatibility/*.example.json` are compatibility mirrors of this
layer. `ToS/derived-exports/` is downstream-facing derivative output. Canon
keeps authored authority.

Quest or RPG reflection vocabulary must remain adjunct-only. It may annotate routes elsewhere, but it must not rename node classes, relation law, or authorship posture inside the canonical tree.

## Boundary Routes

- Raw witness storage routes to `ToS/source-witnesses/`.
- Related authored objects keep separate identities unless a reviewed template
  defines one shared object.
- Compatibility drift routes through example sync and canon review.
- AoA routing or runtime control semantics route to their owning repositories
  or layers.

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
