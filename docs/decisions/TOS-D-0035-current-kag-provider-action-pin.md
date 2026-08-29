# Current KAG provider and action pin supersedes the historical route

## Index Metadata

- Decision ID: TOS-D-0035
- Original date: 2026-08-29
- Surface classes: release/compatibility, GitHub workflow, KAG consumer, validation, docs/route-law
- ToS layers: derived-export, docs, validation, owner-handoff
- Tree classes: provider edge, executable action, source release, generated companion
- Guard families: exact identity, action provenance, release immutability, claim separation
- Posture: accepted

## Context

TOS-D-0030 records the accepted historical compatibility route with provider
`f46f146cc79a26fa81ad0f400b9c5774df293e57` and executable action
`6a79e62c7d20b6b11406dee78f409ada4a51bb3f`. The current aoa-kag source route
has since been repaired and now exposes a different provider/action pair. The
current pair must be recorded without rewriting the evidence for the earlier
immutable pins.

## Decision

For the current Tree-of-Sophia source route, supersede TOS-D-0030's active pin
only with these exact identities:

- current provider source snapshot (not a published release tag) at
  `47598411fba56f126a8530cb1e7e91bed57f5fef`;
- executable workflow action: `repo-local-kag-index` at
  `25cd6263ae2c860c58f86cf3a0747f2070eb45ff`.

The provider and action remain separate roles even when their commits or
payloads change. The workflow, release contract, parity test, generated
receipt, and owner validation route must bind the current refs explicitly and
assert the intentional distinction. TOS-D-0030 remains unchanged as the
historical rationale for the earlier pair; this record is its auditable
supersession for the current route. The published `aoa-kag@v0.5.0` identity
remains `f46f146cc79a26fa81ad0f400b9c5774df293e57`; no tag or GitHub Release
is altered by this current-source pin.

## Evidence

- `git show 25cd6263ae2c860c58f86cf3a0747f2070eb45ff:.github/actions/repo-local-kag-index/action.yml`
  is the current executable action source.
- `git show 47598411fba56f126a8530cb1e7e91bed57f5fef:.github/actions/repo-local-kag-index/action.yml`
  is the current provider-era action source; the two action snapshots remain
  independently identified.
- `.github/workflows/repo-validation.yml`, `docs/RELEASING.md`, and
  `tests/test_roadmap_parity.py` bind the current provider and action refs and
  assert their role separation.
- The identity-bound receipt carries `candidate_identity`,
  `head_source_snapshot`, and `producer_identity`; those are currentness and
  provenance evidence, not runtime activation, proof, delivery, closure,
  owner acceptance, or human acceptance.

## Options Considered

- Rewrite TOS-D-0030 with the new refs. Rejected because it erases the
  historical identity decision and breaks auditability of the earlier route.
- Keep the old refs as the current route. Rejected because they no longer
  describe the repaired provider/action source used by the current workflow.
- Add this superseding record while preserving TOS-D-0030. Chosen because it
  keeps both historical provenance and current compatibility explicit.

## Consequences

Future provider or action changes require another exact identity review and a
new superseding record. A green source or owner check proves the declared
mechanics only; it does not by itself prove artifact admission, runtime
activation, semantic acceptance, closure, or human acceptance.

## Source Surfaces

- `.github/workflows/repo-validation.yml`
- `docs/RELEASING.md`
- `tests/test_roadmap_parity.py`
- `kag/indexes/index_family.manifest.json`
- `kag/receipts/index_family_budget/<family-digest>.json`
- `docs/decisions/TOS-D-0030-exact-kag-action-provider-alignment.md`
- `docs/decisions/TOS-D-0035-current-kag-provider-action-pin.md`

## Validation

Run the focused release-contract parity test, decision-index regeneration and
validation, the current action/provider owner checks, the Tree release gate,
and the required GitHub Repo Validation check.
