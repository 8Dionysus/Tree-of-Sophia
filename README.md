# Tree of Sophia

`Tree-of-Sophia` is the source-first philosophical tree of the AoA ecosystem.
It is where texts, works, concepts, lineages, contexts, and reviewed relations
become legible without being replaced by runtime, proof, memory, routing, or
downstream graph machinery.

AoA gives the wider organism its philosophical-engineering center. ToS carries
the highest meaning-bearing aim: grow the tree of philosophy from sources,
through review, into nodes and graphs that can be inspected, challenged, and
continued.

The tree should grow. The tree wants to grow. Growth is not accumulation:
growth is a branch that can show its root.

> Current release: `v0.2.2`. See [CHANGELOG](CHANGELOG.md) for release notes.

## What This Repository Does

| Function | Surface |
| --- | --- |
| Repository authority | [CHARTER](CHARTER.md) |
| System form | [DESIGN](DESIGN.md) |
| Agent-facing form | [DESIGN.AGENTS](DESIGN.AGENTS.md) |
| Owner boundaries | [BOUNDARIES](BOUNDARIES.md) |
| Agent route law and checks | [AGENTS](AGENTS.md), then the nearest nested `AGENTS.md` |
| ToS source home | [ToS](ToS/README.md) |
| Philosophical growth tree | [ToS/philosophy](ToS/philosophy/) |
| Zarathustra golden route | [ToS/zarathustra](ToS/zarathustra/) |
| Doctrine and node law | [ToS/doctrine](ToS/doctrine/) |
| Canon and reviewed node surfaces | [ToS/canon](ToS/canon/) |
| Repeatable ToS operations | [mechanics](mechanics/README.md) |
| Decisions and durable rationale | [docs/decisions](docs/decisions/README.md) |
| Quests and obligations | [QUESTBOOK](QUESTBOOK.md), [quests](quests/) |

This repository is strongest when it keeps meaning source-owned and makes the
next branch route obvious. It is weakest when the root README becomes an
inventory, a validator sheet, or a substitute for the source home.

## Start Here

Read only the route that matches the work.

| Need | Route |
| --- | --- |
| Shortest honest overview | this README -> [CHARTER](CHARTER.md) -> [DESIGN](DESIGN.md) -> [ToS](ToS/README.md) |
| Decide whether something belongs here | [CHARTER](CHARTER.md) -> [BOUNDARIES](BOUNDARIES.md) |
| Work as an agent | [AGENTS](AGENTS.md), then the nearest nested route card |
| Change doctrine, node shape, or source law | [ToS/doctrine](ToS/doctrine/) and [ToS/AGENTS](ToS/AGENTS.md) |
| Add or review source-facing material | [ToS/AGENTS](ToS/AGENTS.md), then the owning branch route |
| Change mechanics | [mechanics](mechanics/README.md), package `AGENTS.md`, package `ROADMAP.md`, and active part route |
| Change public/export seams | source surface -> builder/export route -> generated companion -> local validator |
| Run validation | [AGENTS](AGENTS.md#verify) and [scripts](scripts/AGENTS.md) |
| Change direction | [ROADMAP](ROADMAP.md) |
| Explain why a boundary changed | [docs/decisions](docs/decisions/README.md) |

- current direction: [ROADMAP](ROADMAP.md)

## Current Public Route

The current bounded route keeps `README.md` as the public `tos-root` and routes through a source-owned tiny-entry seam before any downstream export.

- if you are new here and want the one real current public route: [ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) and [ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY](ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md)
- if you want the compact machine-facing companion to that same root path: `ToS/derived-exports/root_entry_map.min.json`
- if you need the bounded downstream export seam for that route: [mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT](mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md)

The tiny-entry seam is not a second canon. It is a public orientation path that
returns to ToS-authored authority surfaces. The bounded export seam may serve
`aoa-kag` and graph consumers, but it does not become ToS authority.

## Layer Check

| Question | Owner route |
| --- | --- |
| What may ToS claim? | [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) |
| What is the current form of the tree? | [DESIGN](DESIGN.md), [ToS](ToS/README.md), [ToS/philosophy](ToS/philosophy/) |
| What is currently being proved? | [ROADMAP](ROADMAP.md) |
| What owns a source, node, work, concept, lineage, or relation? | nearest authored ToS surface, branch manifest, doctrine card, or canon file |
| What owns a recurring operation? | [mechanics](mechanics/README.md), then package and part cards |
| What owns generated companions? | the source surface and builder that produced them |
| What owns runtime, proof, memory, stack, KAG substrate, or federation logic? | the neighboring AoA repository named by [BOUNDARIES](BOUNDARIES.md) |

## Validation Route

Executable authority belongs in [AGENTS](AGENTS.md#verify), [scripts](scripts/AGENTS.md),
and local route cards. This README chooses the route; it does not run it.

## Core Districts

| District | Use for |
| --- | --- |
| [ToS](ToS/README.md) | source home, doctrine, philosophy tree, canon, review ledger, and public routes |
| [mechanics](mechanics/README.md) | repeatable ToS operations and part-local contracts |
| [docs](docs/README.md) | decisions, root references, and durable rationale |
| [schemas](schemas/) and [examples](examples/) | public-safe contracts and examples when present |
| [generated](generated/) and [ToS/derived-exports](ToS/derived-exports/) | derived companions tied back to source |
| [scripts](scripts/AGENTS.md) and [tests](tests/) | deterministic builders, validators, and regression checks |
| [manifests](manifests/) and [quests](quests/) | recurrence hooks and durable obligations |

Generated files are companions, not authority. Source witnesses, doctrine,
canon, branch manifests, mechanics, decisions, builders, validators, and review
records keep the meaning reviewable.

## Neighbor Organs

| Neighbor | Relationship |
| --- | --- |
| [Agents-of-Abyss](../Agents-of-Abyss/) | philosophical-engineering center and ecosystem law |
| `aoa-kag` | downstream knowledge substrate and retrieval/graph projection consumer |
| `abyss-stack` | runtime and visualization stack for serving the tree |
| `aoa-memo` | durable memory and recall layer |
| `aoa-evals` | proof and evaluation organ |
| `aoa-sdk` | typed helper and control-plane access layer |

## Working Rule

Grow ToS by making the next source route clearer.

Add material only where it improves traceability, review, branch structure, or
graph readiness without weakening source authority. When detail belongs to a
branch, mechanic, decision, roadmap, changelog, generated companion, validator,
or neighboring organ, route it there instead of making this README carry it.

## License

Apache-2.0
