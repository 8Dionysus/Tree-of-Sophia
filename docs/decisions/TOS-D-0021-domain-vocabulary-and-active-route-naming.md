# Domain Vocabulary And Active Route Naming

## Index Metadata

- Decision ID: TOS-D-0021
- Original date: 2026-07-22
- Surface classes: scripts/validation, contracts, source-witness, research-packet, docs/route-law
- ToS layers: scripts, contracts, source-witnesses, research-packets
- Tree classes: source, claim, relation
- Guard families: validator restraint, domain vocabulary, route-law, provenance preservation
- Posture: accepted

## Context

The active-naming validator was introduced while Tree of Sophia was removing
an earlier generation of route and progression names. Its broad token matcher
is useful for catching those names when they return in paths or path-like
identifiers.

The corpus foundation laboratory now has a different use for some of the same
ordinary words. `seed_claim_ref` names the root of a claim family;
`may_seed_drafts` and `may_seed_gold` are explicit false-valued authority
guards; and `first-wave` describes initial machine residency, not a repository
route. These identifiers were frozen before variant outputs and are covered by
content digests and provenance events.

Treating every such identifier as an old route would make the validator police
domain vocabulary. Renaming frozen fields after evidence was produced would
also require either an honest provenance migration and rerun or a false digest
rewrite with no source, quality, or semantic gain.

## Decision

Keep the retired-name check unchanged for active filesystem paths. Keep its
default content behavior unchanged for all identifiers except an exact,
reviewed set of corpus-domain references:

- `seed_claim_ref`
- `may_seed_drafts`
- `may_seed_gold`
- `first-wave`
- `first-wave-resident`

The exception applies only to content matches. It does not authorize these
tokens in paths, does not accept prefixes or suffix variants, and does not
weaken the separate checks for retired route labels, experience markers, or
normalized labels. Regression tests must prove both the exact allowed cases
and rejected near misses.

## Options Considered

- Rename the frozen fields and rewrite every dependent digest. This would
  create a provenance migration without changing laboratory meaning and would
  invalidate already captured run inputs unless every affected run were
  repeated.
- Remove the old token families from the validator. This would admit unrelated
  regressions across the whole active repository.
- Exclude the new corpus files from active-naming validation. This would hide
  their other identifiers and paths from the guard.
- Admit only the exact current domain references in file content while keeping
  path checks and all near-miss checks intact.

## Rationale

The selected route preserves both kinds of truth. Historical route vocabulary
remains retired, while the philosophical and laboratory domain is not forced
to distort its language around a repository migration. Exactness makes the
exception auditable and prevents it from becoming a generic bypass.

It also preserves the frozen-plan evidence chain. The graph and OCR plans keep
the same bytes and SHA-256 digests that their run receipts consumed, so release
validation no longer pressures provenance to conform to a lexical heuristic.

## Consequences

New content exceptions require owner-visible rationale and focused negative
tests. A new filesystem path never inherits a content exception. If a future
field is genuinely a renamed route rather than domain vocabulary, it must be
migrated instead of allowlisted.

The active-naming validator remains a route-law guard. It does not become an
arbiter of philosophical terminology, corpus semantics, or laboratory quality.

## Source Surfaces

- `scripts/validate_active_naming.py`
- `tests/test_validate_active_naming.py`
- `docs/decisions/TOS-D-0008-root-validation-route-unloading.md`
- `ToS/contracts/ocr-visual-sample-plan.schema.json`
- `ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/graph-queries.json`
- `ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/ocr-visual-samples.json`
- `ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/translation-laboratory-plan.v1.json`

## Validation

Run the focused active-naming tests, the active-naming validator, decision
index regeneration and validation, the source-witness foundation validator,
and the repository release lane. Structural green status does not alter the
manual-review posture of any laboratory artifact.
