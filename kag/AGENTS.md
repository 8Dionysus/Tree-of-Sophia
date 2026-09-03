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

Select the local KAG, `public_entry`, or release route in
[`kag/VALIDATION.md`](VALIDATION.md) after the source export and intended claim
are known. KAG export regeneration remains with
`mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md` and its
source builder; local provider procedure stays in the district validation
route.

## Closeout

Report changed KAG records, source-return surfaces, validation run, and any
source export that should be regenerated before consumers read the provider.
