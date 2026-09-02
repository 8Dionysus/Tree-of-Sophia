# Sealed expectation arithmetic failure — comparison v1

Observed: 2026-08-30  
Comparison result SHA-256: `33019dde…1bee`  
Result: 24/25 gates passed; overall fail

## Failed gate

`G14-c-derived-pass-primary-generic-fail` expected six resources for the
public-synthetic `PC1-uxlc-shape` projection. The materialized and manually
inspected projection has seven:

```text
1 book + 1 chapter + 2 verses + 3 words = 7
```

The original sealed manifest encoded `6`, and the evaluator also repeated that
literal. This was a preregistration arithmetic error, not a source discrepancy
or candidate failure.

## Correction boundary

- preserve the original sealed manifest and failed result digest;
- create a versioned sealed manifest changing only the two PC1 resource-count
  expectations from 6 to 7;
- make the evaluator read that registered expectation instead of duplicating a
  literal;
- revision-freeze the changed sealed manifest and evaluator;
- do not rebuild or modify candidate outputs, source receipts, independent
  consumer receipt or manual verdict.

The exact UXLC source expectation remains 28 resources and is unchanged.
