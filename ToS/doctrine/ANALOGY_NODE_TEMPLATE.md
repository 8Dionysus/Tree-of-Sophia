# Analogy Node Template

An analogy node holds a reviewed image-bearing comparison and explains its role in a source-linked reading.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = analogy`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- An image or comparison carries meaning in the route.
- The source and the proposed interpretation have been reviewed.
- The node identifies what is compared, the relevant aspect and the comparison's limits.

## Related node roles

The analogy node carries the comparison. Event and state nodes carry movement
and condition; support nodes identify the carrier subjects around the image.
Each image receives its own source-linked review before canon admission.

## Worked scaffold

The current family is under `ToS/canon/analogy/`; its public example is
`ToS/public-compatibility/analogy_node.example.json`.
