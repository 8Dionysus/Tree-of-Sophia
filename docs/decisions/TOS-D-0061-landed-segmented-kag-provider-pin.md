# TOS-D-0061 Landed Segmented KAG Provider Pin

## Index Metadata

- Decision ID: TOS-D-0061
- Original date: 2026-09-09
- Surface classes: kag/provider, validation, release/compatibility, GitHub workflow
- ToS layers: derived-export, generated carrier, docs/route-law, owner-handoff
- Tree classes: provider edge, bounded generated family, rollback carrier
- Guard families: exact identity, source currentness, bounded reads, fail-closed rollback
- Posture: accepted

## Supersedes

This record supersedes [TOS-D-0060](TOS-D-0060-bounded-segmented-kag-provider-pin.md)
only for the current external provider revision. Its segmented-family boundary,
bounded-reader posture, and explicit rollback law remain in force.

## Context

The segmented provider selected by TOS-D-0060 was validated from a feature
revision before it landed on the public `aoa-kag` default branch. The provider
owner has now landed the same validated provider family and coverage projection
through PR #234, producing the immutable public revision
`1a0342087b18d0a1f5630036937a548b6526e0e9`.

## Decision

Select `aoa-repo-local-kag-segmented-family-v1` through the exact clean
`aoa-kag` revision `1a0342087b18d0a1f5630036937a548b6526e0e9`, recorded in
`kag/provider_pin.json` and used by both hosted workflow edges. The local
adapter continues to validate the external owner implementation by immutable
revision, verifies the manifest and every segment, and performs one bounded
segment read. Complete compatibility assembly remains an explicit opt-in
operation.

Retain the prior v3/v4 family routes as explicit rollback carriers selected by
family digest. A v5 validation failure preserves the last-good manifest and
fails closed; no reader may silently fall back to an older schema or silently
materialize the full corpus.

## Rationale

Pinning the landed public revision removes the unreleased-source ambiguity while
keeping the provider body, consumer adapter, and published `aoa-kag@v0.5.0`
identity distinct. The provider PR's hosted source-fast, owner-family, full
OS-wide release audit, and summary gates are evidence for the landed source
revision; they do not imply runtime health, semantic acceptance, or artifact
admission.

## Consequences

- Source changes require regeneration through the exact landed owner revision
  and a new digest-bound budget receipt.
- CI must fetch and pin the same immutable action/provider revision.
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

Run the exact landed provider adapter, the local KAG provider lane, the
segmented owner-family gate, agent-surface and decision-index parity, then the
release checks. Hosted CI and merge are recorded separately from deployment,
runtime health, artifact trust, and semantic or human acceptance.
