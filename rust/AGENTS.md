# AGENTS.md

This card applies to the native workspace in `rust/`.

## Role and routes

Start with the affected crate. Read the [workspace guide](README.md) when feature
or build boundaries matter. `tos-foundation` owns shared types, `tos-source-store` custody,
`tos-command` mutations, `tos-compiler` projections, `tos-query` reading rules,
`tos-access` adapters, `tos-validation` checks and `tos-ops-mechanics-plan`
operation and release tooling. The root Cargo manifest owns workspace membership.

Use [ToS contracts](../ToS/contracts/AGENTS.md) or [mechanics](../mechanics/AGENTS.md)
for source or operation meaning. When access behavior changes, use [its contracts](../access/contracts/README.md).
Preserve native/WASM feature boundaries and standalone operation.

## Validation

Run affected crate tests on bounded fixtures. Select broader checks through
[VALIDATION](../VALIDATION.md) and [RELEASING](../docs/RELEASING.md) when the
changed contract or CI route requires them. `validation_lanes.json` owns command
order; generated outputs follow their source-owned inputs. Production corpus,
runtime qualification and source admission remain separately selected operations.
