# ToS Manual Review Note: Corpus Scaffold

Date: 2026-03-22

Touched surfaces:

- `README.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
- `docs/KNOWLEDGE_MODEL.md`
- `docs/IDENTIFIER_DISCIPLINE.md`
- `docs/SOURCE_NODE_TEMPLATE.md`
- `docs/CONCEPT_NODE_TEMPLATE.md`
- `docs/REVIEW_CHECKLIST.md`
- `schemas/tos-node-contract.schema.json`
- `examples/source_node.example.json`
- `examples/concept_node.example.json`

Most at-risk checklist items:

- identifier drift across docs, schema, and examples
- source-node versus concept-node template collapse
- scaffold examples quietly pretending to be a branch pilot
- ToS versus AoA ownership boundary

Review result:

- `yes` public `node_id` discipline is stable and readable
- `yes` docs, schema, and examples use the same `tos.<node_type>.<slug[.subslug...]>` grammar
- `yes` source-node and concept-node scaffolds remain distinct
- `yes` worked examples stay bounded scaffold examples rather than a hidden branch pilot
- `yes` ToS keeps ownership of node IDs, templates, and corpus-facing example discipline

What remains interpretive or unresolved:

- the corpus scaffold is still manual-review-first rather than backed by a public validator script
- the worked example family is orienting and coherent, but not a claim of exclusive future canon
- lineage/branch pilot work still belongs to a later wave after the scaffold proves itself

Likely downstream follow-up:

- `Agents-of-Abyss` for center-layer routing and support doctrine
- later ToS lineage or branch pilot work
- later wider world-thought expansion after the scaffold stabilizes
