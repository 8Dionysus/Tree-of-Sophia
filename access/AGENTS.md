# AGENTS.md

This card applies to the installable access product under `access/`.

## Role

`access/` owns portable, read-only delivery of Tree of Sophia projections:
the native Rust query core, installed CLI, local
HTTP/MCP service, web application, WebMCP action contract, standalone
profiles, and native software/data builders.

`ToS/` owns source meaning, review, canon and derived exports.

## Boundary

- `rust/crates/tos-query/` owns query planning, normalization, presentation,
  search and exploration semantics. `rust/crates/tos-access/` owns source-read,
  managed-release, transport and installed-product integration. Query, HTTP
  and source-reading engines reside in native crates. Installed CLI, HTTP, MCP
  and browser actions route to the native owner.
- `contracts/` and `profiles/` own the portable consumer-facing ABI.
- `packaging/` contains explicit data-generation/validation tools only; the
  native software archive owner assembles software subjects.
- ToS operates and releases its own data without ecosystem source checkouts,
  services or interpreters. Integrators adapt to ToS from their own repositories.
- Runtime packets must preserve `source_ref` and must not claim review,
  rights, canon, or semantic authority.
- Software build/test/package uses versioned contracts and representative
  fixtures without production corpus, payload custody or sibling repositories.
- Explicit data selection (`--root` or `TOS_DATA_ROOT`) chooses data, never
  installed code or API schemas. The public site is built and published separately. Missing data does not discover
  another checkout through the current directory.
- Data release owns full snapshot validation and compilation. API-schema
  checking uses schemas without traversing production graph rows.

## Validation route

Select the `standalone_access` route in the repository root
[`VALIDATION.md`](../VALIDATION.md) after the changed contract or adapter is
known. The lane manifest owns command order; the access README remains the
human orientation surface for archive-boundary and installation details.
