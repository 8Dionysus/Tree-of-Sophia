# Release Support Mechanic

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | release-facing source-home, decision, generated, or validator surface changes |
| input | release check, decision index, source-home validator |
| output | checked release route or failing owner surface |
| owner | `mechanics/release-support/` |
| stronger route | root release docs and scripts own release commands |
| next route | [Artifact Bundles](parts/artifact-bundles/README.md) or [Source Release Gate](parts/source-release-gate/README.md) |
| validation | `python scripts/release_check.py` |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
