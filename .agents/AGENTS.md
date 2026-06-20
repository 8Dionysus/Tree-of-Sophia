# AGENTS.md

## Applies to

This card applies to `.agents/` and all descendants unless a nearer
`AGENTS.md` narrows the path.

## Role

`.agents/` holds agent-facing companion lanes for Tree-of-Sophia work:
repo-local skills, Codex Spark guidance, prompts, handoff material, and other
model-facing support surfaces.

Durable ToS meaning, repeatable mechanics, proof authority, memory authority,
runtime state, and public repository entrypoints route to their owning homes.

## Read before editing

Read root `AGENTS.md`, then the nearest lane card. For Codex Spark work, read
`.agents/spark/AGENTS.md` and `.agents/spark/SWARM.md`.

Use ToS source surfaces, mechanics packages, docs, validators, and sibling
owner repositories as stronger authority when the task touches their meaning.

## Boundary Routes

- Keep model-facing launch and handoff guidance under `.agents/`.
- Route philosophical meaning to `ToS/`.
- Route repeatable ToS operation law to `mechanics/`.
- Route proof, memory, runtime, stack, KAG, skill, and playbook authority to
  their owning AoA repositories or layers.
- Route durable doctrine, broad architecture law, and generated truth to their
  canonical owner surfaces before using `.agents/` as launch guidance.

## Validation

Use the narrowest validator named by the touched owner surface. For
agent-route shape changes, run:

```bash
python scripts/validate_nested_agents.py
```
