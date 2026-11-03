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
| next route | [Source Compost](#context-compost) |
| validation | `tos-ops-mechanics-plan --repo-root ABS --source-home` |

## Operations
| Part | Function |
| --- | --- |
| [Source Compost](#context-compost) | move source-linked material through raw, note, synthesis, principle, and canon-facing review |

The part owner is `mechanics/distillation/parts/source-compost/`. Its inputs are
raw sources, witnesses, notes and synthesis candidates; its outputs are
reviewable candidates or returns to source. Follow `ToS/source-witnesses/`,
`ToS/candidate-intake/` or `ToS/canon/` for the next route. The operation uses
source-home validators and `tos-ops-mechanics-plan --repo-root ABS --source-home`.

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `source-compost`: [CONTEXT_COMPOST](#context-compost)

## Context Compost

Context Compost is the source and review method for turning witness-facing or
other source-linked material into layered ToS knowledge. [PRACTICE_BRANCH](../../ToS/doctrine/PRACTICE_BRANCH.md)
addresses neighboring practice genealogies; [COUNTERPART_POLICY](../../ToS/doctrine/COUNTERPART_POLICY.md)
addresses optional bridges to AoA operational forms. Runtime implementation
follows its owner route.

### Core rule

Compost stays source-first, reviewable, layered, reversible and
non-platform-sovereign in this phase. Executable schema coverage follows its
implementation route.

### Canonical compost cycle

`raw -> note -> synthesis -> principle -> canon`

#### `raw`

Undigested intake, such as source excerpts, commentary fragments,
`WitnessTrace` artifacts or bounded summaries awaiting source review.

#### `note`

The first bounded artifact keeps source refs, review state, visible limits and
enough context to return to its sources.

#### `synthesis`

A revisable, source-linked interpretation distills multiple notes and retains
links to those notes and their source refs.

#### `principle`

A higher-order statement remains review-gated and reversible, with a visible
route to its source basis.

#### `canon`

Explicit, reviewable promotion stabilizes canon while retaining its source
route and promotion evidence.

### Contract objects

#### `CompostNode`

A bounded artifact records its cycle stage, reviewable summary, source refs,
review state, visible limits and relevant lineage or relation notes.

#### `SourceRef`

A reference to supporting ToS-authored sources, excerpts, commentary fragments
or witness-facing route artifacts.

#### `DecayPolicy`

A named posture for staleness, refresh and demotion.

#### `CanonBundle`

A reviewable bundle stabilizing one or more canon-facing outputs with source
route and promotion history.

### Worked path

#### `WitnessTrace -> Note -> Principle`

1. `WitnessTrace` enters as `raw` context with source, status and original ownership.
2. A bounded `note` preserves source links, review state and limits.
3. A `principle` may be proposed after review, retaining explicit source refs.

#### Memory bridge fragment

The public bridge-bearing memo pilot may expose one bounded, source-first,
non-identity, reviewable fragment subordinate to authored ToS doctrine. It may
support memo-side or KAG-side bridge readiness references. Memory, runtime and
derived-substrate decisions remain with their owners.

### Ownership boundary

- `aoa-playbooks`: scenario route.
- `aoa-memo`: witness-facing memory contracts.
- `aoa-evals`: proof of trace integrity and provenance preservation.
- `Tree-of-Sophia`: compost-cycle doctrine and canon-facing digestion.
- `abyss-stack`: later runtime instrumentation.

### Connection to later growth

This intake may prepare `TOS-GROWTH-03`, `TOS-GROWTH-05` and `TOS-GROWTH-08`.

## History

Immutable Git history preserves moved-path accounting from the former
`ToS/doctrine/CONTEXT_COMPOST.md` placement. Current source witnesses and
research packets stay in their source-home routes, not in mechanics legacy.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/distillation/PROVENANCE.md) preserves the pre-compaction route.

## Growth conditions

Add source-compost payload or split raw preservation, extraction, and candidate handoff when reviewed packets recur across branches. Preserve provenance through branch review.
