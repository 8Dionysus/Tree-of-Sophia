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
| tools | `tos-ops-mechanics-plan --artifact-bundle` |
| check | `tos-ops-mechanics-plan --artifact-bundle --repo-root ROOT` |

## Boundary

This part does not create ToS meaning and does not define signing doctrine.
It keeps the current generated JSON readmodels consumable through the OS Abyss
artifact bundle verifier while preserving ToS source-first authority.

Current controls are ABI-only for JSON readmodels. C2PA belongs to public
PDF/media/visual exports when such an export exists; SLSA/in-toto and
Sigstore/Cosign trigger only when a generated export becomes a published
release/export bundle.

The validator also promotes durable release-ready evidence with source and
host-managed trust-root metadata, materializes an artifact subject store,
requires an explicit agent-intent trust-gate allow decision before consumption,
and rehearses rejection of corrupted ABI sidecars, private markers, unverified
latest promotion, terminal revocation, consumer trust-gate selection, and
isolated subject-store materialization.

Generated bundle directories, registry records, subject stores, and sidecars
are generated evidence under ignored `dist/` paths. They are not authored ToS
meaning and are not checked into the repository.

## Partitioned projection closure

The static `generated_readmodel.bundle.json` manifest declares exact
`artifact_subjects.path` entries. The OS Abyss resolver does not recursively
follow a `tos_partitioned_projection_v1` root into its digest-named parts. The
ToS validator therefore resolves each declared partition root through the
shared native partition reader and rejects the bundle before sidecar or trust work
when any manifest or part is absent from the exact subject list; a glob does
not satisfy this requirement.

Until this owner surface has a reviewed generated bundle manifest, or an
equivalent consumer contract that carries the per-snapshot closure, the
static ABI bundle cannot admit the new partitioned corpus. Return that
admission here after closure support exists; a successful source or access
build does not widen the current artifact subject set.

The native entry requires an explicit source root. `--abyss-machine` selects the
external owner CLI; `TOS_ABYSS_MACHINE_EXECUTABLE` or `PATH` is used when omitted.
It requires the owner's `artifact_subject_store.search_scope` capability and
binds each child to `ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ISOLATED_ROOT`, including
fresh negative and nested materialization rehearsals. Ambient stores cannot
satisfy those checks. `--abyss-machine-root` supplies an optional source root
for portable provenance redaction; it never selects or imports owner code.

The existing runtime integration pause is checked before owner execution.
Changing implementation does not enable AbyssOS admission. Outside the pause,
all declared partition manifests and parts must be exact subjects. The entry
checks public content and preserves subject hashes across owner operations,
then verifies consumer admission again after portable metadata sanitization.
`--no-clean` preserves a previous generated output for an idempotent rerun;
cleaning outside the default generated directories requires the validator
marker and cannot overlap source subjects or another selected output.
