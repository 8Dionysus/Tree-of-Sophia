# AGENTS.md

## Repository role
This repository is the canonical high-level statement of Tree of Sophia (ToS).

Tree of Sophia is a living knowledge architecture for philosophy and world thought. It traces texts, concepts, contexts, and lineages across time and cultures.

At its current public stage, this repository is primarily a landing and conceptual foundation. Treat it as the place that defines the knowledge architecture and its discipline, not as a grab-bag for unrelated implementation details.

## Priority of instructions
- Follow direct maintainer instructions first.
- Then follow this file.
- Preserve the distinction between authored meaning, derived structure, and operational support.
- When a task belongs to AoA, `aoa-kag`, or `abyss-stack`, route there instead of forcing it into ToS.

## What ToS owns
Use this repository for work on:
- the high-level statement of what ToS is
- conceptual architecture for knowledge nodes and relationships
- source-first knowledge discipline
- text, concept, context, and lineage modeling
- contributor rules for interpretation and curation
- public architecture notes that explain how meaning should be preserved and extended

## What ToS does not own
Do not use this repository as the main home for:
- runtime and deployment implementation details
- general agent workflow machinery
- infrastructure configuration
- eval harnesses that are not specifically about ToS knowledge claims
- derived knowledge substrate as the sole source of truth
- contextless note dumps or flat archives detached from provenance

Related ownership boundaries:
- `Agents-of-Abyss` owns the operational ecosystem around ToS
- `aoa-kag` owns derived provenance-aware substrate layers
- `abyss-stack` owns runtime, deployment, storage, and service posture

## Knowledge posture
Preserve source traceability at all times.

A strong ToS contribution keeps distinct:
- raw source text or fragment
- key terms and concepts
- distilled theses or semantic extraction
- deeper interpretation
- temporal context
- spatial or civilizational context
- lineage relations to predecessors, descendants, parallels, and mutations

Do not flatten these layers into one undifferentiated summary.

## Guiding axis
ToS is not a static archive. Its guiding axis is a living calibration of meaning: becoming, overcoming, creation of values, and affirmation of life.

In the current public architecture, *Thus Spoke Zarathustra* functions as a recurring calibration root for that axis.

Use this axis as an interpretive compass, not as permission to distort sources or force every text into one reading.

## Working posture
- Sources before abstraction.
- Layered meaning over flat summaries.
- Lineages over isolated notes.
- Human curation over blind automation.
- Growth through explicit structure.
- AI as amplifier of judgment, not a replacement for judgment.
- Mark uncertainty, ambiguity, and contested interpretation explicitly.
- Keep public artifacts public-safe and provenance-aware.

## Editing rules
When adding or revising content:
- preserve a visible path back to primary or authoritative material
- keep extraction, interpretation, and synthesis distinguishable
- keep lineage relations explicit when they matter
- keep temporal and civilizational context attached to claims when relevant
- prefer reversible structure over overfit ontology
- avoid premature universal schemas that erase nuance
- avoid replacing source-grounded thought with generic AI paraphrase

When summarizing:
- summarize with fidelity to the source
- name the layer you are operating in when needed: source, extraction, interpretation, synthesis
- preserve unresolved tensions instead of sanding them flat

When introducing structure:
- design nodes and relations so future expansion remains legible
- prefer explicit relation types over vague associative sprawl
- keep derived artifacts clearly derived

## Repository routing guidance
Use the smallest correct destination.

- `Tree-of-Sophia`: knowledge architecture, source discipline, node layering, lineage logic
- `Agents-of-Abyss`: operational ecosystem, federation rules, layer coordination
- `aoa-kag`: derived provenance-aware knowledge substrate
- `aoa-techniques`: reusable engineering practice for knowledge operations
- `abyss-stack`: infrastructure and runtime body beneath ToS and AoA

If the requested work mainly concerns runtime, agent orchestration, or derived substrate mechanics, it probably does not belong here as the primary surface.

## Validation
No public validation script is referenced here yet.

Validate changes through structured review:
- use `docs/REVIEW_CHECKLIST.md` as the default manual review route
- check consistency with `README.md` and any architecture notes
- check that provenance remains visible
- check that node layering remains intact
- check that lineage relations are not lost
- check that authored meaning is not replaced by derived convenience surfaces

If you add a generated or derived artifact, make its source and limits explicit.

## Definition of done
A change is done when:
- a reader can tell what the source is, what was extracted, what was inferred, and what remains interpretive
- provenance is clearer after the change
- conceptual layering is stronger after the change
- lineage and context are preserved where relevant
- the repository remains a living knowledge architecture rather than a flat archive

## Style for this repository
Write with precision, depth, and restraint.

Prefer:
- source-grounded language
- layered explanations
- explicit relation names
- careful distinctions between evidence and interpretation

Avoid:
- generic encyclopedic flattening
- detached summaries with no lineage or context
- overstated certainty about contested ideas
- rhetoric that obscures provenance
