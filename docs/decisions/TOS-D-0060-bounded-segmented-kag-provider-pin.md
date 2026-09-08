# TOS-D-0060 Bounded Segmented KAG Provider Pin

## Index Metadata

- Decision ID: TOS-D-0060
- Original date: 2026-09-08
- Surface classes: kag/provider, validation, release/compatibility, GitHub workflow
- ToS layers: derived-export, generated carrier, docs/route-law, owner-handoff
- Tree classes: provider edge, bounded generated family, rollback carrier
- Guard families: exact identity, source currentness, bounded reads, fail-closed rollback
- Posture: accepted

## Context

The current Tree-of-Sophia source tree is larger than the materialized v3/v4
KAG family limits. A full compatibility corpus would exceed the local carrier
budget and would make every downstream read depend on an unbounded assembly.
The `aoa-kag` owner now provides a v5 segmented family whose control manifest
and individual records stay bounded while preserving source-linked canonical
identity.

## Decision

Select `aoa-repo-local-kag-segmented-family-v1` through the exact clean
`aoa-kag` revision `b95446483f25e1f59c732f5c095d4f641a1c9431`, recorded in
`kag/provider_pin.json`. The local adapter validates the external owner
implementation by immutable revision, verifies the manifest and every segment,
and performs one bounded segment read. Complete compatibility assembly remains
an explicit opt-in operation.

Retain the prior v3/v4 family routes as explicit rollback carriers selected by
family digest. A v5 validation failure preserves the last-good manifest and
fails closed; no reader may silently fall back to an older schema or silently
materialize the full corpus.

## Rationale

The segmented family keeps the generated carrier below the tracked control
budget without weakening source ownership or pretending that a projection is
authored meaning. Pinning the provider separates ToS consumer admission from
the external schema and reader implementation. The bounded probe proves only
the declared local adapter route, not runtime deployment, semantic acceptance,
artifact admission, or live consumer health.

## Consequences

- Source changes require regeneration through the pinned owner revision and a
  new digest-bound budget receipt.
- CI must pin the same immutable action/provider revision and fetch it for the
  release audit's local adapter check.
- v3/v4 validators remain useful for rollback evidence but cannot admit a v5
  manifest.
- Segment integrity and bounded-reader validation do not prove full assembly,
  downstream registry freshness, runtime activation, or human acceptance.

## Source Surfaces

- `kag/provider_pin.json`
- `kag/indexes/index_family.manifest.json`
- `scripts/validate_local_segmented_kag_provider.py`
- `scripts/validate_local_kag_provider.py`
- `.agents/agent-surface.manifest.json`
- `.github/workflows/repo-validation.yml`
- `docs/RELEASING.md`

## Validation

Run the exact pinned provider adapter, the local KAG provider lane, the
segmented owner-family gate, agent-surface and decision-index parity, then the
release checks. Hosted CI, merge, deployment, runtime health, artifact trust,
and semantic or human acceptance remain separate claims.
