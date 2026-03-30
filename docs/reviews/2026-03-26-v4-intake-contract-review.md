# Intake Contract Review

Date: 2026-03-26

Historical note:
This review records the pre-`v6.1` intake contract before the route moved from
`zv1-*` segment ids to the current `seg.1.1.1.n` machine spine.

## What Changed

- replaced the `mode-b` intake stub with a real local contract manifest
- added the first `Wave 1` raw intake tables for
  `thus-spoke-zarathustra/prologue-1`
- kept the pack bounded to `corpus_map`, `witnesses`, `segments`, `nodes`,
  `event_state_nodes`, `edges`, and `translation_tensions`

## Highest-Risk Checklist Items

- `yes` `sources/`, `intake/`, and `tree/` remain visibly distinct
- `yes` the workbook audit scaffold remains outside the current route-local
  live carrier slot and does not become a second canon
- `yes` at that stage `segment_id` stayed aligned to the then-current
  canonical ToS route ids `zv1-*`
- `yes` raw tables stay candidate-only and do not promote into `tree/`
- `yes` no `aoa-kag` surface was changed or implied as ToS authority

## Review Notes

- `edges.csv` includes both source rows and bridge rows from the `v4` master
  sheet, but multi-anchor spans are kept only in `note`; the primary anchor is
  what is normalized into the explicit CSV columns
- `nodes.csv` normalizes workbook `literal.*` targets into candidate `n.*`
  nodes so that edge references remain inside the allowed `node_id` or `es_id`
  space
- second-wave tables and master control sheets remain intentionally deferred

## Remaining Interpretive Or Provisional Areas

- candidate node and event labels are still raw-pass authoring, not canonical
  ToS law
- bridge rows remain included as candidate reasoning structure, but they are not
  yet promoted to any canonical `tree/` relation layer
- future promotion into `tree/` should decide what survives as stable node law
  and what remains only intake scaffolding

## Neighbor Follow-Up

No follow-up belongs in `aoa-kag` yet.
The next move remains inside `Tree-of-Sophia`: review the intake pack against
the `v4` coverage target and then decide what, if anything, should be promoted
from `intake/` into canonical `tree/`.
