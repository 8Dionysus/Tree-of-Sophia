# State Node Template

This document records the current route-local scaffold for canonical state
nodes in ToS.

State nodes do not replace principles or source nodes.
They stabilize bounded held conditions inside a reviewed route so the tree can
carry durable stances and pressures without collapsing into raw graph rows.

## Core fields

A state-node scaffold should expose at least:

- `node_id`
- `node_type = state`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

The shape stays compact because it reuses the current minimal node contract.

## Template posture

Use the state-node scaffold when:

- a bounded condition in the source route needs its own authored handle
- the condition is clearer as a canonical node than as an unreviewed intake row
- the node can stay source-linked rather than turning into commentary doctrine

State nodes should stay:

- source-first
- route-local
- review-gated
- condition-bearing rather than abstract by fiat

## Boundary against principle and intake

A state node is not:

- the source node itself
- a raw row lifted unchanged from `intake/event_state_nodes.csv`
- a principle node that states a distilled claim

The state node holds a sustained condition.
The principle node holds a reviewed thesis distilled from the route.
The intake row remains the fuller candidate field beneath both.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded state family under `tree/state/`
- one worked example mirrored into `examples/state_node.example.json`
- no claim yet that ToS now owns a general state atlas
