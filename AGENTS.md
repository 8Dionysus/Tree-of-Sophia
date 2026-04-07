# AGENTS.md

Guidance for coding agents and humans contributing to `Tree-of-Sophia`.

## Purpose

`Tree-of-Sophia` is the canonical high-level statement of ToS and the home of its canonical authored tree. Treat it as the place that defines the knowledge architecture, preserves source-facing inputs, and carries canonical authored tree surfaces.

Preserve the distinction between authored meaning, derived structure, and operational support at all times.

## Owns

This repository is the source of truth for:

- source-first knowledge architecture
- source discipline and interpretation law
- primary witness and source files that ground authored routes
- candidate intake material that remains visibly provisional
- canonical authored tree nodes, relations, and vocabulary governance
- public compatibility and downstream-safe export seams that remain subordinate to the canonical tree

## Does not own

Do not treat this repository as the main home for:

- runtime, deployment, storage, or service posture in `abyss-stack`
- operational federation rules in `Agents-of-Abyss`
- derived substrate semantics in `aoa-kag`
- general agent workflow machinery
- infrastructure configuration
- flat note archives detached from provenance

If the task mainly concerns runtime, orchestration, or derived substrate mechanics, route there instead of forcing it into ToS.

## Core rule

Sources before abstraction.

Keep these layers visibly distinct whenever you touch them:

- raw source text or fragment
- raw candidate extraction
- canonical authored tree surface
- compatibility or public entry surface
- interpretation or synthesis
- lineage, temporal, and civilizational context

Do not flatten those layers into one undifferentiated summary.

The guiding axis of becoming, overcoming, value creation, and life-affirmation is an interpretive compass. It is not permission to distort sources or force every text into one frame.

## Read this first

Before making changes, read in this order:

1. `README.md`
2. `CHARTER.md` and `BOUNDARIES.md`
3. `docs/KNOWLEDGE_MODEL.md` and `docs/NODE_CONTRACT.md`
4. the specific template, review, or route docs that govern the surface you are changing
5. the target source file you plan to edit
6. any affected export or example surfaces if the task touches `examples/` or `generated/`

If you are editing inside `docs/`, `examples/`, `generated/`, `intake/`, `schemas/`, `scripts/`, `sources/`, or `tree/`, also follow the nested `AGENTS.md` in that directory.

## Primary objects

The most important objects in this repository are:

- source files under `sources/`
- candidate intake material under `intake/`
- canonical authored nodes and relation-bearing surfaces under `tree/`
- knowledge-model, node-contract, and review docs
- bounded public entry and export seams under `examples/` and `generated/`

## Hard NO

Do not:

- replace source-grounded thought with generic AI paraphrase
- collapse extraction, interpretation, and synthesis into one layer
- present derived exports as authored authority
- move runtime or orchestration doctrine here
- erase provenance, temporal context, or lineage when they matter
- introduce private or hidden material into public surfaces

## Contribution doctrine

Use this flow: `PLAN -> DIFF -> VERIFY -> REPORT`

### PLAN

State:

- what ToS surface is changing
- which layer is affected: source, intake, tree, compatibility, or export
- what provenance or interpretation risk exists
- which neighboring repositories may be affected

### DIFF

Keep the change focused. Prefer the smallest reversible structure that preserves fidelity, legibility, and provenance.

### VERIFY

Confirm that:

- source traceability is still visible
- extraction, interpretation, and synthesis remain distinguishable
- lineage and context still attach where relevant
- derived surfaces are still clearly derived
- no wording overclaims authority beyond what the layer can honestly support

### REPORT

Summarize:

- what surfaces changed
- whether semantics changed or only metadata / organization changed
- whether provenance, lineage, or interpretation posture changed
- what validation was run
- any neighboring repo follow-up likely needed

## Validation

The current bounded read-only battery is:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
python -m unittest discover -s tests
```

`python scripts/validate_tiny_entry_route.py` keeps the current source-owned `tos-root` handoff, public compatibility authority vocabulary, and bounded re-entry posture explicit.
`python scripts/validate_kag_export.py` also checks nested local guidance in `docs/`, `examples/`, `generated/`, `intake/`, `schemas/`, `scripts/`, `sources/`, and `tree/`.

When the task falls outside that narrow export seam:

- use `docs/REVIEW_CHECKLIST.md` as the default manual review route
- if you change canonical tree mirrors, run `python scripts/validate_tree_example_sync.py`
- if you change the current tiny-entry route, run `python scripts/validate_tiny_entry_route.py`
- if you change export inputs or generation logic, run `python scripts/generate_kag_export.py`, then `python scripts/validate_tiny_entry_route.py`, then `python scripts/validate_kag_export.py`, and then `python -m unittest discover -s tests`

Do not claim checks you did not run.
