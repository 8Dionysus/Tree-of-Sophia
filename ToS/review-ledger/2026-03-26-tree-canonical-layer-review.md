# 2026-03-26 Tree Canonical Layer Review

## What changed

- added `ToS/source-witnesses/`, `ToS/candidate-intake/`, and `ToS/canon/` as explicit repository layers
- seeded `ToS/source-witnesses/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/` with
  the current trilingual Zarathustra donor witness file
- seeded `ToS/canon/` with the canonical authored Zarathustra source node plus the
  current `becoming` and `overcoming` concept companions
- reframed `ToS/public-compatibility/` as the current public compatibility seam rather than the
  canonical home of the authored tree
- added tree-to-example sync validation and kept the current tiny-entry and
  tiny-export path stable
- updated core doctrine notes so source, intake, canonical tree, compatibility
  surfaces, and derived export remain distinct

## Most at-risk checklist items

- raw source, candidate intake, canonical tree, and public compatibility
  surfaces collapsing into one layer
- `ToS/public-compatibility/` remaining a silent second canon after `ToS/canon/` becomes canonical
- multilingual witness deepening widening into language-split node copies
- downstream KAG-facing language drifting into ToS authority doctrine
- the new repository layers reading as storage mechanics instead of a real
  authored tree posture

## Review result

- `yes` `ToS/source-witnesses/` stays source-facing and does not become node law
- `yes` `ToS/candidate-intake/` stays explicitly provisional and does not become source
  authority
- `yes` `ToS/canon/` now acts as the canonical authored layer for the bounded
  Zarathustra route
- `yes` `ToS/public-compatibility/` remains the public compatibility seam and is checked
  against `ToS/canon/`
- `yes` the current route still keeps one shared `node_id` across multilingual
  witnesses
- `yes` Russian and English remain Dionysus witness layers rather than a second
  canon
- `yes` the current tiny-entry and export seam remains path-compatible for
  downstream consumers

## What remains deferred

- broader migration of older scaffold examples into canonical `ToS/canon/` homes
- a machine-readable raw-source-file reference field in the node schema
- real raw-table intake population under `ToS/candidate-intake/`
- a future KAG candidate-intake contract once raw tables actually begin to land

## Boundary note

The new repository layers make ToS more visibly tree-bearing without turning it
into a source-duplicating archive or a KAG-owned substrate.

Canonical authored authority now lives in `ToS/canon/`.
Public compatibility and downstream-safe export remain explicitly subordinate to
that layer.
