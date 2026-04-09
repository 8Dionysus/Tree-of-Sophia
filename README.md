# Tree of Sophia (ToS)

Tree of Sophia is a source-first living knowledge architecture for philosophy and world thought. It traces texts, concepts, contexts, and lineages across time and cultures while keeping source-linked authority visible as the tree grows.

It is not just a notes repository, not just a graph, and not just a retrieval substrate. It is the architectural root where source-linked meaning should stay legible as the wider ecosystem grows around it.

This repository currently carries four public layers:

- `sources/` for primary witness and source material
- `intake/` for candidate structure that stays visibly provisional
- `tree/` for canonical authored nodes, relations, and vocabulary governance
- `examples/` and `generated/` for bounded public compatibility and downstream-safe export seams

## Start here

Use the shortest route by need:

- if you are new here and want the one real current public route: [docs/TINY_ENTRY_ROUTE](docs/TINY_ENTRY_ROUTE.md) and [docs/ZARATHUSTRA_TRILINGUAL_ENTRY](docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md)
- if you want the compact machine-facing companion to that same root path: `generated/root_entry_map.min.json`
- if you need the bounded downstream export seam for that route: [docs/KAG_EXPORT](docs/KAG_EXPORT.md)
- if you want to verify the current bounded route: [CHARTER](CHARTER.md), [BOUNDARIES](BOUNDARIES.md), `python scripts/build_root_entry_map.py --check`, `python scripts/validate_root_entry_map.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, `python -m unittest discover -s tests`, and [docs/REVIEW_CHECKLIST](docs/REVIEW_CHECKLIST.md) for surfaces outside the current validator perimeter
- mission and source-of-truth boundaries: [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md)
- knowledge model and interpretation law: [docs/KNOWLEDGE_MODEL](docs/KNOWLEDGE_MODEL.md) and [docs/NODE_CONTRACT](docs/NODE_CONTRACT.md)
- current direction: [ROADMAP](ROADMAP.md)
- growth law and curation posture: [docs/CONTEXT_COMPOST](docs/CONTEXT_COMPOST.md), [docs/CALIBRATION_AXIS](docs/CALIBRATION_AXIS.md), [docs/HUMAN_CURATED_EXPANSION](docs/HUMAN_CURATED_EXPANSION.md), [docs/GROWTH_STRUCTURE](docs/GROWTH_STRUCTURE.md), and [docs/MANUAL_CORPUS_ENTRY_GATE](docs/MANUAL_CORPUS_ENTRY_GATE.md)
- scaffold wave and review posture: [docs/IDENTIFIER_DISCIPLINE](docs/IDENTIFIER_DISCIPLINE.md), [docs/SOURCE_NODE_TEMPLATE](docs/SOURCE_NODE_TEMPLATE.md), [docs/CONCEPT_NODE_TEMPLATE](docs/CONCEPT_NODE_TEMPLATE.md), [docs/LINEAGE_NODE_TEMPLATE](docs/LINEAGE_NODE_TEMPLATE.md), [docs/CONTEXT_NODE_TEMPLATE](docs/CONTEXT_NODE_TEMPLATE.md), [docs/TABULAR_BASE_CONTRACT](docs/TABULAR_BASE_CONTRACT.md), [docs/RELATION_PACK_CONTRACT](docs/RELATION_PACK_CONTRACT.md), [docs/REVIEW_CHECKLIST](docs/REVIEW_CHECKLIST.md), `python scripts/validate_tiny_entry_route.py`, and `python scripts/validate_kag_export.py`

For the wider scaffold family, continue through the remaining `*_NODE_TEMPLATE.md` docs in `docs/` after the identifier discipline and first template surfaces.

## How to verify the current route

Use this order:

1. [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) for ownership and non-ownership.
2. `generated/root_entry_map.min.json` and [docs/TINY_ENTRY_ROUTE](docs/TINY_ENTRY_ROUTE.md) for the route shape from `README.md` to capsule, authority, and bounded hop.
3. `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` for the canonical authored source node.
4. `examples/source_node.example.json` and `examples/concept_node.example.json` for the current public compatibility mirrors.
5. `python scripts/build_root_entry_map.py --check`, `python scripts/validate_root_entry_map.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, and `python -m unittest discover -s tests` for the current bounded validator and test battery, then [docs/REVIEW_CHECKLIST](docs/REVIEW_CHECKLIST.md) if your change falls outside that perimeter.

## Route by need

