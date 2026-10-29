# AGENTS.md

This card applies to `tests/`.

## Role and routes

`tests/` owns repo-local regressions for routes, generated parity, canon and
mechanics contracts, validators, test topology and release behavior. Tests
prove bounded behavior; eval verdicts, scoring and proof doctrine remain with
`aoa-evals`.

Use [TEST_TOPOLOGY](../docs/testing/TEST_TOPOLOGY.md) and `tests/test_inventory.json`
for owner and home navigation. Executable tests and selected software checks
govern admission under TOS-D-0062; migration-era inventory names do not freeze
future topology.

## Validation

Follow a failure to the owning source, builder or validator, then run the
affected native owner or standalone SDK unittest tests on local fixtures.
Use schema validators and temporary repositories where the contract needs them.
Generated drift returns to its builder before broader tests; mechanics-local
schemas stay in their mechanics lane.

[tests/VALIDATION.md](VALIDATION.md) selects the available test routes. Widen checks for
a changed contract, selected CI/release route or unresolved risk. Release
command order stays in `docs/validation/validation_lanes.json`.
