# Analogy Node Template

This document records the current route-local scaffold for canonical analogy
nodes in ToS.

Analogy nodes do not replace event, state, or support surfaces.
They stabilize a reviewed image-bearing comparison when leaving it inside a
different family would blur the route.

## Core fields

An analogy-node scaffold should expose at least:

- `node_id`
- `node_type = analogy`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

The shape stays compact because it reuses the current minimal node contract.

## Template posture

Use the analogy-node scaffold when:

- a bounded image or comparison is load-bearing for the route
- the image is more than raw intake residue
- promoting it does not require opening a broad analogy atlas

Analogy nodes should stay:

- source-first
- route-local
- review-gated
- image-bearing rather than doctrinal

## Boundary against dynamic and support surfaces

An analogy node is not:

- a substitute for event or state nodes
- just another support carrier
- a license to move every metaphor into canon

The analogy node holds a reviewed image-bearing comparison.
The event/state node holds movement or condition.
The support node holds the carrier surfaces around that image.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded analogy family under `tree/analogy/`
- one worked example mirrored into `examples/analogy_node.example.json`
- no claim yet that ToS now owns a general analogy program
