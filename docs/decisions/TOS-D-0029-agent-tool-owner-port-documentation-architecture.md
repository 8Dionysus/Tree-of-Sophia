# Model-facing skill and owner-port route architecture

## Index Metadata

- Decision ID: TOS-D-0029
- Original date: 2026-08-24
- Surface classes: agents/mesh, agents/skills, owner-ports, docs/route-law, validation
- ToS layers: agents, docs, validation, ports
- Tree classes: agent route, context boundary, generated read model, owner handoff
- Guard families: source authority, progressive disclosure, context budget, owner boundary, currentness, public safety
- Posture: accepted

## Context

Tree-of-Sophia already had strong local contracts: the root and nearest
AGENTS.md route cards, 25 portable skill packages, the Spark lane, and the
evals, stats, KAG, and memo owner-port surfaces. The skill packages also
carried their own references, examples, review checks, executable helpers, and
portable assets. The local owner ports named stronger AoA owners and kept
candidate, reference, generated, live-status, and receipt authority distinct.

The concrete gap was navigation and drift evidence. There was no ToS-owned
map that answered, in one place, who consumes a surface, when it loads, which
owner is canonical, what freshness means, or which organ receives the next
handoff. There was also no narrow guard for frontmatter/activation metadata
parity, broken on-demand companion routes, stale generated currentness, task
probe overlap, or public-safety leakage in a new model-facing map.

A full package inventory measured 25 SKILL.md entrypoints, 25 activation
metadata files, 58 references, 25 examples, 16 checks, 6 scripts, and 56
assets. Discovery descriptions ranged from 45 to 95 whitespace words and
triggered bodies from 433 to 1,811 words, so a bounded progressive-disclosure
guard is practical without rewriting the strong packages.

Repeated reference schemas and logo assets were also observed across portable
packages. Their source owner is aoa-skills, and the copies are package-local
portable support rather than a local documentation authority. That pressure
does not justify deletion or a second shared copy in Tree-of-Sophia.

## Decision

Keep the current AGENTS.md, Spark, skill, and owner-port contracts unchanged.
Add one compact human entrypoint at .agents/README.md and one authored machine
map at .agents/agent-surface.manifest.json. The machine map records:

- discovery, triggered, on-demand, executable, review, asset, MCP/KAG, local
  intake, generated, status, and receipt carrier roles;
- all 25 skills grouped into engineering-core, risk-control, and session-growth
  families, with consumer, load moment, canonical owner, freshness, next organ,
  and negative controls;
- eval, stats, KAG, and memo local-port consumer/owner/currentness routes;
- each port's local owner card is a currentness input, so owner-law drift is
  visible before a handoff is consumed;
- nine representative task probes for authority, context mapping, repo change,
  session diagnosis/repair, approval/dry-run, eval, stats, KAG, and memo;
- a bounded context budget and public-safety boundary;
- a preserve-and-route policy for portable upstream duplication.

Generate .agents/agent-surface.current.json from that map and the local package
and owner-port inputs. It is a subordinate read model containing file
digests, byte and word counts, companion counts, activation metadata,
probe-depth measurements, and declared port input digests. It is not an
authority surface and carries no live runtime, provider, session, secret, or
operator claim.

Add scripts/build_agent_surface_currentness.py as the deterministic builder
and scripts/validate_agent_surface.py as the narrow source/topology validator.
The new blocking agent_surface lane checks discovery parity, metadata policy,
on-demand route closure, owner-port input closure, task-probe negative
controls, public-safe authored surfaces, inventory counts, generated parity,
and the KAG family digest/receipt binding. Release invokes this lane.
Skill-local executable helpers remain advisory and do not become release
authority by being listed in the map.

## Options Considered

- Rewrite all skill descriptions, bodies, and references into a single ToS
  runbook. Rejected because it would erase strong package-local contracts,
  increase always-on context, and move authority away from aoa-skills.
