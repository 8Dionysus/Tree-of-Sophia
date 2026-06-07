# AGENTS.md

This file applies to the `ToS/` source home.

## Role

`ToS/` is the source-home organ for Tree of Sophia.
It holds ToS-authored meaning, source witnesses, candidate intake, canonical
node and relation canon, public compatibility mirrors, derived exports,
contracts, domain philosophy topology, and review ledgers as one tree-shaped
home.

Root repository files remain the public front door, release route, and
repository-wide landing surface. `docs/decisions/` remains the durable
decision rationale lane.

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home organ for Tree of Sophia |
| input | source witnesses, philosophy branches, candidate extraction, canon payloads, compatibility examples, generated exports, contracts, doctrine, and review notes |
| output | a branch-shaped ToS surface with current owner guidance and validation |
| owner | `ToS/AGENTS.md` for home law; nearest nested `AGENTS.md` for branch law; `ToS/source_home.manifest.json` for the branch inventory |
| next route | witness -> philosophy or candidate intake -> canon -> public compatibility -> derived export |
| tools | source-home manifest, branch manifest, schema, generator, validator, and decision record when durable rationale is needed |
| check | `python scripts/validate_tos_source_home.py`, `python scripts/validate_philosophy_topology.py`, `python scripts/validate_nested_agents.py` |

## Read First

1. Root `AGENTS.md`
2. `ToS/README.md`
3. `ToS/source_home.manifest.json`
4. `ToS/doctrine/KNOWLEDGE_MODEL.md`
5. `ToS/doctrine/NODE_CONTRACT.md`
6. The nearest nested `AGENTS.md` for the touched branch

## Branches

| Branch | Input | Output |
| --- | --- | --- |
| `doctrine/` | ToS knowledge-law pressure, route posture, templates | current doctrine and review law |
| `source-witnesses/` | primary source-facing material and source-page witnesses | provenance-aware witness surfaces |
| `philosophy/` | branch-shaped philosophical domains, eras, regions, traditions, works, figures, concepts, transmissions | authored domain topology and local graph workbenches |
| `candidate-intake/` | provisional extraction and tabular passes | reviewable candidate structure with explicit uncertainty |
| `canon/` | reviewed authored objects and relation packs | canonical nodes, relations, and vocabulary registries |
| `public-compatibility/` | public-safe mirrors and examples | compatibility examples and tiny-entry surfaces |
| `derived-exports/` | generated projections from owned surfaces | downstream-facing read models |
| `contracts/` | structural commitments for public ToS surfaces | schemas and public contracts |
| `review-ledger/` | dated inspection evidence | review notes and provenance of prior checks |

## Boundary Routes

- Treat `ToS/` as one tree-shaped home with branch owner surfaces.
- Keep philosophy, witness, intake, canon, compatibility, export, and contract layers
  visible as different branches.
- Keep `ToS/canon/` stronger than compatibility mirrors and derived exports.
- Keep `ToS/source-witnesses/` stronger than candidate intake.
- Keep `ToS/philosophy/` as the domain-shaped growth branch. Flat imports route
  to the witness or intake owner; provisional extraction routes to
  `ToS/candidate-intake/`.
- Keep doctrine in `ToS/doctrine/`; durable rationale remains in
  `docs/decisions/`.
- Route active ToS-owned source surfaces into the source home instead of
  recreating root-level active homes.

## Validation

For source-home topology changes, run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_philosophy_topology.py
python scripts/validate_nested_agents.py
```

For route, canon, compatibility, contract, or export changes, continue with
the owning validator named by the nested branch route card.
