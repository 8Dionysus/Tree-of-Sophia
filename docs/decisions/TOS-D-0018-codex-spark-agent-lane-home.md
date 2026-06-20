# Codex Spark Agent Lane Home

## Index Metadata

- Decision ID: TOS-D-0018
- Original date: 2026-06-20
- Surface classes: agents/mesh, root/topology, docs/route-law
- ToS layers: agents, docs, root
- Tree classes: agent lane, root topology
- Guard families: owner boundary, source-first authority, agent-lane placement, root convexity
- Posture: accepted

## Context

Tree-of-Sophia had a root `Spark/` directory for GPT-5.3-Codex-Spark work.
That was useful as an early fast-loop lane, but root placement made Spark look
like a public repository organ beside `ToS/`, `mechanics/`, `docs/`, `evals/`,
and `memo/`.

OpenAI's current Codex documentation describes GPT-5.3-Codex-Spark as a fast,
less-capable Codex model for near-instant, real-time coding iteration. It is a
model and workflow lane, not a source, doctrine, canon, proof, memory, or
runtime owner.

Deeply refactored sibling repositories converged on the same placement:
maintained Spark lanes live under `.agents/spark/`, while root `Spark/` is an
earlier transitional shape.

## Decision

Move the maintained Spark lane from root `Spark/` to:

```text
.agents/spark/
```

Add `.agents/AGENTS.md` as the agent-facing companion district card. Keep
`.agents/skills/` for repo-local skills and `.agents/spark/` for the Codex
Spark fast-loop lane.

Keep the initial lane light. Tree-of-Sophia currently needs a bounded Spark
lane for source-traceability cleanup, small route repairs, and micro-patches.
Registry, result, handoff, schema, validator, and test surfaces become
appropriate only when repeated ToS Spark work needs a machine-checkable
scenario contract.

## Options Considered

- Keep root `Spark/`. This preserves the early transitional shape, but leaves an
  agent-facing lane in the public root and makes the root less convex.
- Delete Spark. This removes root noise, but loses a useful lane for fast,
  bounded ToS micro-patches.
- Move Spark to `.agents/spark/` without registry-backed scenarios. This keeps
  the lane available while matching the mature agent-lane placement used by
  deeper sibling refactors.
- Copy the full registry-backed sibling pattern immediately. This would add
  machinery before ToS has repeated Spark scenarios that justify it.

## Rationale

Spark is model-facing work guidance. It helps a fast Codex model stay scoped,
produce small edits, and hand off when the work needs slower architecture or
human review. That makes `.agents/spark/` the right home.

ToS source authority stays in `ToS/`; repeatable operation law stays in
`mechanics/`; durable rationale stays in `docs/decisions/`; proof, memory,
runtime, KAG, skill, and playbook authority stay with their owning AoA
repositories or layers.

The chosen route gives ToS appropriate symmetry with deeply refactored sibling
repositories without copying their heavier registry-backed lane before ToS
needs it.

## Consequences

Root no longer has a standalone `Spark/` directory.

Spark references for current work use `.agents/spark/`. Historical references
to root `Spark/` in release notes remain receipts, not active topology.

Future Spark-lane growth must stay under `.agents/spark/`. If Spark scenarios
become repeated enough to need registry-backed launch, result, handoff,
schemas, validators, or tests, add those under `.agents/spark/` with a
follow-up decision or clear release-facing change note.

This decision keeps doctrine, mechanics, memory, proof, runtime, and public
root authority in their existing owner surfaces.

## Source Surfaces

- `.agents/AGENTS.md`
- `.agents/spark/AGENTS.md`
- `.agents/spark/SWARM.md`
- `README.md`
- `scripts/validate_nested_agents.py`
- `CHANGELOG.md`
- `https://developers.openai.com/codex/speed`
- `https://developers.openai.com/codex/models`
- `https://openai.com/index/introducing-gpt-5-3-codex-spark/`
- sibling decisions:
  - `/srv/AbyssOS/aoa-skills/docs/decisions/AOA-SK-D-0022-codex-spark-agent-lane-home.md`
  - `/srv/AbyssOS/aoa-memo/docs/decisions/AOA-MEM-D-0039-spark-agent-lane-home.md`
  - `/srv/AbyssOS/aoa-evals/docs/decisions/AOA-EV-D-0017-spark-agent-lane-placement.md`

## Validation

Run:

```bash
python scripts/validate_nested_agents.py
python scripts/validate_active_naming.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
