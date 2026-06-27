# AGENTS.md

## Applies to

This card applies to `kag/` and every nested path.

## Role

`kag/` is the repo-local KAG provider home for `Tree-of-Sophia`.

It publishes portable, source-linked KAG records derived from ToS-owned export
surfaces. Authored ToS meaning remains in `ToS/`; these records give
`aoa-kag`, `abyss-stack`, and MCP consumers stable handles back to the owning
tree.

## Operating Card

| Field | Route |
| --- | --- |
| input | `ToS/derived-exports/`, derived KAG seam docs, graph projection read models |
| output | local manifest, portable records, source-return projection, validation receipt |
| owner | `kag/AGENTS.md`, `kag/README.md`, `kag/manifest.json` |
| next route | source surface -> derived export validator -> `aoa-kag` registry/composition |
| validation | local KAG provider validator, derived KAG seam validator, and repo release check |

## Source Routes

- `ToS/derived-exports/kag_export.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/README.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`

## Validation

Use the local provider validator first:

```bash
python scripts/validate_local_kag_provider.py
```

Use the source-owned export validator when source exports change:

```bash
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```

Use the release gate when a provider change crosses generated exports,
contracts, or route cards:

```bash
python scripts/release_check.py
```

## Closeout

Report changed KAG records, source-return surfaces, validation run, and any
source export that should be regenerated before consumers read the provider.
