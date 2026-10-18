# Release Support Parts

Each part owns one release-support operation route.

## Part Map

| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Source Release Gate](parts/source-release-gate/README.md) | route release-facing checks through source-home and decision validators | root release docs, validation lanes, and owning source surfaces |

## Provenance Bridge

Use [PROVENANCE](PROVENANCE.md) when release lineage matters. If older material
changes current behavior, update the owning part first.
