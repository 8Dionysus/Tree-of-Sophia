# Questbook Mechanic

<a id="active-route"></a>
<a id="historical-provenance"></a>

Questbook is the ToS-local route for public obligations and dispatch
compatibility. It keeps tasks and obligations from replacing philosophy.

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | ToS obligation posture, quest schemas, catalog examples, or dispatch examples change |
| input | root `QUESTBOOK.md`, root `quests/`, obligation boundary docs, schemas, examples |
| output | public obligation boundary and dispatch contracts |
| owner | `mechanics/questbook/AGENTS.md`, the operation map below, and active part routes |
| stronger route | `ToS/` for authored meaning; root `quests/` for public quest records |
| next route | obligation boundary or dispatch contracts, then root `QUESTBOOK.md` when public index changes |
| validation | `tos-ops-mechanics-plan --repo-root . --questbook-validate`; `tos-ops-mechanics-plan --repo-root . --mechanics-topology-validate`; questbook-focused tests |

## Operations
Each part owns one obligation operation route.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Obligation Boundary](parts/obligation-boundary/README.md) | keep ToS obligations operational without replacing source interpretation | `ToS/` source-home authority |
| [Dispatch Contracts](parts/dispatch-contracts/README.md) | schema and example surfaces for public quest catalog and dispatch | root `quests/` and `QUESTBOOK.md` |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `obligation-boundary`: [QUESTBOOK_TOS_INTEGRATION](parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md)

## History

This is the active bridge from current Questbook routes to former-path
accounting. Former source material remains recoverable in immutable Git history.

## Legacy Route

Historical old-path accounting is preserved in [TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md).

## Distillation Rule

When former Questbook material changes current behavior, update the owning
active part first, then this history route with an exact immutable history reference.

The former reading-progression shell remains at commit
`0486a9a457e0291eb7716c330b7521f96912aa89`, path `mechanics/rpg/`.
Its obligation boundary now routes through the existing Obligation Boundary part.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/questbook/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Questbook is active as public obligation routing only. It keeps obligations
visible without turning philosophy into a task list.

### Current Contour

- Keep obligation boundary and dispatch contracts distinct.
- Keep root `quests/` as public quest source records while
  `mechanics/questbook/` owns the repeatable obligation posture.
- Keep interpretation, source, branch, and canon pressure routed back to ToS
  owner surfaces.
- Keep graph-review and atlas-growth obligations public only when they are
  durable follow-through, not private working residue.

### Next Work

- Add narrower parts only if quest lifecycle, closure, or public examples
  become distinct repeated operations.
- Keep interpretation pressure routed to `ToS/`, not to Questbook.

### When Time Comes

Time has come when: public obligations repeatedly need lifecycle, closure, or
example handling that dispatch contracts should not own.

Move: add a narrower part or route the recurring shape to root `quests/`.

Guardrail: a quest may preserve follow-through; it may not become
philosophical authority.

### Out Of Scope

- Philosophical truth as backlog.
- Live task execution or runtime scheduling.
- Hidden delegation.
- Memory truth, proof verdicts, or ToS canon mutation.

## Obligation membership and compatibility

The existing [QUESTBOOK register](../../QUESTBOOK.md) owns all five retained
IDs, current follow-through states, owners and return conditions. Original
`quests/` records retain exact historical evidence. Native assertions are the
named consumer of legacy YAML schemas and examples; their count and identifiers
are not requirements for the core register. No SDK runtime ToS consumer has been
established here. Compatibility entries remain available to unknown readers.
