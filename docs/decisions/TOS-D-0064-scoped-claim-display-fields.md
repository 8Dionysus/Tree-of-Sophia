# Explicit Claim display fields retain source and permission boundaries

## Index Metadata

- Decision ID: TOS-D-0064
- Original date: 2026-09-10
- Surface classes: doctrine/ontology, corpus/contract, mechanics/growth-cycle
- ToS layers: doctrine, contracts, source-witnesses, mechanics
- Tree classes: human forms, claim graph
- Guard families: source-first authority, scoped delegation, exact dependency binding, retained history
- Posture: accepted

## Context

Complete qualified Claim statements and source-owned navigation descriptors
serve different purposes. Neither supplies a concise source name, caption or
hover wording merely by being present. Reusing the navigation descriptor or
clipping a statement in the UI would lose that distinction. Adding fields to
the existing source-copy catalogue also risks silently widening older grants,
including raw form application and retries that bypass preparation.

## Decision

Use one optional versioned `qualifiers.display_fields` container for complete
authored `name`, `caption` and `hover` fields. Their language and script are
explicit and nullable; the full statement is required. Every materialization
binds the whole exact Claim, not just a convenient excerpt. Unknown versions
remain inert and preserved. A recognized malformed version fails closed.

Keep older public, private and compound source-copy grants statement-only.
Add a separate public form v2 grant with exact `allowed_field_ids`. Existing
public Claim revision configurations may opt into explicit
`allowed_form_field_ids` without changing their independent date/value/layer
authority. Application and retry check the new selector and exact current or
retained predecessor, not only the preparer's field selection.

This is additive to TOS-D-0057, TOS-D-0067 and TOS-D-0063. It does not turn
navigation text, catalogue dates or a mechanically ready form into a standalone
assertion or assessed knowledge.

## Options Considered

- Truncate statements or reuse navigation titles: rejected because attribution,
  negation and uncertainty can disappear and the source no longer owns wording.
- Use only separately assessed freeform forms: remains available for genuine
  paraphrase and translation, but is not required to copy an explicitly authored
  source field. Neither route substitutes for source-visible wording review.
- Expand every old form grant automatically: rejected because a readable new
  field is not permission to write it or revise its previous representation.
- Raise transport limits or remove repeated context: not selected. Existing
  compact budgets and exact inspection remain; a future lossless shared-context
  envelope would require a separate versioned consumer contract.

## Rationale

The same Claim owns both detailed and compact wording, with one version history.
Its name identifies the Claim; captions and hover text must preserve material
qualifications. Meaning is reviewed at the source, not inferred from a small
character count. Explicit field scope prevents the new reader from silently
becoming a broader writer, including after a field's role has changed.

## Consequences

New wording is a source correction followed by exact-bound forms, not an
overwrite of creation evidence. Mechanical readiness grants no assessment,
admission, rights or publication. Full context may exceed compact delivery
budgets; withheld roles retain exact inspection routes instead of clipped
wording. Existing records are not automatically migrated or declared complete.
Actual historical wording, consumer interaction and delivery remain separately
verified work.

## Source Surfaces

- `ToS/contracts/claim-display-fields.schema.json`
- `ToS/doctrine/HUMAN_FORMS.md`
- `scripts/source_witness_human_forms.py`
- `scripts/source_record_profiles.py`
- `mechanics/growth-cycle/parts/branch-growth-cycle/README.md`
- `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py`
- `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py`
- `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py`

## Validation

Exercise known/unknown field markers, exact source context, language and size
limits; old grant refusal; direct application and retained-predecessor replay;
explicit correction and history retention; and graph form selection. Check
private and compound statement-only regressions and command discovery. Generate
and check decision indexes, validate decision records and the source-home route.
Compact delivery and actual UI consumption require their own bounded checks.
