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

The portable family is temporarily frozen at the identity recorded in
`kag/indexes/hot_profile.json`. The local provider validator must reject any
change to the frozen manifest binding, shard bytes, digests, or record counts
until an explicit ToS operator command changes that state. Source-currentness
drift against evolving ToS files is expected while regeneration is paused and
is not part of the frozen-family integrity check. This freeze does not mutate
or activate any downstream AbyssOS owner.

## Source Routes

- `ToS/derived-exports/kag_export.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/README.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`

## Validation

Select the frozen local KAG binding, `public_entry`, or release route in
[`kag/VALIDATION.md`](VALIDATION.md) after the source export and intended claim
are known. While the freeze is active, `local_kag_provider` remains a blocking
integrity guard and runs
`scripts/validate_local_kag_provider.py --freeze-only`; the full provider
validator and KAG export regeneration resume only after the explicit ToS
operator unfreeze command. KAG export regeneration remains with
`mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md` and its
source builder; local provider procedure stays in the district validation
route.

## Closeout

Report changed KAG records, source-return surfaces, validation run, and any
source export that should be regenerated before consumers read the provider.
