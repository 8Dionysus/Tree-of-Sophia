# Tree of Sophia (ToS)

Tree of Sophia is a source-first living knowledge architecture for philosophy and world thought. It traces texts, concepts, contexts, and lineages across time and cultures while keeping source-linked authority visible as the tree grows.

It is not just a notes repository, not just a graph, and not just a retrieval substrate. It is the architectural root where source-linked meaning should stay legible as the wider ecosystem grows around it.

This repository currently carries four public layers:

- `ToS/source-witnesses/` for primary witness and source material
- `ToS/candidate-intake/` for candidate structure that stays visibly provisional
- `ToS/canon/` for canonical authored nodes, relations, and vocabulary governance
- `ToS/public-compatibility/` and `ToS/derived-exports/` for bounded public compatibility and downstream-safe export seams

> Current release: `v0.2.2`. See [CHANGELOG](CHANGELOG.md) for release notes.

## Start here

Use the shortest route by need:

- if you are new here and want the one real current public route: [ToS/doctrine/TINY_ENTRY_ROUTE](ToS/doctrine/TINY_ENTRY_ROUTE.md) and [ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY](ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md)
- if you want the compact machine-facing companion to that same root path: `ToS/derived-exports/root_entry_map.min.json`
- if you need the bounded downstream export seam for that route: [ToS/doctrine/KAG_EXPORT](ToS/doctrine/KAG_EXPORT.md)
- if you want to verify the current bounded route: [CHARTER](CHARTER.md), [BOUNDARIES](BOUNDARIES.md), `python scripts/build_root_entry_map.py --check`, `python scripts/validate_root_entry_map.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, `python -m unittest discover -s tests`, and [ToS/doctrine/REVIEW_CHECKLIST](ToS/doctrine/REVIEW_CHECKLIST.md) for surfaces outside the current validator perimeter
- mission and source-of-truth boundaries: [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md)
- knowledge model and interpretation law: [ToS/doctrine/KNOWLEDGE_MODEL](ToS/doctrine/KNOWLEDGE_MODEL.md) and [ToS/doctrine/NODE_CONTRACT](ToS/doctrine/NODE_CONTRACT.md)
- current direction: [ROADMAP](ROADMAP.md)
- durable route, boundary, validator, or export rationale: [docs/decisions](docs/decisions/README.md), `python scripts/generate_decision_indexes.py --check`, and `python scripts/validate_decision_records.py`
- growth law and curation posture: [ToS/doctrine/CONTEXT_COMPOST](ToS/doctrine/CONTEXT_COMPOST.md), [ToS/doctrine/CALIBRATION_AXIS](ToS/doctrine/CALIBRATION_AXIS.md), [ToS/doctrine/HUMAN_CURATED_EXPANSION](ToS/doctrine/HUMAN_CURATED_EXPANSION.md), [ToS/doctrine/GROWTH_STRUCTURE](ToS/doctrine/GROWTH_STRUCTURE.md), and [ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE](ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE.md)
- scaffold wave and review posture: [ToS/doctrine/IDENTIFIER_DISCIPLINE](ToS/doctrine/IDENTIFIER_DISCIPLINE.md), [ToS/doctrine/SOURCE_NODE_TEMPLATE](ToS/doctrine/SOURCE_NODE_TEMPLATE.md), [ToS/doctrine/CONCEPT_NODE_TEMPLATE](ToS/doctrine/CONCEPT_NODE_TEMPLATE.md), [ToS/doctrine/LINEAGE_NODE_TEMPLATE](ToS/doctrine/LINEAGE_NODE_TEMPLATE.md), [ToS/doctrine/CONTEXT_NODE_TEMPLATE](ToS/doctrine/CONTEXT_NODE_TEMPLATE.md), [ToS/doctrine/TABULAR_BASE_CONTRACT](ToS/doctrine/TABULAR_BASE_CONTRACT.md), [ToS/doctrine/RELATION_PACK_CONTRACT](ToS/doctrine/RELATION_PACK_CONTRACT.md), [ToS/doctrine/REVIEW_CHECKLIST](ToS/doctrine/REVIEW_CHECKLIST.md), `python scripts/validate_tiny_entry_route.py`, and `python scripts/validate_kag_export.py`

For the wider scaffold family, continue through the remaining `*_NODE_TEMPLATE.md` docs in `docs/` after the identifier discipline and first template surfaces.

## How to verify the current route

Use this order:

1. [CHARTER](CHARTER.md) and [BOUNDARIES](BOUNDARIES.md) for ownership and non-ownership.
2. `ToS/derived-exports/root_entry_map.min.json` and [ToS/doctrine/TINY_ENTRY_ROUTE](ToS/doctrine/TINY_ENTRY_ROUTE.md) for the route shape from `README.md` to capsule, authority, and bounded hop.
3. `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` for the canonical authored source node.
4. `ToS/public-compatibility/source_node.example.json` and `ToS/public-compatibility/concept_node.example.json` for the current public compatibility mirrors.
5. `python scripts/build_root_entry_map.py --check`, `python scripts/validate_root_entry_map.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, and `python -m unittest discover -s tests` for the current bounded validator and test battery, then [ToS/doctrine/REVIEW_CHECKLIST](ToS/doctrine/REVIEW_CHECKLIST.md) if your change falls outside that perimeter.

