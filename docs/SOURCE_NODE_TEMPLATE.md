# Source Node Template

This document records the current sixth-wave ToS scaffold for source nodes.

The point is not to force every source into one rigid mold.
The point is to give the first public source-node surface a reviewable minimum shape.

## Core fields

A source-node scaffold should expose at least:

- `node_id`
- `node_type = source`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

These fields should remain visibly tied to the first-wave node contract.

In a bounded multilingual source entry, a source node may also expose:

- `language_witnesses`
- `translation_tensions`

These remain optional fields layered on top of the same shared `node_id`.

## Template posture

Use the source-node scaffold when:

- the source itself is the primary anchor
- the reader needs a stable authored handle for a work, passage, fragment, or excerpt
- later interpretation should remain visibly downstream of the source

When multilingual source entry is needed, keep one source node with multiple witness layers rather than cloning the node by language.

## Multilingual witness posture

Use `language_witnesses` only when the multilingual surface is part of the source-facing contract itself.

Keep the witness posture:

- bounded
- reviewable
- source-authoritative
- segment-aligned across languages

Use `translation_tensions` only where witness drift is philosophically load-bearing.

## Worked scaffold

The current worked example now also serves as the first bounded trilingual Zarathustra source-entry route.

It should be read as:

- one coherent example set
- one bounded source-facing opening rather than wider plurality
- not a monopoly of future corpus direction
