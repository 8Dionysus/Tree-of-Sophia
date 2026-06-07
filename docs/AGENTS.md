# AGENTS.md

This file applies to repository-level documentation surfaces under `docs/`.

## Read first

Before changing anything here, read:
1. the repository root `AGENTS.md`
2. `README.md`
3. `CHARTER.md` and `BOUNDARIES.md`
4. `ToS/AGENTS.md` when the change routes into the ToS source home
5. the exact decision, release, reference, or index surface you are touching

## Local role

`docs/` holds repository-level documentation that should not live inside the
`ToS/` source home.

These files govern:
- durable decision rationale under `docs/decisions/`
- release guidance under `docs/RELEASING.md`
- preserved root reference material under `docs/AGENTS_ROOT_REFERENCE.md`
- this repository-level route card

ToS knowledge law, node contracts, route doctrine, templates, review ledgers,
source witnesses, domain philosophy topology, canon, contracts, public
compatibility, and derived exports belong under `ToS/`.

## Editing posture

Treat `docs/` as a repository-level rationale and release lane, not as the ToS
source home.

When a repository-level doc describes ToS source-home surfaces, use current
repo-relative paths:

- raw witness in `ToS/source-witnesses/`
- domain philosophy topology in `ToS/philosophy/`
- candidate structure in `ToS/candidate-intake/`
- canonical authored canon in `ToS/canon/`
- compatibility mirrors in `ToS/public-compatibility/`
- derived exports in `ToS/derived-exports/`
- public contracts in `ToS/contracts/`
- review notes in `ToS/review-ledger/`

## Hard no

Do not:
- recreate ToS doctrine here after it has moved into `ToS/doctrine/`
- use a decision note to bypass a stronger current ToS contract
- hand-edit generated decision indexes
- turn repository release docs into source, canon, runtime, or KAG authority

## Validation

For decision metadata changes, run:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

For source-home topology changes, run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_philosophy_topology.py
python scripts/validate_nested_agents.py
```
