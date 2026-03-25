# ToS Node Contract

This document records the first-wave working law for ToS nodes.

It keeps Tree of Sophia source-first, lineage-aware, and explicit about interpretation layers.

## Minimum node contract

At the first-wave baseline, a node should not be treated as a flat note.

Each serious node should preserve at least:

- a source anchor or canonical reference
- key terms or concepts
- a distilled thesis or extraction layer
- explicit relations

These are the minimum layers that keep a node tied to meaning rather than turning it into archive dust.

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

Optional does not mean vague.
Every added layer should remain distinguishable from the source anchor beneath it.

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

Each witness block keeps the same `segment_id` values across languages so the node stays one node rather than three copies.
When a witness is maintainer-curated rather than bibliographically fixed, that posture should be named explicitly instead of being left implicit.

`translation_tensions` is an optional array of `{ segment_id, note }`.
Use it only when drift is philosophically load-bearing.

These fields do not change the required minimum node contract.
They make multilingual witness surfaces inspectable without splitting node identity by language.

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

Topic labels and era labels may help orientation, but they do not replace genealogical movement.

## Interpretation ladder

Interpretation should stay visibly layered:

1. source-linked layer
2. distilled thesis
3. commentary
4. cross-text comparison
5. speculative synthesis

No node should collapse these five levels into one undifferentiated paragraph.

## Anti-goals

Avoid turning ToS into:

- a flat archive of notes
- a summary pile detached from source anchors
- a graph theater that hides provenance behind edges
- a speculative essay machine with no visible interpretation ladder
- three parallel language trees for one authored node
- anonymous multilingual witness blocks that erase translator, edition, or maintainer posture
- synonym piles masquerading as multilingual concept identity
