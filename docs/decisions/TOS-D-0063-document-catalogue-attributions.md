# Document catalogue dates and locations are separate attributed Claims

## Index Metadata

- Decision ID: TOS-D-0063
- Original date: 2026-09-10
- Surface classes: doctrine/ontology, corpus/contract, access/backend, mechanics/growth-cycle
- ToS layers: doctrine, contracts, source-witnesses, access, mechanics
- Tree classes: claim graph, human forms, semantic interchange
- Guard families: source-first authority, exact dependency binding, claim reification, temporal roles, scoped delegation
- Posture: accepted

## Context

The Letter 705 catalogue card exposes a date and origin/destination fields.
The existing Document identity correctly keeps dates and places outside its
metadata, while `historical_dating` and `historical_place` describe historical
situations. Reusing those predicates would turn a catalogue's attribution into
an assertion about composition, dispatch, receipt or a separate commissioning
event. The previous typed-time navigation decision TOS-D-0062 covers historical
time only; it does not authorize this different meaning or its source writes.

## Decision

Add `document_catalogue_date`, `document_catalogue_origin` and
`document_catalogue_destination` with a Document domain (including Letter).
All require exact catalogue attribution: cited evidence, source field label,
separate field role and original wording/language. Date wording must match the
whole temporal value. The date reader `document-catalogue-temporal-v1` and its
`catalogue-assigned-document-date` role remain distinct from historical time.
Elementary date/interval/unknown mechanics are reused, not historical-domain
authority or relative anchors. Origin and destination retain the existing
identity-relation reader with their new narrow schema.

Date creation/correction receives separate exact-value owner configurations;
old public and private grants are not widened. Place relations use existing
exact-predicate identity grants. Template version 3 explicitly adds a separate
source-wording navigation adapter; full `claim.statement` HumanForms retain
the entire qualified Claim. Source comparison requires exact profile, current
operand revision, Document subject and full Claim/value/file binding. It
compares only equal time roles and never infers missing calendars/numbering.

This adds a distinct successor capability to TOS-D-0062, without changing its
historical reader or the prior template's meaning.

## Options Considered

- Widen historical dating/place to Documents: rejected because it loses the
  distinction between a catalogue assignment and an asserted event occurrence.
- Store date/place convenience fields on the Document: rejected because it
  hides evidence, alternative attributions and independent Claim histories.
- Add a new reader/grant hierarchy for every catalogue location: not selected;
  existing identity relations and exact predicate allowlists already supply
  that boundary. Only the genuinely new temporal reader requires a new grant.
- Keep untyped prose only: insufficient for the requested typed comparison
  and discovery path, though prose remains necessary qualified reading context.

## Rationale

Catalogue fields have operational usefulness without becoming historical
truth. One finite profile and three explicitly qualified relations preserve
that distinction across source, operations, normalized access and HumanForms.
Source wording is never replaced by a normalized date or another source's
typography. Neither the registry nor discovery admits a source, creates an
identity or grants a write.

## Consequences

Competing catalogue assignments can remain distinct Claims. Unknown calendar
or year numbering yields undetermined comparison, not inferred coordinates.
Different temporal roles are unsupported as a pair. Actual Naumburg identity,
Letter Claims, source grants, assessment and deployment remain separate owner
work; this change creates none of them. The first runnable examples are
synthetic test attributions over copied existing metadata, not source evidence.

## Source Surfaces

- `ToS/contracts/document-catalogue-claim.schema.json`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/doctrine/semantic-interchange/README.md`
- `ToS/doctrine/HUMAN_FORMS.md`
- `scripts/source_document_catalogue.py`
- `scripts/source_record_profiles.py`
- `mechanics/growth-cycle/parts/branch-growth-cycle/README.md`
- `access/src/tos_access/knowledge.py`
- `access/src/tos_access/temporal_comparison.py`

## Validation

Run the bounded source-command end-to-end fixture, old temporal regression,
operation discovery and temporal comparison tests; check the registry against
the exact pre-change revision. Generate/check decision indexes and validate
decision records. Full corpus regeneration and actual consumer acceptance
belong to the integration owner, separately from local checks and checkpoint.
