# TOS-D-0044 End the Temporary KAG Freeze

## Index Metadata

- Decision ID: TOS-D-0044
- Original date: 2026-09-04
- Surface classes: kag/provider, docs/route-law, scripts/validation, github/checks
- ToS layers: derived exports, local KAG provider
- Tree classes: none
- Guard families: source-first authority, currentness, generated parity
- Posture: superseded by TOS-D-0072

## Current status: superseded KAG integration

The KAG provider/action pin, external checkout, builder, validator and CI-fetch
prescriptions below are historical and superseded by
[TOS-D-0072](TOS-D-0072-repository-independent-execution-and-exports.md).
ToS builds and verifies its own source-return export without `aoa-kag`.
Consumer indexing, provider configuration and admission run on the consumer's
side; they are not ToS commands, completion criteria or release gates.
Historical identities and evidence below remain unchanged; paths to retired
provider files describe that historical state, not files to recreate.

## Context

The portable ToS KAG family was temporarily frozen while authored sources
continued to evolve. Removing obsolete Spark and mechanics archive trees made
the difference explicit: the frozen snapshot still described retired paths.
The operator now explicitly authorizes complete unfreezing, not a one-time
refresh followed by another freeze.

## Decision

End the temporary KAG source-family freeze. Remove its exact-snapshot control
file and freeze-only validation bypasses. Restore full provider currentness,
source ownership and integrity validation, canonical regeneration, and the
required owner-family CI check. No automatic refreeze is introduced.

Use the verified immutable `aoa-kag` source/action commit
`14ee1e33e43749d23c557b3ef526eca7edb36196` for generation and CI. Historical
published provider identities remain unchanged. Exact PR-base lineage and
event-history boundaries remain explicit; generated-delta admission follows
the canonical budget-receipt contract instead of silently bypassing budgets.

## Options Considered

- Keep the frozen historical family and its expected source drift.
- Refresh once and freeze a replacement snapshot.
- Resume normal source-current generation and full validation permanently.

The operator selected the third option.

## Rationale

A derived navigation family should follow the active source tree once the
temporary pause has ended. Integrity of an old snapshot is not currentness;
the full validator must again detect deleted paths and changed source hashes.
Historical snapshots remain recoverable in Git without active freeze rules.

## Consequences

- Current source changes require matching generated KAG companions and checks.
- The previous freeze manifest remains in Git at
  `a6f4467601937cf1378270d3cd06d0912cdca557:kag/indexes/hot_profile.json`.
- Authored text, review, rights, translation, semantics and canon remain with
  their ToS owners. Regeneration cannot confer any of those authorities.
- This source-family unfreeze does not deploy services, activate the optional
  AbyssOS runtime adapter, or admit artifacts into a live consumer.

## Source Surfaces

- `kag/AGENTS.md` and `kag/VALIDATION.md`
- `rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs`
- `scripts/validate_agent_surface.py`
- `scripts/validate_documentation_cross_corpus.py`
- `docs/validation/validation_lanes.json`
- `.github/workflows/repo-validation.yml`
- `.github/workflows/source-side-smoke.yml`

## Validation

Regenerate decision indexes and source currentness through their owner builders.
Run focused negative-currentness tests, full local provider validation, the
pinned canonical owner-family check and the release lane. Local results,
remote CI, merge, runtime and consumer acceptance remain separate claims.
