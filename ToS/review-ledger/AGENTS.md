# AGENTS.md

This file applies to dated review notes under `ToS/review-ledger/`.

## Role

`ToS/review-ledger/` preserves inspection notes, review outcomes, and migration
evidence for ToS surfaces.

Review notes can explain what was inspected. They do not overrule current
doctrine, source witnesses, canon, contracts, or decision records.

## Boundaries

- Keep entries dated and surface-specific.
- Do not use a review note as current doctrine.
- Do not promote archive or review-only examples into active compatibility
  surfaces without changing the owning branch.
- Route durable rationale to `docs/decisions/`.

## Validation

For review-ledger moves or edits, run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
```
