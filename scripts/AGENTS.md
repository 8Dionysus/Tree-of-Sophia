# AGENTS.md

This card applies to deterministic generators, validators and small command
helpers under `scripts/`.

## Role

Scripts carry a bounded operation for a named source owner. Builders produce
owned companions; validators check declared mechanics. They do not acquire
source, semantic, rights, canon, runtime or downstream acceptance authority.

## Read before editing

Read the exact source/contract and its nearest card. Internal command order
lives in `docs/validation/validation_lanes.json`. The descriptive
`docs/validation/script_inventory.json` and
`docs/validation/SCRIPT_TOPOLOGY.md` locate owners and side effects.
For route-card work also read `DESIGN.AGENTS.md` and
`docs/validation/agents_route_inventory.json`; decision indexes follow
`docs/decisions/AGENTS.md`.

## Boundary Routes

- Change the owned source or builder input before regenerating a companion.
- Preserve explicit inputs, stable ordering, finite resource limits and visible
  failure. A validator must not become a hidden source of policy.
- Keep private or payload-bearing data in its selected owner-controlled storage.
  Public outputs retain their source bindings and permitted visibility.
- Source semantics return to the exact `ToS/` branch; repeatable operation law
  returns to `mechanics/`; external integrations remain consumer-owned.
- `rust/crates/tos-ops-mechanics-plan/src/release_check_main.rs` is the native
  release executor. Its manifest sequence, not an inventory entry, determines
  which commands run. Update navigation when the actual route changes.

## Validation

Use [scripts/VALIDATION.md](VALIDATION.md) for the selected owner lane. Route
changes need currentness, inheritance and task-harness checks; builders need
parity and the affected consumer check. Run the broader release route only when
selected by the changed surface or publication operation.
