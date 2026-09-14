# Overview proximity excludes record production — 2026-09-07

## Reviewed boundary

Parent `8e5b5e1d500177377e4d789cbe3cc672bb4f7b18`. The read-only access
`overview` profile now excludes the exact semantic relation types
`tos.relation.made-by` and `tos.relation.generated-by` in addition to its
existing dense text-unit/anchor exclusions. The source registry defines those
types as Claim-to-maker and Claim-to-provenance links; neither establishes
historical or philosophical proximity between the subjects. Native spelling
alone does not classify an unknown relation.

No source assertion, graph carrier, evidence, identity or review is removed.
The `all` profile and exact inspection retain record-production edges. Query
path conditions remain explicit caller requests. A technical center may need
`all` to inspect its technical neighborhood; catalog/API discovery states the
rule. This does not infer agreement or common evidence from shared production.

Checklist: source traceability, source/derived separation, uncertainty,
non-promotion and owner boundaries **yes**. New rights, interpretation, canon,
consent and publication judgments **not applicable**. This is access-profile
behavior, not a new source ontology or UI redesign.

## Checks and observed scope

- A synthetic shared-maker traversal failed in Python and Worker/D1 before
  implementation, then passed; `all` retains reachability and an unknown type
  with the same native spelling is not suppressed.
- Python knowledge contracts: 52 tests passed, 22.174 seconds. Python
  exploration: 11 tests passed, 1.581 seconds.
- Worker knowledge/exploration: 15 tests passed, 29.616 seconds. The suite
  includes indexed D1 parity, actual local Worker HTTP, isolate restart,
  concurrent replay, snapshot publication conflicts and bounded cache
  admission. Typecheck passed. These are local Miniflare checks, not deployed
  Cloudflare or remote D1 health.
- Actual current corpus, `ToSAccessCore.knowledge_focus`, depth 2: Claim
  `tos.claim.nietzsche-letter-705.addressee` returns 9 carriers / 7 distinct
  entity IDs / 11 relations in overview, with no excluded technical edges.
  `all` returns its 200-carrier limit / 198 entities / 205 relations, including
  194 technical edges. Same process: cold graph 18.406 s; overview focus
  0.2907 s, full focus 0.4453 s. This is one local observation, not p95 or a
  scaling budget claim; tests ran concurrently and CPU/RSS were not measured.

## Still open

Naumann focus still returns 3 carriers / 2 entities and Letter focus 6 / 5:
the source-navigation/Claim-carrier boundary still consumes a traversal hop
and produces visual duplicates. The existing observatory uses these opaque
carrier IDs; silently discarding one would lose adjacency and inspection
history. A shared backend presentation contract must supply one scene identity
with explicit constituent carriers and compact Claim paths, while preserving
technical inspection and exact source returns.

No aesthetic, camera, motion, gesture or scene-owner code changed here. Full
Foundation v1, real UI acceptance, indexed scaling, CI and landing remain open.
Reverting this access change restores the previous profile behavior without
rolling back any authored knowledge. Exploration execution v2 prevents reuse
of pre-change D1 checkpoints; v1 result packets remain readable as historical
responses, not resumable execution state.
