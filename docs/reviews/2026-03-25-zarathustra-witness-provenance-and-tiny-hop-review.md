# 2026-03-25 Zarathustra Witness Provenance and Tiny-Hop Alignment Review

## What changed

- added bounded witness-provenance fields to the public source-node contract, schema, and Zarathustra worked example
- added optional segment `locator` support for multilingual witness slices
- updated `docs/NODE_CONTRACT.md`, `docs/SOURCE_NODE_TEMPLATE.md`, and `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md` so source witness, published translation, and maintainer-curated witness can stay distinguishable
- updated `docs/REVIEW_CHECKLIST.md` to test witness-provenance legibility during manual review
- aligned tiny-entry doctrine and public schema around `bounded_hop`
- kept `lineage_or_context_hop` as a legacy compatibility alias in the tiny-entry schema and example during transition

## Most at-risk checklist items

- source traceability under multilingual witness growth
- German source authority being blurred by anonymous translation convenience
- maintainer-curated witness layers being mistaken for published translation canons
- tiny-entry doctrine and schema drifting into two different public vocabularies
- downstream consumers breaking on hop-field rename instead of moving through a bounded compatibility window

## Review result

- `yes` multilingual witness blocks still keep one shared `node_id`
- `yes` the canonical German source remains visibly authoritative over the Russian and English witness layers
- `yes` witness provenance is now explicit enough to tell canonical source, working witness, and bridge witness apart without pretending to a full bibliographic apparatus
- `yes` shared segment locators make the bounded slice easier to trace without opening a broader citation program
- `yes` the tiny-entry route now names `bounded_hop` explicitly in doctrine and public example
- `yes` the legacy `lineage_or_context_hop` label remains bounded to compatibility rather than reasserting itself as the primary public field
- `yes` generated KAG export outputs remained up to date under `python scripts/validate_kag_export.py`

## What remains interpretive or unresolved

- the Russian and English witnesses remain maintainer-curated review layers rather than canonical published translation claims
- the shared locators are bounded route markers rather than a critical-edition citation apparatus
- the legacy tiny-entry alias should later be removed once downstream consumers no longer depend on it

## Intentionally deferred

- wider world-thought expansion
- additional Zarathustra source bundles beyond the current 2-block opening slice
- stricter public validation beyond the current manual review route
- downstream `aoa-routing` or `aoa-kag` cleanup after the compatibility window closes

## Likely downstream follow-up

- later `Tree-of-Sophia` cleanup pass to remove `lineage_or_context_hop` after transition
- later `aoa-routing` follow-up if any consumer still reads the legacy hop key directly
