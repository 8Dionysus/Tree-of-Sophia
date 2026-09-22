# Principle Node Template

A principle node holds a reviewed, distilled claim and its source-bearing ground.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = principle`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- The claim is stable enough to deserve an authored handle.
- Its source, interpretive scope and grounds for revision are explicit.
- A separate node makes the claim easier to inspect within the route.

## Source and intake

The source node anchors the passage. `ToS/candidate-intake/principles.csv`
holds proposed extractions. The principle node records the claim admitted by
review, with the source and review path visible.

## Worked scaffold

The current family is under `ToS/canon/principle/`; its public example is
`ToS/public-compatibility/principle_node.example.json`.
