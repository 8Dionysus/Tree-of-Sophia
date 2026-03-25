# AGENTS.md

This file applies to the small generator and validator tools under `scripts/`.

## What lives here

`scripts/` owns the bounded tiny-export seam and its local documentation guardrails.
The current scripts are:

- `generate_kag_export.py`
- `validate_kag_export.py`
- `validate_nested_agents.py`

## Editing posture

Keep these tools deterministic, local, and source-owned.
Do not add network calls, hidden state, or broad corpus automation here.
Keep the pilot narrow to the current Zarathustra route.
Prefer explicit file dependencies over discovery magic.

## Validation

Run:

```bash
python scripts/validate_nested_agents.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
