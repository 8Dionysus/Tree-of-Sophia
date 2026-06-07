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

## Read First

1. Root `AGENTS.md`
2. `ToS/README.md`
3. `ToS/source_home.manifest.json`
4. `ToS/doctrine/KNOWLEDGE_MODEL.md`
5. `ToS/doctrine/NODE_CONTRACT.md`
6. The nearest nested `AGENTS.md` for the touched branch

## Branches

| Branch | Role |
| --- | --- |
| `doctrine/` | ToS knowledge law, route doctrine, templates, and boundary notes |
| `source-witnesses/` | primary source-facing witness material |
| `philosophy/` | growing domain tree of philosophy, from trunk law through era and tradition branches to local graph workbenches |
| `candidate-intake/` | provisional extraction, tabular base, and promotion ledgers |
| `canon/` | canonical authored nodes, relations, and vocabulary registries |
| `public-compatibility/` | public-safe mirrors and compatibility examples |
| `derived-exports/` | generated downstream-facing exports and compact read models |
| `contracts/` | public schema contracts for ToS surfaces |
| `review-ledger/` | dated review notes and inspection records |

## Home Law

- Treat `ToS/` as one tree-shaped home, not as a bucket for old root folders.
- Keep philosophy, witness, intake, canon, compatibility, export, and contract layers
  visible as different branches.
- Keep `ToS/canon/` stronger than compatibility mirrors and derived exports.
- Keep `ToS/source-witnesses/` stronger than candidate intake.
- Keep `ToS/philosophy/` as the domain-shaped growth branch, not as a flat
  import folder and not as a synonym for `candidate-intake/`.
- Keep doctrine in `ToS/doctrine/`; durable rationale remains in
  `docs/decisions/`.
- Do not recreate root-level `sources/`, `intake/`, `tree/`, `examples/`,
  `generated/`, or `schemas/` as active ToS homes.

## Validation

For source-home topology changes, run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_philosophy_topology.py
python scripts/validate_nested_agents.py
```

For route, canon, compatibility, contract, or export changes, continue with
the owning validator named by the nested branch route card.
