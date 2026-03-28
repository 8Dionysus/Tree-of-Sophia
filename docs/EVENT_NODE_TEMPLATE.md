# Event Node Template

This document records the current route-local scaffold for canonical event
nodes in ToS.

Event nodes do not replace principles or source nodes.
They stabilize bounded movement inside a reviewed route so the tree can carry
dynamic legibility without collapsing into raw edge tables.

## Core fields

An event-node scaffold should expose at least:

- `node_id`
- `node_type = event`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

The shape stays compact because it reuses the current minimal node contract.

## Template posture

Use the event-node scaffold when:

- a bounded movement in the source route needs its own authored handle
- the movement is clearer as a canonical node than as an unreviewed intake row
- the node can stay source-linked rather than turning into abstract doctrine

Event nodes should stay:

- source-first
- route-local
- review-gated
- dynamic rather than atlas-like

## Boundary against principle and intake

An event node is not:

- the source node itself
- a raw row lifted unchanged from `intake/event_state_nodes.csv`
- a principle node that states a distilled claim

The event node holds movement.
The principle node holds a reviewed claim distilled from the route.
The intake row remains the fuller candidate field beneath both.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded event family under `tree/event/`
- one worked example mirrored into `examples/event_node.example.json`
- no claim yet that ToS now owns a general event atlas
