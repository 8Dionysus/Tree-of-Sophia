# Artifact Bundles

## Operating Card

| Field | Route |
| --- | --- |
| role | describe and verify OS Abyss artifact bundles for generated ToS exports |
| input | generated downstream read models under `ToS/derived-exports/` |
| output | abyss-machine verified artifact bundle sidecars in temporary/staging space |
| owner | `mechanics/release-support/parts/artifact-bundles/` |
| stronger route | `abyss-machine` owns artifact/signature policy and verifier logic |
| next route | `ToS/derived-exports/AGENTS.md`, `docs/validation/validation_lanes.json`, or failing generated export |
| tools | `scripts/validate_abyss_machine_generated_readmodel_bundle.py` |
| check | `python mechanics/release-support/parts/artifact-bundles/scripts/validate_abyss_machine_generated_readmodel_bundle.py` |

## Boundary

This part does not create ToS meaning and does not define signing doctrine.
It keeps the current generated JSON readmodels consumable through the OS Abyss
artifact bundle verifier while preserving ToS source-first authority.

Current controls are ABI-only for JSON readmodels. C2PA belongs to public
PDF/media/visual exports when such an export exists; SLSA/in-toto and
Sigstore/Cosign trigger only when a generated export becomes a published
release/export bundle.

The validator also writes a temporary OS Abyss bundle registry read-model,
requires a `release-ready` latest record only after successful ABI verification,
and rehearses rejection of corrupted ABI sidecars, private markers, unverified
latest promotion, terminal revocation, consumer trust-gate selection, and
isolated subject-store materialization.
