# Principle Node Template

This document records the current first-wave scaffold for canonical principle
nodes in ToS.

Principle nodes should not float free as detached maxims.
They remain review-gated distilled claims that keep a visible route back to
their source-bearing ground.

## Core fields

A principle-node scaffold should expose at least:

- `node_id`
- `node_type = principle`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

This is the same minimal node contract used elsewhere in ToS.
What changes here is the center of gravity: a principle node stabilizes a
bounded distilled claim rather than a source surface or a language-neutral
concept handle.

## Template posture

Use the principle-node scaffold when:

- a distilled claim is stable enough to deserve its own authored handle
- the claim remains source-linked and reversible rather than timeless by fiat
- keeping the claim inside a larger source node would make the route harder to
  review

Principle nodes should stay:

- source-first
- review-gated
- bounded
- route-visible

## Boundary against source and intake

A principle node is not:

- the source node itself
- a raw row lifted unchanged from `intake/principles.csv`
- a free-floating doctrine surface with no route back to the source

The source node remains the authority surface for the bounded passage.
The intake table remains the fuller candidate field.
The principle node is the reviewed canonical stabilization that sits between
those two layers.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded principle family under `tree/principle/`
- one worked example mirrored into `examples/principle_node.example.json`
- no claim yet that ToS now owns a global principle atlas
