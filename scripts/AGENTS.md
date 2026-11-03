# AGENTS.md

This card applies to deterministic generators, validators and small command
helpers under `scripts/`.

## Role

Scripts carry bounded operations for named source owners. Builders produce
companions; validators check declared mechanics.

## Read before editing

Read the exact source/contract and its nearest card. Internal command order
lives in `docs/validation/validation_lanes.json`. Decision records follow `docs/decisions/AGENTS.md`.

## Boundary Routes

- Change the owned source or builder input before regenerating a companion.
- Preserve explicit inputs, stable ordering, finite resource limits and visible
  failure. A validator must not become a hidden source of policy.
- Keep private or payload-bearing data in its selected owner-controlled storage.
  Public outputs retain their source bindings and permitted visibility.
- `rust/crates/tos-ops-mechanics-plan/src/release_check_main.rs` is the native
  release executor; its manifest sequence determines command order. Update
  navigation when the route changes.

## Validation

Use [scripts/VALIDATION.md](VALIDATION.md) for the selected owner lane. Route
changes need currentness, inheritance and task-harness checks; builders need
parity and the affected consumer check. Run the broader release route only when
selected by the changed surface or publication operation.
