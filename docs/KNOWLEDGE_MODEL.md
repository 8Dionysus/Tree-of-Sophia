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

## Layering inside a node

A node may carry several layers at once, including:

- source text or canonical reference
- key terms
- distilled theses
- semantic field notes
- temporal context
- spatial or civilizational context
- lineage links
- human-reviewed interpretation
- provenance metadata

The presence of multiple layers does not erase their distinction.

At the first-wave baseline, every serious node should preserve at least:

- a source anchor
- key terms or concepts
- a distilled thesis
- explicit lineage relations

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

## Handoff to derived systems

`aoa-kag` and other downstream systems may consume ToS surfaces to build derived structures.

When that happens, the handoff should preserve:

- stable source identifiers where possible
- provenance paths back to ToS-authored material
- distinction between authored truth and derived projection
- bounded schemas rather than vague graph sprawl
- explicit non-identity notes when counterpart mappings are present

## What this model avoids

ToS should avoid becoming:

- a flat archive with no lineage
- a graph theater that hides sources behind edges
- a summary pile detached from provenance
- a private intuition engine masquerading as architecture
