# ToS Node Contract

ToS nodes give source-linked thought a stable, reviewable form. Each node
connects its subject, source, interpretation and relations.

The same minimum contract currently covers source, concept, principle, lineage,
event, state, support, context, analogy, and synthesis nodes unless a more specific
family template narrows the posture.

## Minimum node contract

Each node preserves at least:

- a source anchor or canonical reference
- key terms or concepts
- a distilled thesis or extraction layer
- explicit relations

Together these layers let a reader recover the basis of a thesis and follow
its relations to other thought.

## Authored description

Describe the subject through its content, purpose, properties, relations and
source-grounded examples. State the scope and uncertainty that affect this
particular reading. Preserve a source's negation, conditional reasoning and
disagreement when they carry its meaning.

Use type, identity, review, rights and operation fields for their respective
conditions. Explain shared processing and permission rules in the owning
contract. Keep a node's description focused on the subject; include a type
distinction there when it resolves an actual ambiguity in the material.

Review a proposed description for both fidelity and explanatory value. Each
qualification should identify the specific reading, evidence or use it limits.
Language revisions retain the subject's identity, substantive scope and
version history under the owning record contract.

## Optional but strongly preferred layers

As the node deepens, it may also include:

- language witnesses when multilingual source entry is load-bearing
- witness provenance when multilingual entry would otherwise blur translator, edition, or maintainer posture
- semantic field notes
- temporal context
- spatial or civilizational context
- commentary
- cross-text comparison
- speculative synthesis
- translation-tension notes when witness drift matters

Give every added layer an explicit role and a visible relationship to its
source anchor.

## Optional multilingual witness layers

When one authored node needs a bounded multilingual entry, it may add:

- `language_witnesses`
- `translation_tensions`

`language_witnesses` is an array of witness blocks with:

- `language`
- `role`
- optional `witness_ref`
- optional `edition_or_source`
- optional `translator_or_editor`
- optional `publication_year`
- optional `normalization_note`
- `segments`

Each witness segment may also expose an optional `locator` when the bounded slice needs segment-level provenance.

The current public roles are:

- `canonical_source`
- `working_translation`
- `bridge_translation`

These roles describe how a witness serves the authored node.
`canonical_source` identifies its source-facing witness;
`working_translation` and `bridge_translation` identify translation functions.
Edition identity, text quality, language competence and permitted uses belong
to the exact source or translation evidence and its scoped assessment.

Each witness block keeps the same `segment_id` values across languages within
one node. Identify the translator, edition or maintainer responsible for each
witness.

`translation_tensions` is an optional array of `{ segment_id, note }`.
Use it only when drift is philosophically load-bearing.

These optional fields add inspectable multilingual evidence to the shared
node and its required source, thesis and relation layers.

## Human representations

An existing native node may explicitly opt into `tos_canonical_node_v1` with
`schema_version` and a positive `record_version`. Its native `node_id` remains
its identity. Unversioned historical nodes retain their legacy status. The first explicit
version is a source-owner migration: preserve the prior bytes and review in
history, and record the adoption date and baseline.
Further corrections preserve identity, advance the explicit version and retain
their predecessors. Forms and assessments resolve the exact version and digest
they were issued for.

Only this opt-in permits `preferred_label`, `variant_labels` and
`field_languages` for `preferred_label` and `distilled_thesis`. Variant names
retain their language, optional script, source reference and separate wording
status. Names share the node's subject identity. Field language describes the
wording; witness language and author language have their own source fields.
Unknown declarations remain unknown. Translation quality, interpretation,
canon and publication follow their respective assessment and permission
routes. Named nodes retain the source anchor, thesis, relations and
interpretation layers required above.

[HUMAN_FORMS](HUMAN_FORMS.md) governs versioned names, captions, hover text,
exact statements, grounds, history and technical readings of the same subject.
Forms bind source records and mandatory context; source-copy, admitted-template
rendering and assessed freeform wording each retain their own provenance and
assessment requirements alongside the node's source and witness layers.

## Lineage before archive

ToS grows best when lineage is clearer than storage.

Prefer explicit relation names such as:

- `predecessor`
- `descendant`
- `parallel`
- `mutation`
- `tension`

When context or commentary needs a more precise fit, use explicit relation names such as:

- `contextualized-by`
- `commentary-on`

Topic and era labels support navigation; explicit relations carry genealogical
movement.

## Interpretation ladder

Interpretation should stay visibly layered:

1. source-linked layer
2. distilled thesis
3. commentary
4. cross-text comparison
5. speculative synthesis

Give each level a visible location and label. A reader should be able to trace
an interpretation through its thesis to the cited source, inspect a relation's
basis, and identify each language witness and its responsible contributor.
