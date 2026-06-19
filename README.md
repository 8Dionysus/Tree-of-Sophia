# Tree of Sophia

`Tree-of-Sophia` is the source-first philosophical tree of the AoA ecosystem.
It turns sources, works, concepts, lineages, contexts, and reviewed relations
into a tree that can later be read as graphs without losing its roots.

AoA is the philosophical-engineering center. ToS carries the meaning-bearing
aim: grow philosophy from source witness through review into nodes, branches,
relations, and public routes that can be inspected and continued.

The tree should grow. The tree wants to grow. Growth is not accumulation:
growth is a branch that can show its root.

> Current release: `v0.2.2`. See [CHANGELOG](CHANGELOG.md).

## Start Here

| Need | Route |
| --- | --- |
| Short overview | this README -> [CHARTER](CHARTER.md) -> [DESIGN](DESIGN.md) |
| Authority boundary | [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) |
| Agent work | [AGENTS](AGENTS.md), then the nearest nested `AGENTS.md` |
| ToS source home | [ToS](ToS/README.md) |
| Philosophy domain tree | [ToS/philosophy](ToS/philosophy/) |
| Current golden route | [ToS/zarathustra](ToS/zarathustra/) |
| Doctrine, node law, canon | [ToS/doctrine](ToS/doctrine/) and [ToS/canon](ToS/canon/) |
| Repeatable operations | [mechanics](mechanics/README.md) |
| Current direction | [ROADMAP](ROADMAP.md) |
| Durable rationale | [docs/decisions](docs/decisions/README.md) |

This README chooses the route. It is not the source home, validator sheet, or
branch inventory.

## Current Public Route

The current bounded route keeps `README.md` as the public `tos-root` and routes
through a source-owned tiny-entry seam before any downstream export.

- human route: [ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) -> [ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY](ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md)
- machine-facing root-entry companion: `ToS/derived-exports/root_entry_map.min.json`
- bounded export seam: [mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT](mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md)

The tiny-entry seam is orientation, not a second canon. The export seam may
serve `aoa-kag`, graph consumers, and future visualization stacks, but it does
not become ToS authority.

## Repository Organs

| Organ | Owns |
| --- | --- |
| [ToS](ToS/README.md) | source home, doctrine, philosophy tree, canon, review ledger, public compatibility, derived exports |
| [mechanics](mechanics/README.md) | repeatable ToS operations and part-local contracts |
| [docs](docs/README.md) | decisions, releasing, root-reference receipts, and durable rationale |
| [scripts](scripts/AGENTS.md) and [tests](tests/) | builders, validators, and regression checks |
| [quests](quests/) and [QUESTBOOK](QUESTBOOK.md) | public quest items and obligation posture |

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
| `aoa-sdk` | typed helper and control-plane access layer |

## Working Rule

Grow ToS by making the next source route clearer. Add material where it
improves traceability, review, branch structure, or graph readiness without
weakening source authority. When detail belongs to a branch, mechanic,
decision, roadmap, changelog, generated companion, validator, or neighboring
organ, route it there.

## License

Apache-2.0
