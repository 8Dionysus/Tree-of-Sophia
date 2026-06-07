# ToS Source-Home Topology

## Index Metadata

- Decision ID: TOS-D-0002
- Original date: 2026-06-06
- Surface classes: source-home, docs/route-law, scripts/validation
- ToS layers: doctrine, source-witnesses, candidate-intake, canon, examples, generated, schemas, review-ledger, scripts
- Tree classes: source, concept, principle, lineage, event, state, support, analogy, synthesis, relation
- Guard families: source-home symmetry, source-first authority, generated parity, compatibility boundary, owner boundary
- Posture: accepted

## Context

`Tree-of-Sophia` needed symmetry with the already refactored AoA repositories:
a real home folder, manifest-backed topology, nested route cards, and validators
that can tell when source-home shape drifts.

The old topology had ToS-owned active homes spread across root-level
`sources/`, `intake/`, `tree/`, `examples/`, `generated/`, `schemas/`, and
doctrine docs under `docs/`. That shape preserved layer boundaries, but it made
`tree/` look like the real source home while `ToS/` did not exist.

The refactor pressure is not to erase the old distinctions. The pressure is to
move those distinctions into one tree-shaped ToS organ.

## Decision

Adopt `ToS/` as the source-home organ for Tree of Sophia.

The active ToS-owned surfaces now live under:

- `ToS/doctrine/`
- `ToS/source-witnesses/`
- `ToS/candidate-intake/`
- `ToS/canon/`
- `ToS/public-compatibility/`
- `ToS/derived-exports/`
- `ToS/contracts/`
- `ToS/review-ledger/`

Root-level `docs/` remains a repository-level lane for decisions, release
guidance, and preserved root reference material. It is not the ToS source home.

## Options Considered

- Keep `tree/` at the root and create a small decorative `ToS/` folder. This
  would preserve validator paths cheaply, but it would create two heads: `ToS/`
  as nominal home and root `tree/` as actual canon.
- Move only `tree/` under `ToS/` and leave the rest of the ToS-owned layers at
  root. This would reduce the immediate diff, but it would keep source witness,
  intake, compatibility, exports, and contracts outside the home organ.
- Move all active ToS-owned layers into `ToS/` with speaking branch names and a
  source-home manifest. This creates a larger first pass, but gives the repo a
  real home topology that can be validated.

## Rationale

The selected route keeps symmetry with sibling repositories without forcing a
foreign ontology onto ToS. `ToS/` is not an SDK, playbook, agent, skill, or
technique home. It is a philosophical source home whose branches preserve the
source-first ladder:

source witness -> candidate intake -> authored canon -> public compatibility
-> derived export.

The branch names are intentionally more explicit than the old root folders.
`ToS/canon/` replaces root `tree/` so the canonical authored layer stays inside
the ToS home. `ToS/source-witnesses/` names source-facing witness material
without pretending every witness is already authored node law. `ToS/derived-exports/`
keeps generated read models visibly downstream.

The manifest and validator make this topology checkable. They do not make the
first contour final ontology; they make the current contour honest enough for
the next refactor pass.

## Consequences

Future active ToS-owned source surfaces should route into `ToS/` unless a
stronger repository-level lane owns them.

Do not recreate root-level `sources/`, `intake/`, `tree/`, `examples/`,
`generated/`, or `schemas/` as active ToS homes. If a compatibility alias is
needed later, it should be explicit, temporary, and lower authority than the
`ToS/` home.

Validators and generated exports now use `ToS/` paths. Downstream consumers
must treat those as the current repo-relative ToS paths.

## Source Surfaces

- `ToS/AGENTS.md`
- `ToS/README.md`
- `ToS/source_home.manifest.json`
- `ToS/doctrine/KNOWLEDGE_MODEL.md`
- `ToS/doctrine/NODE_CONTRACT.md`
- `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
- `ToS/canon/`
- `ToS/public-compatibility/`
- `ToS/derived-exports/`
- `scripts/validate_tos_source_home.py`
- `scripts/release_check.py`

## Validation

Run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
python scripts/build_root_entry_map.py --check
python scripts/validate_root_entry_map.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_intake_pack.py
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_tree_example_sync.py
python scripts/validate_kag_export.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python -m unittest discover -s tests
```
