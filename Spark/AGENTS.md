# Spark lane for Tree-of-Sophia

This file only governs work started from `Spark/`.

The root `AGENTS.md` remains authoritative for repository identity, ownership boundaries, reading order, and validation commands. This local file only narrows how GPT-5.3-Codex-Spark should behave when used as the fast-loop lane.

If `SWARM.md` exists in this directory, treat it as queue / swarm context. This `AGENTS.md` is the operating policy for Spark work.

## Default Spark posture

- Use Spark for short-loop work where a small diff is enough.
- Start with a map: task, files, risks, and validation path.
- Prefer one bounded patch per loop.
- Read the nearest source docs before editing.
- Use the narrowest relevant validation already documented by the repo.
- Report exactly what was and was not checked.
- Escalate instead of widening into a broad architectural rewrite.

## Spark is strongest here for

- source-grounded wording fixes
- README / docs / boundaries alignment
- schema, example, and review-checklist consistency work
- lineage, counterpart, and provenance wording cleanup
- small structure edits that preserve distinction between source, extraction, interpretation, and synthesis

## Do not widen Spark here into

- broad ontology redesign
- deep multi-text synthesis across many sources
- new source interpretation leaps without clear grounding
- flattening temporal, civilizational, or lineage context into generic summary

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
