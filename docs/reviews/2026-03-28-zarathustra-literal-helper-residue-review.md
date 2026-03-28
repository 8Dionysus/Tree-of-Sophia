# Zarathustra Literal Helper Residue Review

Date: 2026-03-28

## Summary

After closing the route-local analogy and synthesis residues, the bounded
Zarathustra `prologue-1` route keeps only literal helper residue in `intake/`.

The remaining helper rows are:

- `literal.ten_years`
- `literal.too_much`
- 3 `deferred_literal` edges

## Decision

This residue is intentional end-state, not temporary delay.

The current route does not open a `literal` family.
It keeps literal helpers visible only for referential closure inside the intake
ledger while preserving authored canon in `tree/`.

## Rationale

These helpers carry bounded numeric or quantity support, not source-facing
authored meaning that currently warrants its own canonical family.

Leaving them explicit in intake is more honest than either:

- silently deleting them, or
- inflating them into a premature authored ontology.
