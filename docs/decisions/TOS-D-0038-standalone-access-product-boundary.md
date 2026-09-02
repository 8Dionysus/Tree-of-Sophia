# Standalone Access Is A ToS Product Boundary

## Index Metadata

- Decision ID: TOS-D-0038
- Original date: 2026-09-01
- Surface classes: runtime/access, contracts/portability, docs/route-law
- ToS layers: derived-exports, docs
- Tree classes: none
- Guard families: source-first authority, runtime ownership, portability, artifact allowlist
- Posture: accepted

## Context

Tree of Sophia already owns authored sources, review, canon, builders, and
source-returning derived projections. Native MCP and the constellation site
were implemented only in abyss-stack, however, so a ToS checkout could
validate its source repository but could not install and run its own access
product without sibling repositories and host-specific paths.

The split also produced live compatibility drift: the current ToS philosophy
graph projection is v2 and stores one global node/edge set with per-view
membership IDs, while the stack-owned MCP and site readers still require the
older inline v1 shape. Runtime ownership had become coupled to one downstream
deployment rather than to the projection producer's portable product seam.

## Decision

Tree of Sophia owns a new top-level access/ product boundary. It contains a
portable read/query core and read-only CLI, HTTP, native MCP, web, browser
action, profile, and packaging adapters over allowlisted ToS derived exports.
Standalone installation and loopback runtime require no AbyssOS repository or
service.

abyss-stack remains the owner of AbyssOS deployment, service composition,
Neo4j operation, and ecosystem orchestration. Its integration is an optional
adapter profile, not a dependency of the standalone profile. aoa-kag,
aoa-stats, and abyss-machine retain their existing substrate, grammar,
artifact-policy, registry, and admission authority.

Existing projection v1/v2 fields that name runtime_owner: abyss-stack remain
unchanged for consumer compatibility. The consumer-neutral ownership contract
is access/contracts/runtime-manifest.v1.json: ToS owns source authority and
the standalone runtime; abyss-stack owns only its optional deployment profile.
A future projection contract may encode that distinction directly, but this
decision does not break the current export ABI.

## Options Considered

- Keep all MCP, site, and WebMCP work in abyss-stack. Rejected because ToS
  would remain non-installable and projection compatibility would lag its
  source owner.
- Copy the stack services into ToS without a shared core. Rejected because CLI,
  native MCP, HTTP, and WebMCP would acquire divergent query rules.
- Move ecosystem orchestration, KAG, stats, Neo4j, and artifact policy into
  ToS. Rejected because that imports sibling-owner authority and makes the
  standalone product heavier rather than autonomous.
- Add a portable ToS product core with optional ecosystem adapters. Accepted
  because it preserves source authority, removes installation coupling, and
  gives WebMCP one stable action seam.

## Consequences

access/src/tos_access/core.py becomes the reusable read center. CLI, HTTP,
native MCP, browser actions, and later WebMCP must call it rather than
reimplement projection semantics. The core accepts current graph projection
v1 and v2, including v2 view membership materialization.

The standalone bundle is allowlist-only and must fingerprint every included
subject. Restricted payloads and the dictionary-recoverable lexical projection
remain excluded. A successful build or archive smoke is only producer evidence:
it does not publish, register, sign, or admit the artifact for an installer or
runtime consumer.

WebMCP registration is deliberately the next slice. The current decision lands
its page-action ABI and self-contained site/runtime so WebMCP can be developed
without reopening packaging, data discovery, or core-query ownership.

## Source Surfaces

- BOUNDARIES.md
- DESIGN.md
- ROADMAP.md
- access/AGENTS.md
- access/contracts/runtime-manifest.v1.json
- access/contracts/runtime-data.v1.json
- access/contracts/web-actions.v1.json
- ToS/derived-exports/philosophy_graph_projection.min.json
- ToS/derived-exports/tos_corpus_index.min.json

## Validation

Run the access unit, frontend typecheck/build, source validator, deterministic
bundle build, and archive smoke. Regenerate and validate the decision indexes,
then run the affected root route and broad repository checks. Preserve the
claim boundary between producer validation and abyss-machine consumer
admission.
