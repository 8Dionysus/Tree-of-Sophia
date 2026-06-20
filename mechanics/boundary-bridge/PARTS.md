# Boundary Bridge Parts

Each part owns one bridge operation route.

## Part Map

| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Derived KAG Seam](parts/derived-kag-seam/README.md) | keep KAG/public handoff bounded and downstream | `ToS/` authored truth and downstream KAG owner |
| [Public Mirror Sync](parts/public-mirror-sync/README.md) | keep public compatibility mirrors aligned with canon without making them canon | `ToS/canon/` and source review |

## Provenance Bridge

Use [PROVENANCE](PROVENANCE.md) for old-path accounting. If older material
changes current behavior, update the owning part first.
