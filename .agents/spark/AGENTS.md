# AGENTS.md

## Applies to

This card applies to `.agents/spark/` and all descendants unless a nearer
`AGENTS.md` narrows the path.

The root `AGENTS.md` remains authoritative for repository identity, ownership
boundaries, reading order, and validation commands. This local file only
narrows how GPT-5.3-Codex-Spark should behave when used as the fast-loop lane.

Use `.agents/spark/SWARM.md` only when a Spark swarm is explicitly requested.

## Role

`.agents/spark/` is the fast, interruptible Codex Spark lane for
Tree-of-Sophia. It is calibrated for GPT-5.3-Codex-Spark style work:
near-instant, bounded edits, tight audits, small route repairs, and portable
handoffs.

Spark is an agent lane. Source homes, doctrine layers, mechanic packages,
generated truth, proof authority, memory authority, runtime state, and public
root entrypoints keep their own stronger owner surfaces. Spark's core execution
rule is `done-or-handoff`.

## Read before editing

Read root `AGENTS.md`, `.agents/AGENTS.md`, this card, `.agents/spark/SWARM.md`
when swarm context is requested, and the nearest owner surface for the files
being changed.

## Boundary Routes

- Use Spark for short-loop work where a small diff is enough.
- Start with a map: task, files, risks, and validation path.
- Prefer one bounded patch per loop.
- Keep one bounded ToS surface or route seam per Spark loop.
- End as `done` or `handoff`; slower-model continuation becomes an explicit
  handoff, not an in-session dependency.
- Read the nearest source docs before editing.
- Use the narrowest relevant validation already documented by the repo.
- Report exactly what was and was not checked.
- Escalate instead of widening into a broad architectural rewrite.
- Route canon promotion, doctrine invention, proof definition, memory truth,
  runtime behavior, and downstream KAG claims to their owning surfaces.

## Spark is strongest here for

- source-grounded wording fixes
- README / docs / boundaries alignment
- schema, example, and review-checklist consistency work
- lineage, counterpart, and provenance wording cleanup
- small structure edits that preserve distinction between source, extraction, interpretation, and synthesis

## Escalation Routes

- broad ontology redesign
- deep multi-text synthesis across many sources
- source interpretation leaps without clear grounding
- temporal, civilizational, or lineage synthesis that needs a slower review path

## Validation

Use the narrowest validator named by the touched owner surface. For purely
Spark-local edits, do manual consistency review and route-card validation.

If the Spark lane itself changes, run:

```bash
python scripts/validate_nested_agents.py
```

## Local done signal

A Spark task is done here when:

- provenance is clearer after the change
- layer distinctions remain visible
- uncertainty and contested interpretation are still marked
- derived convenience did not replace source-grounded thought
- manual review followed the repo’s existing review posture

## Local note

Spark should be a careful annotator here, not a conquering commentator.

## Reporting contract

Always report:

- the restated task and touched scope
- which files or surfaces changed
- whether the change was semantic, structural, or clarity-only
- what validation actually ran
- what still needs a slower model or human review
