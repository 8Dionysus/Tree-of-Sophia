# Changelog

All notable changes to `Tree-of-Sophia` will be documented in this file.

The format is intentionally simple and human-first.
Tracking starts with the community-docs baseline for this repository.

## [Unreleased]

## [0.2.0] - 2026-04-10

### Added

- source-owned tiny-entry route validation and reentry doctrine together with a
  zero-entry `generated/root_entry_map.min.json` capsule
- stronger routed-language witness handling through generalized language
  validation and restored schema-level uniqueness
- repo-local project-foundation, session-harvest, and automation-opportunity
  skill surfaces for source-first review and closeout follow-through

### Changed

- tightened AGENTS and review guidance around source-first boundaries,
  validator scope, and the current knowledge-first route posture

## [0.1.0] - 2026-04-01

First public baseline release of `Tree-of-Sophia` as the source-first knowledge architecture repository in the AoA / ToS ecosystem.

This changelog entry uses the release-prep merge date.

### Summary

- first public baseline release of `Tree-of-Sophia` as the canonical home of ToS knowledge architecture and authored tree posture
- the public baseline now includes source, intake, tree, example, and generated export layers around one bounded trilingual Zarathustra entry route
- the release keeps ToS authority source-first while exposing one downstream-safe KAG export and one source-owned tiny-entry seam

### Added

- community-docs baseline established for this repository
- `CHANGELOG.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and `CONTRIBUTING.md`
- `docs/TINY_ENTRY_ROUTE.md` as the first public doctrine note for the tree-first tiny-entry seam
- `schemas/tos-tiny-entry-route.schema.json` and `examples/tos_tiny_entry_route.example.json`
- `docs/reviews/2026-03-23-tree-first-tiny-entry-review.md` for the manual review result of this wave
- bounded witness-provenance fields and optional segment locators in the public multilingual source-node contract
- `docs/reviews/2026-03-25-zarathustra-witness-provenance-and-tiny-hop-review.md`
- `docs/reviews/2026-03-25-zarathustra-pass-001-stabilization-review.md`

### Changed

- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now also serves as the first worked tiny-entry capsule
- `README.md`, `ROADMAP.md`, and `docs/KNOWLEDGE_MODEL.md` now expose the tree-first tiny-entry seam in the public entry path
- `docs/REVIEW_CHECKLIST.md` now includes tiny-entry-specific boundary checks
- tiny-entry doctrine now records the current downstream consumption posture in `aoa-kag` and `aoa-routing` without changing ToS authority boundaries
- `docs/NODE_CONTRACT.md`, `docs/SOURCE_NODE_TEMPLATE.md`, `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `schemas/tos-node-contract.schema.json`, and `examples/source_node.example.json` now keep multilingual witness provenance explicit for the bounded Zarathustra route
- `docs/TINY_ENTRY_ROUTE.md`, `schemas/tos-tiny-entry-route.schema.json`, and `examples/tos_tiny_entry_route.example.json` now prefer `bounded_hop` while keeping `lineage_or_context_hop` as a bounded compatibility alias during transition
- `examples/source_node.example.json` now makes Russian and English maintainer-curated witness posture more explicit and adds a second opening-movement translation-tension note
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now records pass-based stabilization posture and the non-canonical boundary for restartable checkpoint sidecars

### Included in this release

- the current source-first repository layers under `sources/`, `intake/`, `tree/`, `examples/`, and `generated/`
- the bounded Zarathustra route carried by `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `docs/TINY_ENTRY_ROUTE.md`, `examples/source_node.example.json`, and `generated/kag_export.min.json`
- the current canonical tree surface validated as `92` node payloads plus the route-local canonical relation pack

### Validation

- `python scripts/validate_tree_example_sync.py`
- `python scripts/validate_tree_node_contracts.py`
- `python scripts/validate_tree_relation_pack.py`
- `python scripts/validate_intake_pack.py`
- `python scripts/generate_kag_export.py`
- `python scripts/validate_kag_export.py`

### Notes

- this release exposes a bounded public entry path and export seam without weakening ToS source authority
