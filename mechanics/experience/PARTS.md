# Experience Parts

Each part owns one boundary operation family.

## Part Map

| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Candidate Review](parts/candidate-review/README.md) | candidate dossiers and intake decisions | `ToS/` review and source authority |
| [Adoption Boundary](parts/adoption-boundary/README.md) | adoption dossiers and no-runtime adoption guard | runtime and owner repos |
| [Governance Boundary](parts/governance-boundary/README.md) | governance dossiers, precedents, and review notes | governance owner surfaces |
| [Installation Boundary](parts/installation-boundary/README.md) | installation dossiers and install-write guard | installation and runtime owners |
| [Service Office Boundary](parts/service-office-boundary/README.md) | service dossiers and office runtime guard | service and office owners |
| [Pattern Review](parts/pattern-review/README.md) | pattern review notes | ToS source review and stronger owner handoff |
| [Write Guards](parts/write-guards/README.md) | generic no-direct-write guard surfaces | source, runtime, and center owners stay stronger |

## Provenance Bridge

Use [PROVENANCE](PROVENANCE.md) for old-path accounting. If older material
changes current behavior, update the owning part first.
