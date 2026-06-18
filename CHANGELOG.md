# Changelog

All notable changes to `Tree-of-Sophia` will be documented in this file.

The format is intentionally simple and human-first.
Tracking starts with the community-docs baseline for this repository.

## [Unreleased]

### Changed

- compacted root `README.md`, `AGENTS.md`, `DESIGN.md`, `DESIGN.AGENTS.md`,
  `BOUNDARIES.md`, and `ROADMAP.md` into route-oriented owner surfaces while
  keeping the current ToS root-entry and export contracts visible
- relaxed root-doc validation pressure so README and ROADMAP route to owner
  surfaces instead of carrying command inventories or script-path lists
- added explicit validation-lane command authority, script and test inventories,
  and release-check delegation through `docs/validation/validation_lanes.json`
- split Questbook obligation and dispatch validation into its own
  `questbook_surface` lane while keeping KAG export validation focused on the
  bounded export seam
- moved canon and intake checks out of `validate_kag_export.py` and into the
  explicit release lane command sequence
- renamed migration-era Experience boundary batch tests by the mechanic
  contracts they protect
- moved Experience boundary contract tests into an explicit mechanics-local
  `experience_contracts` validation lane
- reduced route-card and tiny-entry validators toward structural route tokens
  instead of ordinary prose inventories

## [0.2.2] - 2026-04-23

### Summary

- this patch lands Agon Sophian threshold intake posture, ToS candidate
  dossier review, canon-restraint checks, rejection/branching posture, and
  follow-up guards while keeping ToS on source-owned interpretation and intake
  boundaries
- Experience wave3-wave5 boundary contracts are planted for adoption,
  governance, installation, service dossiers, runtime stop-lines, and
  sovereign office intake without allowing direct arena, runtime, or assistant
  office writes into the tree
- `Tree-of-Sophia` remains the source-first knowledge authority and does not
  turn installation, governance, runtime, or assistant-office artifacts into
  direct authored tree writes

### Added

- Agon Sophian threshold intake registry surfaces, ToS candidate dossier
  review notes, pattern review notes, canon-restraint guidance, and
  rejection/branching boundaries
- wave3 Experience adoption and no-runtime-adoption boundary dossiers, wave4
  governance and constitution-runtime no-direct-write guards, and wave5
  installation, service, and sovereign-office intake boundaries
- governance, installation, service, and sovereign-office dossier contracts
  with explicit decision-reference requirements

### Changed

- schema and contract guards now enforce stronger no-direct-write,
  no-runtime-adoption, no-runtime-office-write, and governance decision-ref
  stop-lines
- post-merge review guards are tightened around Experience boundary surfaces
  so ToS can receive dossiers without becoming a runtime adoption mechanism

### Validation

- `python scripts/release_check.py`

### Notes

- this patch is a source-boundary release for ToS; runtime, release
  certification, and office-operation authority remain in their owning AoA
  repositories

## [0.2.1] - 2026-04-19

### Summary

- this patch tightens release posture, roadmap routing, and current-direction
  locks around the active root-entry line
- README front-page, release-audit baseline, PR template coverage, and
  required-check plus Node24 CI surfaces now agree
- `Tree-of-Sophia` remains the source-first authority while keeping
  release-facing entry surfaces bounded and legible

### Added

- a release-audit baseline, current-direction route locks, and a GitHub pull
  request template for the repository

### Changed

- roadmap entry-routing docs, README release front page, changelog contract,
  and CI/protection refs are aligned with the current root-entry contour

### Validation

- `python scripts/release_check.py`

### Notes

- this patch is release-contract and route-legibility work for the source
  repository; it does not widen Tree-of-Sophia ownership boundaries

## [0.2.0] - 2026-04-10

### Summary

- this release adds a source-owned zero-entry root capsule, reentry doctrine, stronger routed-language validation, and repo-local follow-through skill surfaces
- witness handling, schema-level uniqueness, and source-first review guidance are tightened across the current trilingual tree contour
- `Tree-of-Sophia` remains source-first authority while exposing stronger downstream-safe entry and export seams

