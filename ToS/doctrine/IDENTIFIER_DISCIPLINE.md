# Identifier Discipline

This document defines stable public node identifiers for templates, authored
nodes, examples and derived handoffs.

## Core rule

Public ToS node IDs should follow:

`tos.<node_type>.<slug[.subslug...]>`

Keep them:

- lowercase
- ASCII
- dot-delimited
- stable enough to survive later corpus growth

## Segment meaning

- `tos` marks the public ToS namespace
- `<node_type>` names the current public family such as `source` or `concept`
- each later segment is a slug or subslug that helps keep the identifier readable

## Identity and lifecycle

The ID names the node throughout its lifecycle. Review status, maturity,
branch ownership and projection state belong to versioned metadata with
their own evidence. Updating that metadata preserves the node's ID.

## Current scaffold posture

The public ID grammar covers:

- source-node scaffolds
- concept-node scaffolds
- principle-node scaffolds
- lineage-node scaffolds
- event-node scaffolds
- state-node scaffolds
- support-node scaffolds
- context-node scaffolds
- analogy-node scaffolds
- synthesis-node scaffolds

New families declare their node type and use the same stable grammar.
