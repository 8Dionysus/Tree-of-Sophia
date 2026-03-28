# ToS Knowledge Model

This document defines the high-level model for Tree of Sophia.

## Orientation

ToS is a source-first, lineage-aware knowledge architecture.

It should be understood through two simultaneous structural views:

1. **Tree-shaped primary orientation**
   - used for rooted traversal
   - keeps long-horizon lineage legible
   - provides stable entry paths for humans and smaller models

2. **Graph-typed secondary relations**
   - used for parallels, convergences, mutations, and commentary links
   - allows thought to cross branches without pretending there is no trunk

The practical rule is simple:

**Tree for orientation. Graph for relation. Source for authority.**

## Node families

ToS may grow multiple node families. At the public baseline, the most important are:

- **source nodes**: works, books, passages, fragments, excerpts
- **concept nodes**: key terms, theses, motifs, semantic fields
- **lineage nodes**: schools, streams, genealogies, developmental arcs
- **context nodes**: temporal, geographic, linguistic, civilizational, and institutional context
- **synthesis nodes**: human-reviewed interpretation that remains explicitly linked to sources

Additional node families may appear later, but they should be introduced explicitly rather than implicitly.

## Identifier discipline and first templates

At the current corpus-scaffold wave, public node IDs should follow:

`tos.<node_type>.<slug[.subslug...]>`

The first template family stays narrow:

- source nodes
- concept nodes

These templates and examples are scaffold surfaces, not a full branch pilot and not a complete corpus ontology.

See [IDENTIFIER_DISCIPLINE](IDENTIFIER_DISCIPLINE.md), [SOURCE_NODE_TEMPLATE](SOURCE_NODE_TEMPLATE.md), and [CONCEPT_NODE_TEMPLATE](CONCEPT_NODE_TEMPLATE.md) for the compact public scaffold.

## First bounded lineage pilot

At the current pilot wave, ToS may also turn the new scaffold into one real small branch.

This pilot should stay:

- source-first
- bounded
- reviewable
- calibration-family specific rather than universal

The first public branch pilot adds a lineage-node template and one small worked route.
It is not yet a broad branch program and not yet wider world-thought expansion.

See [LINEAGE_NODE_TEMPLATE](LINEAGE_NODE_TEMPLATE.md) and [CALIBRATION_LINEAGE_PILOT](CALIBRATION_LINEAGE_PILOT.md) for the compact pilot surfaces.

## Pre-expansion soil before wider movement

At the previous eighth-wave note, ToS may also prepare soil for later expansion without treating that preparation as active tree movement.

This preparation may include:

- a context-node template
- manual corpus-entry gating
- review-route hardening for pre-expansion work

This wave should stay:

- source-first
- reviewable
- structurally useful
- visibly non-expansionary

It does not yet open a second family, new branch nodes, or wider world-thought expansion.

See [CONTEXT_NODE_TEMPLATE](CONTEXT_NODE_TEMPLATE.md), [MANUAL_CORPUS_ENTRY_GATE](MANUAL_CORPUS_ENTRY_GATE.md), and [PRE_EXPANSION_SOIL](PRE_EXPANSION_SOIL.md) for the compact preparation surfaces.

## Bounded trilingual Zarathustra entry

At the current gate-opening slice, ToS may now begin real tree motion through one bounded manual trilingual route.

This route should stay:

- source-first
- bounded
- reviewable
- single-tree rather than language-split

The first public route uses:

- `tos.source.thus-spoke-zarathustra.prologue`
- `tos.concept.becoming`
- `tos.concept.overcoming`

The current role contract is asymmetric on purpose:

- German as canonical source authority
- Russian as human-reviewed working translation
- English as bridge translation for public structural use

This route grows one `node_id` with `language_witnesses`, not three node copies.
It may also record `translation_tensions` when drift is philosophically load-bearing.
Concept nodes remain language-neutral even when their source route is multilingual.

This is still not wider world-thought expansion.

See [ZARATHUSTRA_TRILINGUAL_ENTRY](ZARATHUSTRA_TRILINGUAL_ENTRY.md) for the compact route note.

## Tree-first tiny-entry seam

At the current wave, ToS may also publish one tree-first tiny-entry seam for humans and smaller models without replacing authored node law.

The current public chain is:

`README.md -> node kind -> capsule surface -> authority surface -> one bounded concept hop`

In the first public route:

