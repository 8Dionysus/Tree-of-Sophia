# Event Node Template

An event node holds reviewed movement or change within a source-linked route.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = event`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- A movement in the source needs an authored handle.
- The movement has a clear basis in the source and has passed review.
- Its participants, conditions and relation to the surrounding route are legible.

## Related node roles

An event node describes movement. A state node describes a sustained
condition. A principle node holds a distilled claim. The source node anchors
the passage. The owning route under `ToS/candidate-intake/` retains the
candidate material and its review path; each event returns to that evidence.

## Worked scaffold

The current family is under `ToS/canon/event/`; its public example is
`ToS/public-compatibility/event_node.example.json`.
