# Support Node Template

A support node gives a reviewed carrier, symbol, place, hinge or recipient within a route its own authored handle.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = support`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- The subject helps explain the surrounding route.
- Its reading is stable enough to move from intake through review.
- The existing support family expresses the subject at the required level of detail.

## Source and family scope

The support family currently holds reviewed non-literal `n.*` subjects. Use
source, concept, principle, event and state nodes according to their defined
roles. Literal helpers remain in `ToS/candidate-intake/` for their dedicated
review pass. Each promotion records its own source and assessment.

## Worked scaffold

The current family is under `ToS/canon/support/`; its public example is
`ToS/public-compatibility/support_node.example.json`.