- `README.md` is the current public `tos-root`
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` is the worked capsule surface
- `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json` is the canonical authored source node
- `examples/source_node.example.json` is the current public compatibility authority surface
- `tree/concept/becoming/node.json` is the canonical authored bounded hop
- `examples/concept_node.example.json` is the current public compatibility bounded hop
- `docs/KNOWLEDGE_MODEL.md` remains the in-repo fallback orientation surface

This seam stays:

- tree-first
- source-first
- bounded
- non-authoritative in itself

Its job is orientation, not ownership transfer.
`aoa-kag` and `aoa-routing` may consume this route downstream, and the current public seam is already used that way, but they do not become ToS authority surfaces.

See [TINY_ENTRY_ROUTE](TINY_ENTRY_ROUTE.md) for the compact route doctrine.

## Repository content layers

ToS now also keeps distinct repository-facing layers for how material enters and stabilizes:

- `sources/` for primary witness and source files
- `intake/` for raw extracted candidate material and current candidate tabular base packs
- `tree/` for canonical authored nodes, relations, and vocabulary governance surfaces
- `examples/` for current public compatibility and tiny-entry surfaces
- `generated/` for downstream-safe derived exports

These layers may point to one another.
They should not be silently collapsed into one interchangeable surface.

The current workbook carrier may seed these layers, but it is not itself a
canonical ToS surface.

The current bounded Zarathustra route also keeps a deliberate split:

- `tree/source/.../node.json` remains the authored source-node canon
- `intake/.../mode-b/*.csv` remains the candidate tabular base pack

Those surfaces may mirror one another in bounded ways.
They do not replace one another.

## Layering inside a node

A node may carry several layers at once, including:

- source text or canonical reference
- language witnesses when multilingual source entry is load-bearing
- key terms
- distilled theses
- semantic field notes
- temporal context
- spatial or civilizational context
- lineage links
- human-reviewed interpretation
- provenance metadata
- translation-tension notes when witness drift matters

The presence of multiple layers does not erase their distinction.

At the first-wave baseline, every serious node should preserve at least:

- a source anchor
- key terms or concepts
- a distilled thesis
- explicit relations

See [NODE_CONTRACT](NODE_CONTRACT.md) for the compact working contract.

## Core relation types

At the baseline, ToS should privilege a compact relation vocabulary:

- **predecessor**
- **descendant**
- **parallel**
- **mutation**
- **tension**
- **commentary-on**
- **contextualized-by**

New relation types should be added only when they clarify meaning rather than decorate it.

When context or commentary nodes are present, `contextualized-by` and `commentary-on` may be the more honest relation names than forcing every link into a genealogical label.

## Source-first discipline

The minimum authority chain should remain visible:

- source material
- extracted or distilled layer
- human-reviewed synthesis
- downstream derived structures

Every stronger layer should be able to point back toward its sources.

## Interpretation ladder

ToS should keep interpretation visibly layered rather than collapsing source, commentary, and synthesis into one voice.

The working ladder is:

1. source-linked layer
2. distilled thesis
3. commentary
4. cross-text comparison
5. speculative synthesis

This keeps interpretation alive without letting it float free from source anchors.

## Idea lineage and practice lineage

ToS is primarily a knowledge architecture for the genealogy of ideas.

At the current second-wave note, it may also recognize a neighboring genealogy of practices:
- origin
- mutation
- adaptation
- promotion
- canonization
- deprecation

That recognition stays conceptual.
Operational detail still belongs in AoA repositories such as `aoa-techniques`, `aoa-playbooks`, and `aoa-evals`.

See [PRACTICE_BRANCH](PRACTICE_BRANCH.md) for the compact boundary note.

## Counterpart mapping without collapse

ToS may also permit an optional derived counterpart bridge between some concepts and some AoA operational forms.

That bridge stays:

- derived
- optional
- suggestive
- non-identity

It should help orientation, not erase difference.
Conceptual meaning remains authored in ToS.
Operational meaning remains authored in the source AoA repository.
Machine-readable counterpart projections belong downstream in `aoa-kag`.

See [COUNTERPART_POLICY](COUNTERPART_POLICY.md) for the compact policy and example pairs.

## Context compost

ToS may also digest witness-facing or other source-linked raw material through a source-first compost cycle:

`raw -> note -> synthesis -> principle -> canon`

This route stays:

- source-first
- reviewable
- layered
- reversible

The current wave treats that route as doctrine, not as a full executable platform.
Operational ownership of the witness-producing route remains in AoA repositories.

The named doctrinal objects for this route are:
- `CompostNode`
- `SourceRef`
- `DecayPolicy`
- `CanonBundle`

One compact worked path for the current pilot wave is:
- `WitnessTrace -> Note -> Principle`

That path gives ToS a disciplined digestion route without claiming that canonization or later growth seeds are already solved.

See [CONTEXT_COMPOST](CONTEXT_COMPOST.md) for the compact doctrine.

## Calibration axis

ToS is not a neutral dust cloud of equally weighted fragments.

Its current guiding axis is a living calibration of meaning:

- becoming
- overcoming
- creation of values
- affirmation of life

In the current public architecture, *Thus Spoke Zarathustra* serves as a recurring calibration root for that axis.

That axis should guide curation and reading posture without becoming a hidden monopoly of meaning.
It is an orienting calibration, not a license to force every text into one thesis.

See [CALIBRATION_AXIS](CALIBRATION_AXIS.md) for the compact rule and guardrails.

## Human-curated, AI-amplified expansion

ToS may use AI to amplify growth, but not to replace human judgment.

AI may assist with:

- extraction
- clustering
- cross-link suggestions
- contrast proposals
- lineage hypotheses

Human-reviewed curation still owns:

- source reading
- node judgment
- final interpretive stance
- principle or canon-facing promotion

This keeps AI visible as an amplifier rather than a hidden sovereign author.

See [HUMAN_CURATED_EXPANSION](HUMAN_CURATED_EXPANSION.md) for the compact curation note.

## Growth by explicit structure

ToS should grow through explicit structure rather than silent accumulation.

The practical growth choices are:

- deepen a node when a stable source route already exists and the new material strengthens it
- create a node when a distinct source, concept, or context needs its own authored home
- form a branch when multiple nodes now justify a durable lineage or thematic path

This protects ToS from node explosion, archive sediment, and quantity theater.

See [GROWTH_STRUCTURE](GROWTH_STRUCTURE.md) for the compact growth law.

## Handoff to derived systems

`aoa-kag` and other downstream systems may consume ToS surfaces to build derived structures.

When that happens, the handoff should preserve:

- stable source identifiers where possible
- provenance paths back to ToS-authored material
- distinction between authored truth and derived projection
- bounded schemas rather than vague graph sprawl
- explicit non-identity notes when counterpart mappings are present

The same rule applies to tiny-entry routes: downstream routing or KAG layers may consume them later, but they do not replace ToS-authored capsule or authority surfaces.

## What this model avoids

ToS should avoid becoming:

- a flat archive with no lineage
- a graph theater that hides sources behind edges
- a summary pile detached from provenance
- a private intuition engine masquerading as architecture
