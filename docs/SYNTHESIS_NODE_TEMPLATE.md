# Synthesis Node Template

This document records the current route-local scaffold for canonical synthesis
nodes in ToS.

Synthesis nodes do not replace source, principle, or lineage surfaces.
They stabilize a reviewed interpretive reading when the route has moved beyond
distilled principle but should not open a separate commentary family.

## Core fields

A synthesis-node scaffold should expose at least:

- `node_id`
- `node_type = synthesis`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

The shape stays compact because it reuses the current minimal node contract.

## Template posture

Use the synthesis-node scaffold when:

- a source-linked interpretive reading deserves a canonical home
- the reading is more than a distilled principle
- the route can keep the synthesis bounded and reviewable

Synthesis nodes should stay:

- source-first
- route-local
- review-gated
- explicitly interpretive rather than hidden doctrine

## Boundary against principle and source

A synthesis node is not:

- the source node itself
- a replacement for the principle spine
- a free-floating essay detached from route anchors

The synthesis node holds reviewed interpretation.
The principle node holds distilled reversible claim.
The source node remains authoritative for the route.

## Worked scaffold

The first worked example is route-local and narrow:

- one bounded synthesis family under `tree/synthesis/`
- one worked example mirrored into `examples/synthesis_node.example.json`
- no separate commentary family and no wider synthesis atlas yet
