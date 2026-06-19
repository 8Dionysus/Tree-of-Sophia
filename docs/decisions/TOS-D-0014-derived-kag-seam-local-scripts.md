# Derived KAG Seam Local Scripts

## Index Metadata

- Decision ID: TOS-D-0014
- Original date: 2026-06-17
- Surface classes: mechanics/boundary-bridge, scripts/topology, docs/validation, ToS/derived-exports
- ToS layers: mechanics, scripts, validation, derived-exports, public-compatibility
- Tree classes: mechanics part, generated export, script inventory, validation lanes
- Guard families: mechanics symmetry, script topology, command authority, owner boundary, generated parity
- Posture: accepted

## Context

The KAG export generator and validator were already owned by
`mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`, but the
scripts still lived in root `scripts/`. Earlier refactors split questbook,
canon, intake, and route-card checks out of the KAG validator, leaving it
focused on the bounded generated export seam.

After `TOS-D-0011` and `TOS-D-0013`, ToS had a script topology map and a
mechanics-local validation route strong enough to move the seam machinery
without turning root `scripts/` into a compatibility warehouse.

## Decision

Move the KAG export generator and validator into
`mechanics/boundary-bridge/parts/derived-kag-seam/scripts/`.

Keep the command lane as `public_entry`, because the checked behavior is still
the current public tiny-entry and KAG export seam. The physical script home is
mechanics-local, but command authority remains in
`docs/validation/validation_lanes.json`.

Keep generated payloads in `ToS/derived-exports/`. Moving the scripts does not
move generated export authority into mechanics and does not make KAG substrate
semantics local ToS authority.

## Options Considered

- Leave the scripts in root `scripts/`. This keeps old commands short, but
  leaves a mechanics-owned bridge machine implemented as root machinery.
- Add root wrappers that call mechanic-local scripts. This preserves command
  compatibility, but keeps the root noise and hides the owner route.
- Move the scripts to the derived KAG seam part and update route docs,
  inventories, and command lanes. This makes the owner route explicit while
  preserving the public-entry validation lane.

## Rationale

Boundary Bridge is the operation that keeps ToS-derived handoffs bounded. The
generator and validator are part of that operation, while the generated JSON
payloads remain derived companions under `ToS/derived-exports/`.

The path is intentionally a little longer because it says what owns the machine:
`mechanics/boundary-bridge/parts/derived-kag-seam/scripts/`. Short root commands
were convenient, but they made the owner harder to see.

## Consequences

Root `scripts/` no longer owns KAG export generation or validation.

Any future widening of the KAG export must update the source-owned route, the
derived KAG seam part, the generated payloads, inventories, and validation lane
together.

The `public_entry` lane still owns release blocking for this seam; mechanics
local discovery may run the validator as a local sanity check, but it does not
replace `public_entry` command authority.

## Source Surfaces

- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`
- `ToS/derived-exports/kag_export.json`
- `ToS/derived-exports/kag_export.min.json`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `docs/validation/SCRIPT_TOPOLOGY.md`

## Validation

Run:

```bash
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
python scripts/validate_tiny_entry_route.py
python scripts/run_mechanics_local_tests.py
python -m unittest tests.test_script_topology tests.test_docs_verify_routes tests.test_validation_lanes
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
