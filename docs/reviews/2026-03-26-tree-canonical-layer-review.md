# 2026-03-26 Tree Canonical Layer Review

## What changed

- added `sources/`, `intake/`, and `tree/` as explicit repository layers
- seeded `sources/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/` with
  the current trilingual Zarathustra donor witness file
- seeded `tree/` with the canonical authored Zarathustra source node plus the
  current `becoming` and `overcoming` concept companions
- reframed `examples/` as the current public compatibility seam rather than the
  canonical home of the authored tree
- added tree-to-example sync validation and kept the current tiny-entry and
  tiny-export path stable
- updated core doctrine notes so source, intake, canonical tree, compatibility
  surfaces, and derived export remain distinct

## Most at-risk checklist items

- raw source, candidate intake, canonical tree, and public compatibility
  surfaces collapsing into one layer
- `examples/` remaining a silent second canon after `tree/` becomes canonical
- multilingual witness deepening widening into language-split node copies
- downstream KAG-facing language drifting into ToS authority doctrine
- the new repository layers reading as storage mechanics instead of a real
  authored tree posture

## Review result

- `yes` `sources/` stays source-facing and does not become node law
- `yes` `intake/` stays explicitly provisional and does not become source
  authority
- `yes` `tree/` now acts as the canonical authored layer for the bounded
  Zarathustra route
- `yes` `examples/` remains the public compatibility seam and is checked
  against `tree/`
- `yes` the current route still keeps one shared `node_id` across multilingual
  witnesses
- `yes` Russian and English remain Dionysus witness layers rather than a second
  canon
- `yes` the current tiny-entry and export seam remains path-compatible for
  downstream consumers

## What remains deferred

- broader migration of older scaffold examples into canonical `tree/` homes
- a machine-readable raw-source-file reference field in the node schema
- real raw-table intake population under `intake/`
- a future KAG candidate-intake contract once raw tables actually begin to land

## Boundary note

The new repository layers make ToS more visibly tree-bearing without turning it
into a source-duplicating archive or a KAG-owned substrate.

Canonical authored authority now lives in `tree/`.
Public compatibility and downstream-safe export remain explicitly subordinate to
that layer.
