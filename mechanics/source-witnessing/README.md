# Source Witnessing Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `local` |
| trigger | source witness material needs route discipline before branch or canon movement |
| input | primary witness, published source metadata, translation layer, evidence status |
| output | witness route or return-to-source |
| owner | `mechanics/source-witnessing/` |
| stronger route | `ToS/source-witnesses/` owns witness material |
| next route | [Witness Route](parts/witness-route/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ABS --source-home` |

## Operations
| Part | Function |
| --- | --- |
| [Witness Route](parts/witness-route/README.md) | keep source witnesses distinct from branch, candidate, and canon material |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `witness-route`: [ACQUISITION_BATCH_ROUTE](parts/witness-route/docs/ACQUISITION_BATCH_ROUTE.md), [MANUAL_CORPUS_ENTRY_GATE](parts/witness-route/docs/MANUAL_CORPUS_ENTRY_GATE.md)

## History

Immutable Git history preserves moved-path accounting from the former
`ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE.md` placement. Source texts remain
active source-home witnesses, not mechanics legacy.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/source-witnessing/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Source Witnessing is planted as the local witness-route discipline. It protects
the difference between source-facing evidence, research scaffolds, branch
review, candidate extraction, and canon.

### Current Contour

- Keep witness-route around primary witnesses, published source metadata,
  translation posture, and evidence status.
- Keep source witnesses in `ToS/source-witnesses/`; mechanics owns the
  repeatable route discipline, not witness storage.
- Keep research packets outside source-witness authority until source anchoring
  and branch review happen.
- Keep prepared atlas rows and Deep Research dossiers weaker than source
  witnesses until they are anchored to editions, translations, corpora, or
  published evidence.

### Next Work

- Clarify witness movement when new source corpora repeatedly feed the same
  branch, candidate, or canon path.
- Keep capture containers and AI research scaffolds in research-packet routes,
  not witness authority.

### When Time Comes

Time has come when: witness-route receipts become repeated reviewed objects or
the same source-corpus movement recurs across branches.

Move: add payload to `witness-route` or split source entry, translation
posture, and witness-to-branch handoff into narrower operations.

Guardrail: source witnessing routes evidence; witness storage stays in
`ToS/source-witnesses/` and translation posture stays below canon.

### Out Of Scope

- Witness storage in mechanics.
- Translation-as-canon.
- Source erasure.
