# State Node Template

A state node holds a reviewed, sustained condition within a source-linked route.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = state`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- A condition in the source needs an authored handle.
- The condition has a clear basis in the source and has passed review.
- Its duration or persistence, scope and relation to the route are legible.

## Related node roles

A state node describes a sustained condition. An event node describes
movement. A principle node holds a distilled claim. The source node anchors
the passage. The owning route under `ToS/candidate-intake/` retains the
candidate material and its review path; each state returns to that evidence.

## Worked scaffold

The current family is under `ToS/canon/state/`; its public example is
`ToS/public-compatibility/state_node.example.json`.
