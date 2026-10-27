# TOS-D-0073 Verified PR Source CI and Exact-SHA Releases

## Index Metadata

- Decision ID: TOS-D-0073
- Original date: 2026-10-26
- Surface classes: github/checks, scripts/validation, docs/route-law
- ToS layers: access, docs
- Tree classes: none
- Guard families: artifact integrity, scoped validation, compatibility
- Posture: accepted; implementation and platform evidence tracked by the landing PR

## Context

The operator authorized an ordered CI optimization campaign, including a
separate PR/main/release boundary while preserving necessary checks. The
existing workflow repeats the same selected software tests and packaging on
each PR and its subsequent main push. Full baseline runs take about 48–51
minutes each. [D0062](TOS-D-0062-independent-software-corpus-and-integration-releases.md)
already separates software qualification from corpus and downstream releases;
it does not require duplicate validation of identical source trees.

GitHub tests a synthetic PR merge commit. Its SHA differs from the PR head and
the eventual squash commit. A green head status alone therefore cannot prove
which tree was tested. Equal tracked trees also do not establish equal commit
metadata, history, environment or package provenance.

## Decision

PRs retain the normal changed-path checks and required gate. On a main push,
reuse their source-CI result only after an explicit GitHub metadata adapter
verifies one merged PR whose merge commit is the landed SHA, one unambiguous
completed successful Repo Validation run at its latest attempt, and that run's
successful required gate. Distinct matching runs or a truncated run list keep
normal selection; numeric run IDs do not order subsequent reruns. Bind the
gate's pinned checkout log to the actual synthetic commit,
verify its PR base/head parents, and require its tree to equal the landed tree.
The native selector independently checks the supplied tree against its own
checkout. Changed-document and selected documentation-owner checks still run.

Missing, ambiguous, expired, failed or inaccessible evidence selects the normal
main route. A direct push or bypass without the complete proof gets that same
route. Explicit full runs cannot be reduced by prior-tree evidence. This
extends D0062's scoped validation boundary; it does not alter its corpus or
external-consumer authority.

A reused source-CI result does not create a package for the landed SHA. A
software release must run the complete qualification workflow for its exact
selected main SHA and verify the resulting receipts and artifact identity.
PR artifacts remain candidates for their actual tested commit. Publication,
deployment and consumer admission remain separate operations.

## Options Considered

- Retain duplicate full selected PR/main runs and their recurring cost.
- Skip main work based only on a green PR head or merged-PR association.
- Reuse only an observed successful checkout of the exact landed tree, with
  normal selection as the fallback and fresh exact-SHA release qualification.

The implemented choice is the third. The second loses the tested-tree and
failure boundaries; the first supplies no new source-content coverage when
the complete proof succeeds.

## Rationale

Source validation and package provenance answer different questions. The
exact-tree proof preserves the former while a fresh release preserves the
latter. API access is read-only and optional: its failure increases work
instead of accepting an unverified result. Existing native selection, document
checks, artifact authentication and selected-job failure rules remain owned
by their current implementation.

## Consequences

- Routine merges can avoid repeating a successful source-CI run.
- Main still prepares current native executors, records the proof, checks
  changed documents and aggregates explicitly selected results.
- Expired logs or API permission failures can make main slower without
  weakening its required checks.
- Main reuse makes no claim about branch-protection enforcement, approval,
  commit-history-sensitive behavior, exact-SHA artifacts or runtime health.
- Release qualification remains explicit and can still fail independently.

## Source Surfaces

- `.github/workflows/repo-validation.yml`, `.github/scripts/verified-pr-tree.mjs`
- `rust/crates/tos-ops-mechanics-plan/src/software_ci.rs`
- `rust/crates/tos-ops-mechanics-plan/src/software_ci_main.rs`
- `.github/AGENTS.md`, `docs/RELEASING.md`

## Validation

Use the owner decision-index builder, parity check and record validator in
`docs/VALIDATION.md`. Exercise accepted and rejected API proof fixtures, actual
Git tree matching, explicit full selection and changed-document refusal.
Verify PR CI and a subsequent main push on their exact commits; keep release
artifact evidence distinct. Local tests and this record alone do not establish
the platform result or release qualification.
