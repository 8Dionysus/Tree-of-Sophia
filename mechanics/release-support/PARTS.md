# Release Support Parts

Each part owns one release-support operation route.

## Part Map

| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Artifact Bundles](parts/artifact-bundles/README.md) | route generated ToS export readmodels through the OS Abyss artifact verifier | source ToS readmodel contract and OS Abyss verifier |
| [Source Release Gate](parts/source-release-gate/README.md) | route release-facing checks through source-home and decision validators | root release docs, validation lanes, and owning source surfaces |

## Provenance Bridge

Use [PROVENANCE](PROVENANCE.md) when release lineage matters. If older material
changes current behavior, update the owning part first.
