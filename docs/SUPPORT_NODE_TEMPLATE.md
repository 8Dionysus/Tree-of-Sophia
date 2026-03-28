# Support Node Template

This document records the current route-local scaffold for canonical support
nodes in ToS.

Support nodes do not replace source, concept, principle, event, or state
surfaces.
They stabilize the reviewed `n.*` layer that helps the current route stay
legible without forcing an early split into many semantic families.

## Core fields

A support-node scaffold should expose at least:

- `node_id`
- `node_type = support`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

The shape stays compact because it reuses the current minimal node contract.

## Template posture

Use the support-node scaffold when:

- a route-local carrier, symbol, place, hinge, or recipient surface needs its
  own authored handle
- the row is stable enough to move beyond raw intake
- opening a whole new semantic family would be premature

Support nodes should stay:

- source-first
- route-local
- review-gated
- bounded rather than taxonomic

## Boundary against concept and residue

A support node is not:

- the source node itself
- a substitute for concept, principle, event, or state nodes
- a claim that every remaining `n.*` row should already be canonical

The support family is the current bounded home for the route's reviewed
non-literal `n.*` surfaces.
Literal helpers remain in `intake/` until a later dedicated literal pass.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded support family under `tree/support/`
- one worked example mirrored into `examples/support_node.example.json`
- no claim yet that ToS now owns a broad support ontology
