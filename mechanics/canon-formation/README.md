# Canon Formation Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `local` |
| trigger | candidate, branch, node, or relation material asks to become canon |
| input | source anchor, review state, candidate object |
| output | promotion gate and validation route |
| owner | `mechanics/canon-formation/` |
| stronger route | `ToS/canon/` owns canonical objects |
| next route | [Promotion Gate](parts/promotion-gate/README.md) |
| validation | canon and relation validators |

## Operations
| Part | Function |
| --- | --- |
| [Promotion Gate](parts/promotion-gate/README.md) | route reviewed candidates into canonical ToS surfaces without bypassing source review |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts


### promotion-gate

## Operating Card

| Field | Route |
| --- | --- |
| role | route reviewed material toward canon without bypassing source review |
| input | candidate object, source anchor, evidence status |
| output | canon promotion route or return-to-review |
| owner | `mechanics/canon-formation/parts/promotion-gate/` |
| next route | `ToS/canon/` only after review and validators pass |
| tools | `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`, canon validators |
| check | `tos-ops-mechanics-plan --repo-root ROOT --tree-node-validate` |

## History

No package-local `legacy/` archive is maintained. Candidate and canon material remains in
the `ToS/` source home.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/canon-formation/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Canon Formation is planted as the ToS local promotion gate. Canon admission follows the declared source evidence and review route.

### Current Contour

- Keep promotion-gate posture local to movement toward `ToS/canon/`.
- Keep source anchors, evidence status, candidate posture, and review state
  visible before anything becomes canonical.
- Keep canonical objects stored in `ToS/canon/`, not mechanics.
- Keep graph-workbench proposed nodes and relations pre-canon until a reviewed
  promotion route accepts them.

### Next Work

- Clarify promotion criteria when branch review and candidate intake start
  producing repeated canon requests.
- Keep promotion residue in the source branch, candidate lane, or canon owner
  surface until a repeated operation needs a mechanics route.
- Use the first Table I review passes to learn which promotion checks are real
  and which should stay branch-local.

### When Time Comes

Time has come when: promotion ledgers repeatedly outgrow
`ToS/philosophy/graph-workbench/`, `ToS/candidate-intake/`, or the local canon
owner route.

Move: add payload to `promotion-gate` or split node, relation, vocabulary, or
synthesis promotion into a narrower part.

Guardrail: the part routes promotion; canon storage stays under `ToS/canon/`.

### Out Of Scope

- Canon storage.
- Automatic canonization.
- Zarathustra core relocation.
