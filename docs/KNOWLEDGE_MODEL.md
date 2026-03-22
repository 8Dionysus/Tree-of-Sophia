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

## Core relation types

At the baseline, ToS should privilege a compact relation vocabulary:

- **predecessor**
- **descendant**
- **parallel**
- **mutation**
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

## Handoff to derived systems

`aoa-kag` and other downstream systems may consume ToS surfaces to build derived structures.

When that happens, the handoff should preserve:

- stable source identifiers where possible
- provenance paths back to ToS-authored material
- distinction between authored truth and derived projection
- bounded schemas rather than vague graph sprawl

## What this model avoids

ToS should avoid becoming:

- a flat archive with no lineage
- a graph theater that hides sources behind edges
- a summary pile detached from provenance
- a private intuition engine masquerading as architecture
