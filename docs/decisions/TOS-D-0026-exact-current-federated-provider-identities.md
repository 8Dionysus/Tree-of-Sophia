# Exact-Current Federated Provider Identities

## Index Metadata

- Decision ID: TOS-D-0026
- Original date: 2026-08-23
- Surface classes: contracts, docs/route-law, generated, release
- ToS layers: docs, derived-exports
- Tree classes: none
- Guard families: source-first authority, identity separation, provenance, projection boundary, release integrity
- Posture: accepted; KAG integration prescriptions superseded by TOS-D-0072

## Current status: superseded KAG integration

The KAG provider/action pin, external checkout, builder, validator and CI-fetch
prescriptions below are historical and superseded by
[TOS-D-0072](TOS-D-0072-repository-independent-execution-and-exports.md).
ToS builds and verifies its own source-return export without `aoa-kag`.
Consumer indexing, provider configuration and admission run on the consumer's
side; they are not ToS commands, completion criteria or release gates.
Historical identities and evidence below remain unchanged; paths to retired
provider files describe that historical state, not files to recreate.

Only the KAG integration prescriptions are superseded; unrelated route and
source-authority rationale remains in force.

## Context

Tree-of-Sophia consumes generated and validation surfaces from neighboring
providers. The preceding Tree release named an older published `aoa-kag`
provider and an older `aoa-stats` revision, while the KAG provider campaign
subsequently published exact successors. The workflow action that generates
the local KAG family is a separate immutable action identity and must not be
mistaken for the published KAG provider release.

The pressure is release truth, not philosophical meaning: a consumer contract
that names an ancestor or moving branch can pass a green ancestry check while
still being stale against the provider that was actually published.

## Decision

Record direct federated dependencies in the Tree release contract by their
published tag and exact commit. Pin the workflow's `aoa-stats` checkout to the
same exact published commit and keep the immutable `repo-local-kag-index`
action commit separately named. Regenerate the local KAG family through that
action after source changes; generated indexes remain derived evidence and do
not become provider authority.

When a provider identity changes in a release-bearing contract, publish an
immutable Tree successor. Preserve all predecessor tags, Releases, and
artifact records. A new Tree artifact must bind its own exact source commit;
artifact consumer verdicts remain the artifact owner's independent results.

## Options Considered

- Keep the prior provider pins until a moving branch visibly changes. This
  permits stale consumer evidence and cannot prove exact-current compatibility.
- Record only provider tags or only provider commits. Tags provide release
  context and commits provide exact content identity; omitting either weakens
  reconciliation.
- Treat the KAG workflow action as the KAG provider release. This conflates
  execution machinery with provider content and breaks separate action/release
  identity.
- Record tag-plus-commit provider identities, retain the separate action pin,
  regenerate derived companions, and publish a successor when the edge moves.
  This is the chosen route.

## Rationale

The source-owned Tree contract should say which published provider it consumed,
while the KAG action owns how a derived family is generated. Exact ref equality
protects provider-before-consumer sequencing; it does not claim runtime health,
semantic proof, artifact admission, or human acceptance.

Keeping the generated family downstream preserves the source-first architecture:
ToS-authored meaning remains authoritative, provider and action identities stay
traceable, and a future agent can tell whether a failure belongs to source, CI,
artifact trust, runtime, or acceptance.

## Consequences

Future provider updates require a fresh exact-current revalidation and an
independent release decision. The scheduled canary may keep other sibling
inputs moving when its owner documents that lane; that canary is not immutable
release proof. Production or public artifact admission may remain
`manual_review_required` under the trust-root policy and must not be promoted
by source or release helpers.

This decision adds no authored philosophy, source-text, graph, canon, runtime,
semantic, or owner-acceptance claim.

## Source Surfaces

- `docs/RELEASING.md`
- `.github/workflows/repo-validation.yml`
- `README.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `kag/`
- `tests/test_roadmap_parity.py`

## Validation

Validate the exact provider tag/commit and release independently, then run the
action-owned full/incremental/family KAG checks, decision-index generation and
validation, the local provider and release gates, and the hosted Repo
Validation workflow. Artifact trust, runtime, proof, delivery, closure, and
acceptance remain separate claims.