### Validation

- `python scripts/release_check.py`

### Notes

- detailed route, corpus, and generated-surface coverage for this release remains enumerated below under `Added`, `Changed`, and `Included in this release`

### Added

- source-owned tiny-entry route validation and reentry doctrine together with a
  zero-entry `ToS/derived-exports/root_entry_map.min.json` capsule
- stronger routed-language witness handling through generalized language
  validation and restored schema-level uniqueness
- repo-local project-foundation, session-harvest, and automation-opportunity
  skill surfaces for source-first review and closeout follow-through

### Changed

- tightened AGENTS and review guidance around source-first boundaries,
  validator scope, and the current knowledge-first route posture

### Included in this release

- root-route and contributor-posture refreshes across `README.md`,
  `BOUNDARIES.md`, `ROADMAP.md`, `AGENTS.md`, `.agents/`, `.github/`, and
  `docs/`, including public-route clarity, review guidance, and follow-through
  installs
- source, intake, and tree route artifacts across `ToS/source-witnesses/`, `ToS/candidate-intake/`,
  `ToS/canon/`, `ToS/public-compatibility/`, `ToS/derived-exports/`, `ToS/contracts/`, `scripts/`, `tests/`, and
  `Spark/`, including routed-language witness handling and the zero-entry root
  capsule

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
- `ToS/doctrine/TINY_ENTRY_ROUTE.md` as the first public doctrine note for the tree-first tiny-entry seam
- `ToS/contracts/tos-tiny-entry-route.schema.json` and `ToS/public-compatibility/tos_tiny_entry_route.example.json`
- `ToS/review-ledger/2026-03-23-tree-first-tiny-entry-review.md` for the manual review result of this phase
- bounded witness-provenance fields and optional segment locators in the public multilingual source-node contract
- `ToS/review-ledger/2026-03-25-zarathustra-witness-provenance-and-tiny-hop-review.md`
- `ToS/review-ledger/2026-03-25-zarathustra-pass-001-stabilization-review.md`

### Changed

- `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now also serves as the first worked tiny-entry capsule
- `README.md`, `ROADMAP.md`, and `ToS/doctrine/KNOWLEDGE_MODEL.md` now expose the tree-first tiny-entry seam in the public entry path
- `ToS/doctrine/REVIEW_CHECKLIST.md` now includes tiny-entry-specific boundary checks
- tiny-entry doctrine now records the current downstream consumption posture in `aoa-kag` and `aoa-routing` without changing ToS authority boundaries
- `ToS/doctrine/NODE_CONTRACT.md`, `ToS/doctrine/SOURCE_NODE_TEMPLATE.md`, `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `ToS/contracts/tos-node-contract.schema.json`, and `ToS/public-compatibility/source_node.example.json` now keep multilingual witness provenance explicit for the bounded Zarathustra route
- `ToS/doctrine/TINY_ENTRY_ROUTE.md`, `ToS/contracts/tos-tiny-entry-route.schema.json`, and `ToS/public-compatibility/tos_tiny_entry_route.example.json` now prefer `bounded_hop` while keeping `lineage_or_context_hop` as a bounded compatibility alias during transition
- `ToS/public-compatibility/source_node.example.json` now makes Russian and English maintainer-curated witness posture more explicit and adds a second opening-movement translation-tension note
- `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md` now records pass-based stabilization posture and the non-canonical boundary for restartable checkpoint sidecars

### Included in this release

- the current source-first repository layers under `ToS/source-witnesses/`, `ToS/candidate-intake/`, `ToS/canon/`, `ToS/public-compatibility/`, and `ToS/derived-exports/`
- the bounded Zarathustra route carried by `ToS/doctrine/ZARATHUSTRA_TRILINGUAL_ENTRY.md`, `ToS/doctrine/TINY_ENTRY_ROUTE.md`, `ToS/public-compatibility/source_node.example.json`, and `ToS/derived-exports/kag_export.min.json`
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
