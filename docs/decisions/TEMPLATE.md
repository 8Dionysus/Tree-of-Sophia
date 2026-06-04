# TOS-D-#### Short Decision Title

## Index Metadata

- Decision ID: TOS-D-####
- Original date: YYYY-MM-DD
- Surface classes: docs/route-law
- ToS layers: docs
- Tree classes: none
- Guard families: source-first authority
- Posture: proposed

## Context

What pressure made the decision necessary?

Name the source, intake, tree, example, generated, docs, or scripts surfaces that shaped the choice.

## Decision

State the chosen route in one or two paragraphs.

## Options Considered

- Option A:
- Option B:
- Option C:

## Rationale

Explain why this route fits ToS source-first authority, authored tree meaning, provenance, lineage, and bounded downstream seams.

## Consequences

Name what becomes easier, what remains constrained, and what future contributors must not infer from this decision.

## Source Surfaces

- `README.md`
- `CHARTER.md`
- `BOUNDARIES.md`

## Validation

Run:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

Also run the validator for the owning surface the decision describes.
