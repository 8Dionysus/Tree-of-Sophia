# Identifier Discipline

This document records the current sixth-wave ToS doctrine for stable public node identifiers.

It does not introduce a full executable corpus platform.
It defines the public grammar that later templates, examples, and derived handoffs should meet.

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

## What IDs should not encode

Do not encode into `node_id`:

- review status
- maturity claims
- branch ownership claims that are not yet stabilized
- derived KAG projections

The ID should identify the node, not narrate its whole lifecycle.

## Current scaffold posture

This wave gives public ID discipline to:

- source-node scaffolds
- concept-node scaffolds
- principle-node scaffolds
- lineage-node scaffolds
- event-node scaffolds
- state-node scaffolds
- support-node scaffolds

It does not claim a full branch pilot, full corpus taxonomy, or public validator program yet.
