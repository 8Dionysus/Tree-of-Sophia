# Distillation Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | raw or witness-facing material must become reviewable ToS structure |
| input | source witness, note, synthesis candidate, principle candidate |
| output | source compost route or canon-facing candidate |
| owner | `mechanics/distillation/` |
| stronger route | `ToS/` owns source, philosophy, and canon meaning |
| next route | [Source Compost](parts/source-compost/docs/CONTEXT_COMPOST.md) |
| validation | `tos-ops-mechanics-plan --repo-root ABS --source-home` |

## Operations
| Part | Function |
| --- | --- |
| [Source Compost](parts/source-compost/docs/CONTEXT_COMPOST.md) | move source-linked material through raw, note, synthesis, principle, and canon-facing review |

The part owner is `mechanics/distillation/parts/source-compost/`. Its inputs are
raw sources, witnesses, notes and synthesis candidates; its outputs are
reviewable candidates or returns to source. Follow `ToS/source-witnesses/`,
`ToS/candidate-intake/` or `ToS/canon/` for the next route. The operation uses
source-home validators and `tos-ops-mechanics-plan --repo-root ABS --source-home`.

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `source-compost`: [CONTEXT_COMPOST](parts/source-compost/docs/CONTEXT_COMPOST.md)

## History

Immutable Git history preserves moved-path accounting from the former
`ToS/doctrine/CONTEXT_COMPOST.md` placement. Current source witnesses and
research packets stay in their source-home routes, not in mechanics legacy.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/distillation/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions

Add source-compost payload or split raw preservation, extraction, and candidate handoff when reviewed packets recur across branches. Preserve provenance through branch review.
