# V6.1 Carrier Migration Review

Date: 2026-03-27

## What Changed

- adopted the `v6.1` workbook as a carrier rather than a canonical repo surface
- migrated the bounded intake route to the 9-table tabular base contract
- replaced the active segment spine with `seg.1.1.1.n`
- moved graph vocabulary governance into tracked registries under `tree/`

## Review Notes

- `tree/source/.../node.json` remains the authored source-node canon
- `examples/source_node.example.json` remains a compatibility mirror rather than
  a second source of truth
- `intake/.../mode-b/*.csv` now carries the candidate tabular base pack
- `tree/registries/*.csv` now carries predicate and class governance surfaces
- the root workbook remains a carrier and review artifact, not repo canon

## Residual Boundaries

- `translation_tensions.csv` is now long-model and anchor-based, while the
  source-node JSON keeps the compact `{ segment_id, note }` surface
- `15_Master`, `16_Coverage`, and `*_actual` workbook sheets remain derivation
  and review views rather than tracked primary repo surfaces
- promotion from `intake/` into broader authored tree law still requires a
  separate review pass
