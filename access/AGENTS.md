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

## Verify

Run the focused lane from the repository root:

```bash
python -m unittest discover -s access/tests
python access/packaging/validate_standalone.py
npm --prefix access/web run typecheck
npm --prefix access/web run build
```

For an archive boundary check, build a candidate and validate the extracted
result outside the repository. A green bundle check proves portability and
integrity mechanics only; it does not authorize publication or OS Abyss
artifact admission.
