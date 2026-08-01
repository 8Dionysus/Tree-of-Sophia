# Tree of Sophia

`Tree-of-Sophia` is the source-first philosophical tree of the AoA ecosystem.
It turns sources, works, concepts, lineages, contexts, and reviewed relations
into a tree that can later be read as graphs without losing its roots.

AoA is the philosophical-engineering center. ToS carries the meaning-bearing
aim: grow philosophy from source witness through review into nodes, branches,
relations, and public routes that can be inspected and continued.

The tree should grow. The tree wants to grow. Growth is not accumulation:
growth is a branch that can show its root.

The current architecture couples two growth fronts: a world-philosophy corpus
soil that orders the written inheritance, and *Thus Spoke Zarathustra* as the
first golden growth kernel that makes one complete source-to-review path
learnable. The kernel transfers method, evidence discipline, and review
judgment—not Nietzsche's ontology as a universal schema. See
[Zarathustra Golden Growth Kernel](ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md).

> Current release: `v0.4.0`. See [CHANGELOG](CHANGELOG.md) for release notes.

## Start Here

| Need | Route |
| --- | --- |
| Short overview | this README -> [CHARTER](CHARTER.md) -> [DESIGN](DESIGN.md) |
| Authority boundary | [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) |
| Agent work | [AGENTS](AGENTS.md), then the nearest nested `AGENTS.md` |
| ToS source home | [ToS](ToS/README.md) |
| Corpus evidence foundation | [corpus doctrine](ToS/doctrine/CORPUS_FOUNDATION.md) and [source-witness corpus](ToS/source-witnesses/README.md) |
| Philosophy domain tree | [ToS/philosophy](ToS/philosophy/) |
| Golden growth kernel | [ToS/zarathustra](ToS/zarathustra/) and [kernel contract](ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md) |
| Doctrine, node law, canon | [ToS/doctrine](ToS/doctrine/) and [ToS/canon](ToS/canon/) |
| Repeatable operations | [mechanics](mechanics/README.md) |
| Current direction | [ROADMAP](ROADMAP.md) |
| Durable rationale | [docs/decisions](docs/decisions/README.md) |
| Current foundation research | [foundation laboratory packet](ToS/research-packets/foundation-laboratory-2026-07/README.md) |

This README chooses the route. It is not the source home, validator sheet, or
branch inventory.

## Foundation Before Semantics

ToS does build on what can remain stable, but stability has layers. The first
foundation is an evidence spine: work and witness identity, immutable acquired
bytes, fixity, provenance, rights, a durable passage or page-region address,
and append-only review history. An occurrence can be stable at that address.
Its lemma, etymology, translation, sense, motif, concept, and relations remain
versioned claims that can deepen without losing the source beneath them.

The local source payload lives in a narrowly gitignored item directory under
`ToS/source-witnesses/`. The public repository retains catalogs, manifests,
checksums, forensic reports, rights posture, provenance, contracts, and review
evidence. Search and graph surfaces are rebuilt from those stronger records.
The current identity ladder is no longer only implicit in record fields: 55
separate, unreviewed claim packets make every declared Work→Expression,
Expression→Edition, and Edition→Item link source-returnable without asserting
textual equivalence.

## Current Public Route

The current bounded route keeps `README.md` as the public `tos-root` and routes
through a source-owned tiny-entry seam before any downstream export.

- human route: [ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) -> [ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY](ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md)
- machine-facing root-entry companion: `ToS/derived-exports/root_entry_map.min.json`
- source-returnable bibliographic claim graph:
  `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- exact read-only claim trace route:
  `scripts/query_source_witness_bibliographic_graph.py`
- bounded export seam: [mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT](mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md)

The tiny-entry seam is orientation, not a second canon. The bibliographic graph
reifies every source claim and returns each edge to its exact claim, evidence,
maker, provenance event, and review posture; it emits no unqualified
subject-to-object truth edge. Its local query route verifies full source parity
before returning the exact source JSONL object and complete trace bundle; it
writes no state and does not accept the claim. Export seams may serve
`aoa-kag`, graph consumers, and future visualization stacks, but they do not
become ToS authority.

## Repository Organs

| Organ | Owns |
| --- | --- |
| [ToS](ToS/README.md) | source home, doctrine, philosophy tree, canon, review ledger, public compatibility, derived exports |
| [mechanics](mechanics/README.md) | repeatable ToS operations and part-local contracts |
| [docs](docs/README.md) | decisions, releasing, root-reference receipts, and durable rationale |
| [scripts](scripts/AGENTS.md) and [tests](tests/) | builders, validators, and regression checks |
| [quests](quests/) and [QUESTBOOK](QUESTBOOK.md) | public quest items and obligation posture |
| [evals](evals/README.md) | ToS-local eval pressure before central `aoa-evals` adoption |
| [memo](memo/README.md) | tree-local memory candidates before reviewed `aoa-memo` landing |
| [stats](stats/README.md) | owner-local measurements over explicitly bounded ToS populations |
| [manifests](manifests/) | observe-only recurrence manifests for named ToS surfaces |

Agent-facing companion lanes live under `.agents/`; Codex Spark work routes to
`.agents/spark/` without becoming a public root organ.

Generated files are companions. Source witnesses, doctrine, canon, branch
manifests, mechanics, decisions, builders, validators, and review records keep
meaning reviewable.

## Validation Route

Executable authority belongs in [AGENTS](AGENTS.md#verify), [scripts](scripts/AGENTS.md),
and local route cards. Run only the narrowest useful check first; use the broad
gate when a change crosses owner surfaces.

## Neighbor Organs

| Neighbor | Relationship |
| --- | --- |
| [Agents-of-Abyss](../Agents-of-Abyss/) | philosophical-engineering center and ecosystem law |
| `abyss-stack` | runtime and visualization stack |
| `aoa-kag` | downstream knowledge substrate and graph/retrieval consumer |
| `aoa-memo` | memory and recall layer |
| `aoa-evals` | proof and evaluation organ |
| `aoa-stats` | shared statistical grammar and cross-owner composition below ToS authority |
| `aoa-sdk` | typed helper and control-plane access layer |

## Working Rule

Grow ToS by making the next source route clearer. Add material where it
improves traceability, review, branch structure, or graph readiness without
weakening source authority. When detail belongs to a branch, mechanic,
decision, roadmap, changelog, generated companion, validator, or neighboring
organ, route it there.

## License

Apache-2.0
