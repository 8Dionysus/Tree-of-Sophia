# Source Node Template

A source node gives a work, passage, fragment or excerpt a stable authored
handle. Its anchor lets readers return from interpretation to the source.

## Core fields

A source-node scaffold should expose at least:

- `node_id`
- `node_type = source`
- `source_anchor`
- `key_terms`
- `distilled_thesis`
- `relations`
- `interpretation_layers`

These fields follow the shared [node contract](NODE_CONTRACT.md).

In a bounded multilingual source entry, a source node may also expose:

- `language_witnesses`
- `translation_tensions`

These remain optional fields layered on top of the same shared `node_id`.

## Template posture

Use the source-node scaffold when:

- the source itself is the primary anchor
- the reader needs a stable authored handle for a work, passage, fragment, or excerpt
- later interpretation should remain visibly downstream of the source

For multilingual source entry, keep one source node with separately identified
witness layers for each language.

## Multilingual witness posture

Use `language_witnesses` only when the multilingual surface is part of the source-facing contract itself.

Keep the witness posture:

- bounded
- reviewable
- source-authoritative
- segment-aligned across languages
- explicit enough to distinguish source witness, published translation, and maintainer-curated witness where that difference matters

The role vocabulary describes each witness's function in the authored node.
Edition/Item/File identity, immutable text layers, selectors and philological
assessment retain their exact source records. Translation quality and
competence are established through the translation packet and its scoped
assessment.

A bounded multilingual source entry may also expose optional witness provenance fields such as:

- `witness_ref`
- `edition_or_source`
- `translator_or_editor`
- `publication_year`
- `normalization_note`

Each witness segment may also expose an optional `locator`.

Use these fields to identify the witness and the contributors responsible for
it. Detailed bibliographic records belong to the source-witness owner.

Use `translation_tensions` only where witness drift is philosophically load-bearing.

## Worked scaffold

The worked example serves the trilingual Zarathustra source-entry route.
It shows one source node with aligned witnesses and their provenance. Further
sources use the shared contract and their own source and review evidence.
