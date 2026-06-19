# Public Mirror Sync Bridge Part

## Index Metadata

- Decision ID: TOS-D-0016
- Original date: 2026-06-18
- Surface classes: mechanics/boundary-bridge, scripts/topology, ToS/public-compatibility, docs/validation
- ToS layers: mechanics, scripts, public-compatibility, canon, validation
- Tree classes: mechanics part, public mirror, script inventory, validation lanes
- Guard families: mechanics symmetry, owner boundary, script topology, command authority, public compatibility
- Posture: accepted

## Context

The public example sync scripts mirrored canonical ToS nodes into
`ToS/public-compatibility/`, but the scripts still lived in root `scripts/`.
Their job was not authored philosophy, canon formation, or source witnessing;
it was a repeatable bridge operation from canon into a weaker public
compatibility surface.

Boundary Bridge already owned the derived KAG seam, and its roadmap named public
compatibility mirrors as the next kind of seam that should become a separate
part when repeated enough.

## Decision

Add `mechanics/boundary-bridge/parts/public-mirror-sync/`.

Move the public mirror sync builder, helper, and validator into that part:

- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/sync_tree_examples.py`
- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/tree_example_sync.py`
- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py`

Keep the mirrored payloads in `ToS/public-compatibility/`. The part owns the
operation that keeps mirrors aligned; it does not own the public mirror
payloads as source truth and does not replace canon authority.

Keep the blocking lane as `canon_contracts`, with command authority in
`docs/validation/validation_lanes.json`.

## Options Considered

- Leave the scripts in root `scripts/`. This keeps commands short, but leaves a
  bridge operation in the root command-plane after Boundary Bridge already has a
  local seam pattern.
- Put the scripts under `ToS/public-compatibility/`. This would put machinery
  inside the public mirror home and blur the user's distinction between ToS
  authored/resource homes and mechanics.
- Add a `public-mirror-sync` Boundary Bridge part. This names the operation
  without moving canon or public payload authority into mechanics.

## Rationale

Public compatibility mirrors are weaker than canon, but they are still ToS
resources. The repeated operation is the sync itself: read canon, write or
validate mirrors, then feed downstream public/export seams when needed.

That operation belongs to Boundary Bridge because it controls a crossing from
authored canon into a public compatibility surface. It should sit beside the
derived KAG seam, not inside it, because KAG export and public mirror sync have
different outputs and failure routes.

## Consequences

Root `scripts/` no longer carries public mirror sync machinery.

`mechanics/boundary-bridge` now has two active parts:
`derived-kag-seam` and `public-mirror-sync`.

Future mirror expansion must update the stronger canon/source route, the public
compatibility payloads, the public mirror sync part, inventories, and validation
lanes together.

## Source Surfaces

- `mechanics/boundary-bridge/parts/public-mirror-sync/README.md`
- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/sync_tree_examples.py`
- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/tree_example_sync.py`
- `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py`
- `ToS/canon/`
- `ToS/public-compatibility/`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `docs/validation/SCRIPT_TOPOLOGY.md`

## Validation

Run:

```bash
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python scripts/validate_mechanics_topology.py
python scripts/run_mechanics_local_tests.py
python -m unittest tests.test_script_topology tests.test_docs_verify_routes tests.test_validation_lanes
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
