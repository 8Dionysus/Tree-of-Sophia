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
| validation | local provider and export checks for the selected KAG artifact; independent of software merge |

TOS-D-0062 supersedes the universal source-currentness merge obligation in
TOS-D-0044. Build KAG from an explicitly selected immutable ToS source/data
revision, record that revision and the provider revision, and validate source
refs, hashes, shards and parity before publishing that KAG artifact. A stale
integration remains visibly stale; it does not become current because software
CI passed. No regeneration is required for an unrelated software PR.

## Source Routes

- `ToS/derived-exports/kag_export.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/README.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`

## Validation

Use [`kag/VALIDATION.md`](VALIDATION.md) for the selected integration.
`local_kag_provider` blocks publication of an invalid or falsely current KAG
artifact, not standalone software merge or release. Source exports remain
owned by their source builders and review routes.

## Closeout

Report changed KAG records, source-return surfaces, validation run, and any
source export that should be regenerated before consumers read the provider.