## Route by need

- current canonical authority for the bounded public route: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` and `ToS/canon/concept/becoming/node.json`
- canonical authored tree and registries: `ToS/canon/` and `ToS/canon/registries/*.csv`
- source witness and provisional intake: `ToS/source-witnesses/`, `ToS/candidate-intake/`, and [ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE](ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE.md)
- node-template scaffold family: [ToS/doctrine/SOURCE_NODE_TEMPLATE](ToS/doctrine/SOURCE_NODE_TEMPLATE.md), [ToS/doctrine/CONCEPT_NODE_TEMPLATE](ToS/doctrine/CONCEPT_NODE_TEMPLATE.md), [ToS/doctrine/LINEAGE_NODE_TEMPLATE](ToS/doctrine/LINEAGE_NODE_TEMPLATE.md), [ToS/doctrine/CONTEXT_NODE_TEMPLATE](ToS/doctrine/CONTEXT_NODE_TEMPLATE.md), [ToS/doctrine/PRINCIPLE_NODE_TEMPLATE](ToS/doctrine/PRINCIPLE_NODE_TEMPLATE.md), [ToS/doctrine/EVENT_NODE_TEMPLATE](ToS/doctrine/EVENT_NODE_TEMPLATE.md), [ToS/doctrine/STATE_NODE_TEMPLATE](ToS/doctrine/STATE_NODE_TEMPLATE.md), [ToS/doctrine/SUPPORT_NODE_TEMPLATE](ToS/doctrine/SUPPORT_NODE_TEMPLATE.md), [ToS/doctrine/ANALOGY_NODE_TEMPLATE](ToS/doctrine/ANALOGY_NODE_TEMPLATE.md), and [ToS/doctrine/SYNTHESIS_NODE_TEMPLATE](ToS/doctrine/SYNTHESIS_NODE_TEMPLATE.md)
- bounded public compatibility and export surfaces: [ToS/public-compatibility/README](ToS/public-compatibility/README.md), [ToS/derived-exports/README](ToS/derived-exports/README.md), `ToS/derived-exports/kag_export.json`, `ToS/derived-exports/kag_export.min.json`, and [ToS/doctrine/KAG_EXPORT](ToS/doctrine/KAG_EXPORT.md)
- review posture and bounded change checks: `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py`, and `python -m unittest discover -s tests` for the current bounded route, plus [ToS/doctrine/REVIEW_CHECKLIST](ToS/doctrine/REVIEW_CHECKLIST.md) and `ToS/review-ledger/` for broader boundary-sensitive changes outside the current validator perimeter
- durable rationale lookup: [docs/decisions](docs/decisions/README.md) and generated decision indexes under `docs/decisions/indexes/`

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

- `ToS/source-witnesses/` grounds authority
- `ToS/candidate-intake/` prepares candidate structure without becoming authority
- `ToS/canon/` is the canonical authored layer
- `ToS/public-compatibility/` is the current public compatibility seam
- `ToS/derived-exports/` stays derived and downstream-facing

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
