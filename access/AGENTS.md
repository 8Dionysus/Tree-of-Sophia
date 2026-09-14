# AGENTS.md

This card applies to the installable access product under `access/`.

## Role

`access/` owns portable, read-only delivery of Tree of Sophia projections:
the shared query core, CLI, local HTTP service, native MCP adapter, web
application, WebMCP action contract, standalone profiles, and bundle builder.

It does not author philosophical meaning. `ToS/` source, review, canon, and
derived-export owners remain stronger than every packet emitted here.

## Boundary

- `src/tos_access/core.py` is the reusable read/query center.
- CLI, HTTP, native MCP, browser actions, and optional AbyssOS integrations
  are adapters over that center.
- `contracts/` and `profiles/` own the portable consumer-facing ABI.
- `packaging/` may assemble only the subjects allowlisted by
  `contracts/runtime-data.v1.json`.
- `integrations/abyssos/` may adapt to ecosystem services but must remain
  optional; its absence is a capability state, not an installation failure.
- Runtime packets must preserve `source_ref` and must not claim review,
  rights, canon, or semantic authority.
- Software build/test/package uses versioned contracts and representative
  fixtures without production corpus, payload custody or sibling repositories.
- Explicit data selection (`--root` or `TOS_DATA_ROOT`) chooses data, never
  installed code, browser assets or API schemas. Missing data does not discover
  another checkout through the current directory.
- Full snapshot validation and compilation belong to data release. API-schema
  checking must not traverse production graph rows as a hidden side effect.

## Validation route

Select the `standalone_access` route in the repository root
[`VALIDATION.md`](../VALIDATION.md) after the changed contract or adapter is
known. The lane manifest owns command order; the access README remains the
human orientation surface for archive-boundary and installation details.
