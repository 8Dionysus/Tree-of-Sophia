# ToS Review Checklist

Use this checklist when `Tree-of-Sophia` changes and no public validator covers the touched surface.

The bounded tiny-export seam already has a public validator at `python scripts/validate_kag_export.py`.

The goal is not to imitate a machine check.
The goal is to make source-first review repeatable and explicit.

## When to use it

Use this checklist for changes to:

- `README.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
- `docs/KNOWLEDGE_MODEL.md`
- `docs/NODE_CONTRACT.md`
- `docs/PRACTICE_BRANCH.md`
- `docs/COUNTERPART_POLICY.md`
- `docs/CONTEXT_COMPOST.md`
- `docs/CALIBRATION_AXIS.md`
- `docs/HUMAN_CURATED_EXPANSION.md`
- `docs/GROWTH_STRUCTURE.md`
- `docs/IDENTIFIER_DISCIPLINE.md`
- `docs/SOURCE_NODE_TEMPLATE.md`
- `docs/CONCEPT_NODE_TEMPLATE.md`
- `docs/LINEAGE_NODE_TEMPLATE.md`
- `docs/CALIBRATION_LINEAGE_PILOT.md`
- `docs/CONTEXT_NODE_TEMPLATE.md`
- `docs/MANUAL_CORPUS_ENTRY_GATE.md`
- `docs/PRE_EXPANSION_SOIL.md`
- `docs/TINY_ENTRY_ROUTE.md`
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`
- `examples/*.json`
- `schemas/*.json`
- new authored architecture notes

## Review route

Walk the change in this order:

1. Compare the change against `README.md` and the owning architecture note it touches.
2. Check that authored meaning remains distinguishable from derived structure.
3. Check that source, extraction, interpretation, and synthesis layers remain legible.
4. Check that lineage and context did not disappear where they matter.
5. Check that downstream AoA operational detail did not quietly move into ToS.
6. Check that counterpart mapping, when present, stays optional and non-identity.
7. Check that compost routes, when present, preserve source refs, review state, and decay or demotion posture.
8. Check that calibration, when present, sharpens orientation without replacing source reading or plurality.
9. Check that AI-assisted growth, when present, stays visible, reviewable, and non-sovereign.
10. Check that growth decisions stay structural rather than quantity-driven.
11. Check that node IDs, when present, stay stable, readable, and consistent across docs, schema, and examples.
12. Check that multilingual witnesses, when present, keep one shared node ID, explicit source authority, and no parallel language trees.
13. Check that scaffold templates or scaffold examples stay bounded and legible rather than quietly becoming an uncontrolled corpus program.
14. Check that a lineage pilot, when present, stays bounded rather than turning into hidden wider expansion.
15. Check that manual corpus-entry gates, when present, keep bounded source entry distinct from wider expansion.
16. Check that tiny-entry routes, when present, stay tree-first, keep capsule and authority distinct, keep fallback inside ToS, and stay bounded to one real authored node plus one bounded hop.
17. Check that uncertainty, ambiguity, or contestability did not get flattened into false certainty.

## Checklist

Answer each item with `yes`, `no`, or `not-applicable`.

- Does the changed surface keep a visible path back to source or authoritative meaning?
- Are node layers still distinguishable rather than collapsed into one summary voice?
- Are lineage relations still explicit where the subject needs them?
- Are temporal, spatial, civilizational, or interpretive contexts still attached where they matter?
- Is ToS still clearly authored truth rather than a restatement of a derived KAG or operational AoA surface?
- If practice lineage is mentioned, does it stay conceptual rather than absorbing operational ownership?
- If counterpart mapping is mentioned, does it stay derived, optional, and explicitly non-identity?
- If context compost is mentioned, do source refs, review state, and decay or demotion posture remain visible?
- If calibration is mentioned, does it guide curation without becoming a monopoly of meaning?
- If AI amplification is mentioned, does human judgment remain the owning review layer?
- If growth is mentioned, are node deepening, node creation, and branch formation still distinguishable?
- If node IDs are mentioned, do they follow a stable, readable public grammar?
- If multilingual witnesses are present, do they keep one shared node_id rather than language-split copies?
- If multilingual witnesses are present, does the canonical source remain visibly authoritative over the other witness layers?
- If Russian or English witnesses are present, do they remain witness layers rather than a new authored canon?
- If templates or examples are mentioned, do source-node and concept-node scaffolds stay distinct?
- If multilingual source entry is present, do concept nodes remain language-neutral rather than becoming synonym piles?
- If scaffold examples are present, do they remain bounded scaffolds rather than pretending to be a real branch pilot?
- If a lineage pilot is present, does it make lineage more legible rather than more graph-like and vague?
- If a lineage pilot is present, does it stay visibly smaller than a wider world-thought expansion wave?
- If lineage-pilot examples are present, do they stay visibly bounded rather than quietly posing as a wider expansion wave?
- If manual corpus-entry gating is present, does it keep bounded active source entry distinct from wider expansion?
- If a tiny-entry route is present, does `README.md` remain the current public `tos-root` rather than being replaced by a derived surface?
- If a tiny-entry route is present, do capsule and authority remain distinct rather than collapsing into one interchangeable surface?
- If a tiny-entry route is present, does fallback stay inside ToS rather than pointing to `aoa-kag`, `aoa-routing`, or another downstream repository?
- If a tiny-entry route is present, does it stay bounded to one real authored node plus one bounded hop?
- Does the change preserve plurality rather than forcing every branch through one interpretive flattening?
- Are any new abstractions reversible and narrower than the material they summarize?
- Does the change name uncertainty honestly where the source or interpretation remains contested?

## Stop conditions

Pause and revise before merge if any answer is `no` for:

- source traceability
- authored versus derived distinction
- node layering
- lineage preservation
- ToS versus AoA ownership boundary
- calibration flattening
- human-review sovereignty
- identifier drift
- language-split node copies
- source-authority replacement by translation
- template collapse between node families
- pilot-boundary collapse
- manual-entry boundary collapse
- tiny-entry capsule/authority collapse
- downstream authority bleed into ToS entry

## Review note

A short review note should record:

- what changed
- which checklist items were most at risk
- what remained interpretive or unresolved
- whether follow-up belongs in `aoa-kag`, `Agents-of-Abyss`, or another neighboring repository
