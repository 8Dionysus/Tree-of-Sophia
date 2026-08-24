# Explicit separate KAG action ABI and provider identity

## Index Metadata

- Decision ID: TOS-D-0030
- Original date: 2026-08-24
- Surface classes: release/compatibility, GitHub workflow, KAG consumer, validation
- ToS layers: derived-export, docs, validation, owner-handoff
- Tree classes: provider edge, executable action, source release, generated companion
- Guard families: exact identity, action provenance, release immutability, claim separation
- Posture: accepted

## Context

Tree-of-Sophia consumes the published `aoa-kag@v0.5.0` provider body at
`f46f146cc79a26fa81ad0f400b9c5774df293e57`. Its workflow uses the older,
immutable repo-local KAG action identity
`6a79e62c7d20b6b11406dee78f409ada4a51bb3f`. Those commits contain different
executable action snapshots. Treating them as interchangeable would falsify
action provenance, while changing the workflow ref would change a separate
CI dependency rather than merely correcting the provider body.

## Decision

For the current source route, retain two explicit immutable identities:

- provider body: `aoa-kag@v0.5.0` at
  `f46f146cc79a26fa81ad0f400b9c5774df293e57`;
- executable workflow action: `repo-local-kag-index` at
  `6a79e62c7d20b6b11406dee78f409ada4a51bb3f`.

The consumer workflow, release contract, and parity test must carry both refs,
assert their intentional inequality, and keep the role labels visible. This
separate-action ABI closes the provenance ambiguity without publishing a
successor or changing any existing tag or GitHub Release.

The action remains an executable CI dependency and the provider remains a
published source identity even when their commit is equal. A green action or
provider check does not establish artifact admission, runtime activation,
proof, delivery, closure, owner acceptance, or human acceptance.

## Evidence

- `git show 6a79e62c7d20b6b11406dee78f409ada4a51bb3f:.github/actions/repo-local-kag-index/action.yml`
  is the executable action source used by the workflow.
- `git show f46f146cc79a26fa81ad0f400b9c5774df293e57:.github/actions/repo-local-kag-index/action.yml`
  is the provider-era action source; its bytes differ from the `6a79` action
  and it is not substituted silently.
- `.github/workflows/repo-validation.yml`, `docs/RELEASING.md`, and
  `tests/test_roadmap_parity.py` bind both refs and assert their inequality.
- The owner action gate is executed against the current Tree source and the
  provider source is independently validated at `f46`; those are executable
  compatibility evidence, not an artifact or runtime admission claim.

## Consequences

Future action or provider changes require their own exact source refs and a
new compatibility review. Neither identity can be inherited by an artifact
record for the other role.

## Source Surfaces

- `.github/workflows/repo-validation.yml`
- `docs/RELEASING.md`
- `CHANGELOG.md`
- `tests/test_roadmap_parity.py`
- `docs/decisions/TOS-D-0030-exact-kag-action-provider-alignment.md`

## Validation

Run the focused release-contract parity test, decision-record/index validators,
the executable `6a79` owner-family gate against current source, the provider
validation route at `f46`, the Tree release gate, and the required GitHub Repo
Validation check.
