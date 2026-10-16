# Questbook Provenance Bridge

This is the active bridge from current Questbook routes to former-path
accounting. Former source material remains recoverable in immutable Git history.

## Current Route First

- [README](README.md)
- [PARTS](PARTS.md)
- [parts/obligation-boundary](parts/obligation-boundary/README.md)
- [parts/dispatch-contracts](parts/dispatch-contracts/README.md)
- root [QUESTBOOK](../../QUESTBOOK.md)

If those surfaces answer the task, stop there.

## Legacy Route

Historical old-path accounting is preserved in [TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md).

## Distillation Rule

When former Questbook material changes current behavior, update the owning
active part first, then this bridge with an exact immutable history reference.

The former reading-progression shell remains at commit
`0486a9a457e0291eb7716c330b7521f96912aa89`, path `mechanics/rpg/`.
Its obligation boundary now routes through the existing Obligation Boundary part.
