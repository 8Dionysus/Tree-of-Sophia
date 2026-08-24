# Same-Day Campaign Release Consolidation

## Index Metadata

- Decision ID: TOS-D-0027
- Original date: 2026-08-23
- Surface classes: docs/route-law, release, provenance
- ToS layers: docs, derived-exports
- Tree classes: none
- Guard families: source-first authority, identity separation, provenance, release integrity, preservation boundary
- Posture: accepted

## Context

The 2026-08-23 Tree-of-Sophia campaign produced four same-day published
Release/tag pairs (`v0.5.0` through `v0.5.3`) while the source and provider
surface continued to move. The immutable pre-cleanup release-truth corpus and
its line-level reconciliation ledger distinguish this release-only churn from
the pre-campaign release history, first-parent commits, pull requests, source
contracts, generated receipts, validation evidence, limitations, and
non-claims that must remain available.

`TOS-D-0026` correctly requires exact provider identities and a successor when
a provider edge changes, but its general predecessor-preservation wording does
not by itself distinguish duplicate same-day campaign publication churn from a
distinct historical release. Without a narrower decision, the cleanup route
would either leave duplicate campaign Releases in place or appear to contradict
the exact one-release mandate.

## Decision

For the bounded `release-cleanup-tree-consolidate-v050` campaign, the four
same-day Release/tag pairs listed in the immutable pre-cleanup corpus are
treated as superseded publication churn, not as four independent durable
version histories. Only after the owner source PR, required CI, GitHub merge,
landed-main gates, and final body/ledger reconciliation pass, the cleanup may
delete exactly those four same-day GitHub Releases and tag refs. It then creates
exactly one annotated `v0.5.0` tag on the exact landed `main` commit and exactly
one GitHub Release whose body is derived from the canonical dated `v0.5.0`
changelog section.

This decision supersedes `TOS-D-0026` only for the treatment of these four
same-day duplicate campaign publications. It does not weaken exact provider
tag-plus-commit binding, the separation of provider release and workflow action
identity, or the general successor rule for a genuinely distinct provider or
source release. All pre-campaign refs, source/PR/commit material, release
bodies, tag-scoped changelogs, reports, receipts, artifact records, and
explicit claim limits remain preserved as historical evidence; no deletion is
allowed to remove their content or to promote an artifact/runtime/proof/
delivery/closure/acceptance verdict.

## Options Considered

- Keep all four same-day Releases and tags. This preserves publication clutter
  and fails the explicit one-canonical-release target.
- Retag or force-move an existing `v0.5.0`. This would destroy tag-object
  provenance and make the old publication indistinguishable from the final
  landed source.
- Delete all historical releases or refs before recreating `v0.5.0`. This
  would violate pre-campaign preservation and lose independent evidence.
- Delete only the four immutable-corpus-listed same-day campaign pairs after
  landed gates, then create one exact landed `v0.5.0` publication while keeping
  the full historical corpus. This is the chosen route.

## Rationale

The canonical object is the final source and its exact landed commit, not a
moving tag or the latest publication wrapper. Narrowly classifying same-day
duplicates allows the public release surface to become legible without
rewriting the source history or collapsing source, CI, artifact, runtime,
proof, delivery, closure, and acceptance claims. The immutable corpus and
content-conservation ledger make the cleanup auditable and reversible at the
evidence level even though the listed GitHub publication objects are removed.

## Consequences

The release route must prove the exact deletion set, tag object/type/peel,
body equality, latest marker, assets, local/remote sync, and unchanged
pre-campaign refs after publication. A release body may retain historical
version strings only when they are explicitly labeled historical evidence
inside the single active `v0.5.0` section. A clean source release remains a
source/CI/publication claim; it does not imply artifact admission, runtime
health, semantic proof, transport delivery, closure, owner acceptance, or
human acceptance.

## Source Surfaces

- `CHANGELOG.md`
- `README.md`
- `ROADMAP.md`
- `docs/RELEASING.md`
- `.github/workflows/repo-validation.yml`
- `kag/`
- `tests/test_roadmap_parity.py`
- `TOS-D-0026-exact-current-federated-provider-identities.md`

## Validation

Validate decision indexes and records with `python scripts/generate_decision_indexes.py`,
`python scripts/generate_decision_indexes.py --check`, and
`python scripts/validate_decision_records.py`.

The release execution must additionally bind the immutable pre-cleanup corpus,
the content-conservation ledger, the exact landed source, provider evidence,
GitHub deletion/publication receipts, and postpublish preservation checks.