- current canonical authority for the bounded public route: `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` and `tree/concept/becoming/node.json`
- canonical authored tree and registries: `tree/` and `tree/registries/*.csv`
- source witness and provisional intake: `sources/`, `intake/`, and [docs/MANUAL_CORPUS_ENTRY_GATE](docs/MANUAL_CORPUS_ENTRY_GATE.md)
- node-template scaffold family: [docs/SOURCE_NODE_TEMPLATE](docs/SOURCE_NODE_TEMPLATE.md), [docs/CONCEPT_NODE_TEMPLATE](docs/CONCEPT_NODE_TEMPLATE.md), [docs/LINEAGE_NODE_TEMPLATE](docs/LINEAGE_NODE_TEMPLATE.md), [docs/CONTEXT_NODE_TEMPLATE](docs/CONTEXT_NODE_TEMPLATE.md), [docs/PRINCIPLE_NODE_TEMPLATE](docs/PRINCIPLE_NODE_TEMPLATE.md), [docs/EVENT_NODE_TEMPLATE](docs/EVENT_NODE_TEMPLATE.md), [docs/STATE_NODE_TEMPLATE](docs/STATE_NODE_TEMPLATE.md), [docs/SUPPORT_NODE_TEMPLATE](docs/SUPPORT_NODE_TEMPLATE.md), [docs/ANALOGY_NODE_TEMPLATE](docs/ANALOGY_NODE_TEMPLATE.md), and [docs/SYNTHESIS_NODE_TEMPLATE](docs/SYNTHESIS_NODE_TEMPLATE.md)
- bounded public compatibility and export surfaces: [examples/README](examples/README.md), [generated/README](generated/README.md), `generated/kag_export.json`, `generated/kag_export.min.json`, and [docs/KAG_EXPORT](docs/KAG_EXPORT.md)
- review posture and bounded change checks: `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, and `python -m unittest discover -s tests` for the current bounded route, plus [docs/REVIEW_CHECKLIST](docs/REVIEW_CHECKLIST.md) and `docs/reviews/` for broader boundary-sensitive changes outside the current validator perimeter

## What ToS owns

Tree of Sophia is the canonical home for:

- source-first knowledge architecture for the tree
- source discipline, interpretation law, and contributor curation rules
- primary witness and source files that ground canonical authored routes
- candidate intake material that stays visibly provisional
- canonical authored tree nodes, relations, and vocabulary governance
- bounded public entry and export seams that remain subordinate to the canonical tree

## What does not belong here

This repository should not become the main home for:

- runtime, deployment, storage, or service posture
- general agent workflow machinery
- infrastructure configuration
- eval harnesses that are not specifically about ToS knowledge claims
- derived KAG projections presented as authored source truth
- flat note dumps detached from provenance

## Repository layers

The working distinction matters:

- `sources/` grounds authority
- `intake/` prepares candidate structure without becoming authority
- `tree/` is the canonical authored layer
- `examples/` is the current public compatibility seam
- `generated/` stays derived and downstream-facing

Tree for orientation. Graph for relation. Source for authority.

## Go here when...

- you need the operational federation around ToS: [`Agents-of-Abyss`](https://github.com/8Dionysus/Agents-of-Abyss)
- you need the runtime body beneath ToS and AoA: [`abyss-stack`](https://github.com/8Dionysus/abyss-stack)
- you need derived, provenance-aware substrate work built from authoritative sources: [`aoa-kag`](https://github.com/8Dionysus/aoa-kag)
- you need reusable engineering practice for knowledge operations: [`aoa-techniques`](https://github.com/8Dionysus/aoa-techniques)

## Current contour

The current public route is intentionally bounded. It opens one trilingual Zarathustra prologue entry, keeps `README.md` as the public `tos-root`, routes through a source-owned tiny-entry seam, preserves a public compatibility authority surface inside Tree of Sophia, and exposes one downstream-safe KAG export without replacing ToS authority.

The current authored tree also carries the route-local principle, lineage, event, state, support, analogy, and synthesis surfaces needed to make that bounded path reviewable. The immediate task is to prove that route and its review posture before wider corpus movement broadens.

## Guiding axis

ToS is not a static archive. Its guiding axis is a living calibration of meaning: becoming, overcoming, creation of values, and affirmation of life. In the current architecture, *Thus Spoke Zarathustra* acts as a recurring calibration root for that axis.

Use that axis as an interpretive compass, not as permission to flatten sources into one reading.

## License

Apache-2.0
