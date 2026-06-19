# AGENTS.md

This card applies to `mechanics/audit/`.

## Role

Audit routes source-home inspection evidence to the review surface that can act
on it.

## Boundary Routes

Review evidence routes to source review, decision rationale, or the affected
mechanic. Source truth, proof authority, and remediation authority stay with
their owning surfaces.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
```
