# TOS-D-0032 cross-corpus documentation map and minimal projection

## Index Metadata

- Decision ID: TOS-D-0032
- Original date: 2026-08-24
- Surface classes: documentation/families, validation/currentness, generated/carrier, agent-routing
- ToS layers: docs, agents, validation, derived exports
- Tree classes: owner route, context boundary, generated read model, external handoff
- Guard families: family coverage, link closure, executable currentness, authority conflict, generated parity, public safety, context budget
- Posture: accepted

## Context

The PR #150 atlas established a reproducible tracked-documentation method and
counted 693 human-readable surfaces. PRs #151-#159 then strengthened ToS,
mechanics, agent-surface, and AGENTS-route owners independently. Those owners
already have good source maps, builders, validators, and generated companions,
but no source-owned contract joined their families or measured cross-family
currentness. A family map was therefore needed to expose the handoff without
making a generated index, runtime receipt, KAG record, or local port a second
source owner.

At the fresh post-#159 head, the same atlas method counted 698 human-readable
surfaces: five net additions, with route-card and script-card compaction
offsetting growth elsewhere. After this bounded change the head counts 699
human-readable surfaces. The final carrier covers 1,849 tracked textual,
structured, and executable route surfaces. It considers 2,141 tracked paths
after the three explicit generated carrier exclusions; 292 paths outside the
declared extension set remain outside the machine projection by explicit
extension rule.

The repository was also checked for `llms.txt`, `llms.md`, loader references,
workflow references, and manifest consumers. No real loader exists. README,
AGENTS, `.agents/agent-surface.manifest.json`, public-entry, and `kag/manifest.json`
already own the active human, contributor, task-routing, skill, public, and
KAG/MCP seams.

## Decision

Add `docs/validation/documentation_family_map.json` as the authored compact
family law. It defines 15 stable atlas family IDs and, for each family, its
consumer, loading moment, owner, authored/generated/executable/runtime/tool/
receipt roles, currentness inputs, next organ, context posture, public-safety
boundary, and validation lane. It also declares stronger external handles and
the seven cross-corpus guard families.

Generate `docs/validation/documentation-family.current.json` with
`scripts/build_documentation_family_currentness.py`. The generated carrier is
derived only from `git ls-files`, the authored map, and current file bytes. It
contains exact tracked textual/structured/executable paths, family membership,
content hashes, family summaries, atlas counts, and an explicit unhandled-family
count. It is on demand and machine-readable; it is not an always-on prompt,
public source entry, runtime status surface, KAG authority, or acceptance
receipt.

Use `scripts/validate_documentation_cross_corpus.py` as a coordinating guard.
It reuses the existing link/fragment, mechanics topology, decision,
AGENTS-route, agent-surface, KAG, and public-entry contracts, then adds focused
cross-family checks for broken routes, stale local executables, explicit
authority declaration conflicts, generated mismatch, missing coverage, public
host/session/provider markers, and selected context budgets. Repeated words in
prose are not treated as authority conflicts; only duplicate or conflicting
structured declarations are admitted to that guard.

Do not add `llms.txt` or another public projection until a real consumer and
owner contract exists. The minimal added projection is the internal currentness
carrier because its validator and machine readers have a concrete route need;
the no-add decision for `llms.txt` is durable here and in the authored map.

## Options Considered

- Rewrite README, root AGENTS, or `DESIGN.md` into a universal inventory.
  Rejected: those are strong source/architecture surfaces and the reproduced
  pressure is cross-family coordination, not missing prose.
- Add `llms.txt` speculatively. Rejected: no tracked or CI/manifest consumer
  would load it, so it would create a second entry seam without route value.
- Add only a prose family table. Rejected: exact Git currentness, missing
  coverage, generated parity, and context guards need repeatable evidence.
- Let `.agents/agent-surface.current.json` or KAG indexes absorb the full map.
  Rejected: those projections have narrower owners and would conflate skill,
  KAG, or runtime carriers with repository-wide documentation law.
- Add an authored map, generated exact carrier, and coordinating validator.
  Chosen: it closes the observed cross-family gap while preserving every mature
  owner surface and the source/generated/runtime/receipt claim boundaries.

## Consequences

Contributors changing a family route can inspect one compact map and run one
coordinator in addition to the nearest owner lane. The generated carrier is
larger than an always-on entry and must stay summary-first/on demand. New
top-level route families require an explicit stable ID, owner, currentness
inputs, validation lane, and map review rather than silently falling into a
catch-all.

The coordinator intentionally does not claim semantic source acceptance,
translation quality, rights clearance, canon promotion, runtime health, CI
success, external-owner acceptance, or terminal closure. Those claims remain
with their source, owner, GitHub, runtime, and receipt authorities.

## Source Surfaces

- `docs/validation/documentation_family_map.json`
- `docs/validation/documentation-family.current.json`
- `docs/validation/documentation-family-map.schema.json`
- `scripts/build_documentation_family_currentness.py`
- `scripts/validate_documentation_cross_corpus.py`
- `docs/validation/validation_lanes.json`
- `docs/validation/README.md`
- `README.md`
- `.agents/README.md`
- `kag/manifest.json`
- `docs/decisions/TOS-D-0031-agents-route-topology-and-progressive-disclosure.md`

## Validation

Run the focused family builder/check and coordinator, then the affected
AGENTS-route, agent-surface, mechanics, public-entry, KAG, decision-index, and
release lanes. Rebuild decision indexes after this record changes. The family
carrier remains subordinate to all named source, owner, runtime, and receipt
surfaces.