- Deduplicate repeated references and assets in this repository. Rejected
  because package portability is an upstream package concern and local deletion
  would break independent skill exports.
- Add only a prose overview. Rejected because prose alone cannot prove package
  discovery parity, activation metadata, companion routes, or generated
  currentness.
- Add a large cross-owner status registry. Rejected because live status,
  runtime claims, provider internals, receipts, and memory decisions belong to
  their carriers and stronger owners.
- Add a compact authored map, generated structural companion, and narrow
  validator. Chosen because it closes the observed navigation and drift gap
  while preserving source and owner boundaries.

## Rationale

The root and local route-card law already says to name the owner, keep entry
surfaces short, distinguish authored and generated layers, and use the nearest
check. This decision applies that law specifically to the model-facing contour.
The human entrypoint carries selection and handoff meaning; the machine map
carries repeatable topology; the generated file carries currentness evidence;
the validator carries mechanics. None of these surfaces authors ToS
philosophical meaning or stronger AoA owner policy.

The task probes deliberately preserve refusal and return semantics. A local
eval packet is not a verdict, a reference stats packet is not live state, a
KAG provider is not runtime graph authority, a memo candidate is not durable
memory, and a dry-run is not authorization. These are route boundaries, not
claims that the probes execute the downstream owner work.

## Consequences

Agents can choose one skill package or owner port from a single source-owned
map and load the body and companions progressively. Reviewers can see measured
description/body context and explicit task-probe depth. Broken local assets,
metadata drift, stale currentness, missing owner-port inputs, and unsafe new
authored markers fail close to this owner surface.

The source skill packages remain portable and independently readable. Changes
to skill meaning, source metadata, shared reference schemas, or assets must
return to aoa-skills and then refresh this local export. Changes to ToS
philosophical meaning, KAG source exports, eval verdicts, stats questions,
runtime state, or durable memory remain with their named owners.

The KAG generated read model follows the same boundary. Its portable family
manifest and shards are rebuilt by the canonical aoa-kag generator from the
current repository snapshot; a tracked budget receipt carries any exceptional
delta approval. The family digest and receipt are currentness/provenance
carriers, not a second source of agent-route meaning. They are checked in the
KAG lane rather than copied into the agent currentness input set: the portable
family indexes the agent currentness read model itself, so hashing the raw
family from that read model would create an unresolvable self-index cycle.
The agent map therefore declares the generated-family carrier and its exact
canonical builder/validator route alongside authored KAG inputs.

The AGENTS.md documentation line remains a separate later direction. No
AGENTS.md source was changed by this decision.

## Source Surfaces

- .agents/README.md
- .agents/agent-surface.manifest.json
- .agents/skills/*/SKILL.md
- .agents/skills/*/agents/openai.yaml
- .agents/skills/*/references/
- .agents/skills/*/examples/
- .agents/skills/*/checks/
- .agents/skills/*/scripts/
- .agents/skills/*/assets/
- .agents/AGENTS.md
- .agents/spark/AGENTS.md
- .agents/spark/SWARM.md
- evals/PORT.yaml
- stats/port.manifest.json
- kag/manifest.json
- kag/indexes/index_family.manifest.json
- kag/indexes/shards/
- kag/receipts/index_family_budget/<family-digest>.json
- memo/PORT.yaml
- scripts/build_agent_surface_currentness.py
- scripts/validate_agent_surface.py
- docs/validation/validation_lanes.json
- docs/validation/script_inventory.json
- tests/test_agent_surface.py
- tests/test_inventory.json
- DESIGN.AGENTS.md
- TOS-D-0018-codex-spark-agent-lane-home.md
- TOS-D-0028-mechanics-executable-route-map.md

## Validation

Run the agent_surface focused sequence, route-card validation, validation
authority, decision-index generation/check, decision-record validation, and
the release sequence as appropriate. The generated currentness file must be
rebuilt by its builder; it must not be hand-edited. Manual source, semantic,
owner, runtime, transport, and acceptance judgments remain outside this
mechanical guard.
