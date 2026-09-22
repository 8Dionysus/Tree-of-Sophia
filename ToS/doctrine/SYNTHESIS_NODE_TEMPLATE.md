# Synthesis Node Template

A synthesis node holds a reviewed interpretation that brings source-linked claims and relations into a coherent reading.

## Core fields

The scaffold uses the shared [node contract](NODE_CONTRACT.md):

- `node_id`
- `node_type = synthesis`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

## Use the scaffold when

- The interpretation deserves an authored canonical handle.
- Its relation to the source and distilled principles is explicit.
- The reading has a defined scope and an inspectable review basis.

## Related node roles

The synthesis node carries interpretation. A principle node carries a
distilled, revisable claim. The source node anchors the passage. Their
relations let a reader follow the synthesis back through its grounds.

## Worked scaffold

The current family is under `ToS/canon/synthesis/`; its public example is
`ToS/public-compatibility/synthesis_node.example.json`.
