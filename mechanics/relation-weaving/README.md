# Relation Weaving Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `local` |
| trigger | graph-workbench fragments need relation-pack review or promotion |
| input | proposed node, proposed relation, branch fragment, promotion ledger |
| output | graph promotion route or return-to-review |
| owner | `mechanics/relation-weaving/` |
| stronger route | `ToS/philosophy/graph-workbench/` and `ToS/canon/relations/` own material |
| next route | [Graph Promotion](parts/graph-promotion/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --relation-pack-validate` |

## Operations
Each part owns one relation operation route.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Graph Promotion](parts/graph-promotion/README.md) | route proposed graph fragments toward reviewed relation packs | `ToS/philosophy/graph-workbench/` and `ToS/canon/relations/` |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts


### graph-promotion

## Operating Card

| Field | Route |
| --- | --- |
| role | route graph fragments toward reviewed relation packs |
| input | proposed node, proposed relation, branch fragment |
| output | relation-pack promotion route or return-to-review |
| owner | `mechanics/relation-weaving/parts/graph-promotion/` |
| next route | `ToS/philosophy/graph-workbench/` or `ToS/canon/relations/` |
| tools | `ToS/doctrine/RELATION_PACK_CONTRACT.md`, `tos-ops-mechanics-plan --repo-root ROOT --relation-pack-validate` |
| check | `tos-ops-mechanics-plan --repo-root . --relation-pack-validate` |

## Payload

- `tos-ops-mechanics-plan --repo-root ROOT --relation-pack-validate`

The script is local because relation-pack promotion is the repeatable
operation. The canonical relation carrier stays in `ToS/canon/relations/`, and
the candidate ledger stays in `ToS/candidate-intake/`.

## History

No package-local `legacy/` archive is maintained.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/relation-weaving/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Relation Weaving is active as the local graph-promotion contour. It protects
the difference between a graph workbench fragment and a reviewed relation pack.

### Current Contour

- Keep graph-promotion around `ToS/philosophy/graph-workbench/` and
  `ToS/canon/relations/`.
- Keep proposed nodes, proposed relations, branch fragments, and promotion
  ledgers visibly provisional until canon review.
- Keep generated graph views weaker than authored relation packs.
- Keep semantic graph layers as review lenses over tree growth, not a second
  relation ontology.

### Next Work

- Clarify relation movement when proposed graph fragments repeatedly ask to
  become relation packs.
- Keep graph visualization demands routed through derived/export boundaries,
  not canon shortcuts.
- Use Table I review packets to decide which relation movements are stable
  enough for relation-pack promotion.

### When Time Comes

Time has come when: relation promotion ledgers become repeated reviewed objects
outside the ordinary graph-workbench route.

Move: add payload to `graph-promotion` or split node proposal, relation
proposal, and relation-pack promotion into narrower operations.

Guardrail: relation weaving promotes reviewed relations through the canon gate.

### Out Of Scope

- Graph theater.
- Generated graph authority.
- Canon bypass.
