# Mode B Tabular Base Pack

This directory holds the current `v6.1`-aligned candidate tabular base pack for
the bounded `thus-spoke-zarathustra/prologue-1` route.

## Status

- status: `candidate intake only`
- pack format: `CSV + README`
- authority: none of these files are canonical ToS tree law
- workbook role: the root `v6.1` workbook remains a carrier, not a second canon

Primary witness material remains in `sources/`.
Canonical authored source-node law remains in `tree/`.
Nothing in this directory promotes itself automatically.

## File Manifest

The current bounded base pack consists of exactly these files:

- `corpus_map.csv`
- `witnesses.csv`
- `segments.csv`
- `nodes.csv`
- `event_state_nodes.csv`
- `edges.csv`
- `translation_tensions.csv`
- `witness_glosses.csv`
- `principles.csv`

Master and coverage views are intentionally not tracked here as primary intake
surfaces.

## Active Spine

- `source_secondary` stays in the form `1,1,1,n`
- `paragraph_anchor` stays in the form `[n]`
- `segment_id` now follows the active machine spine `seg.1.1.1.n`
- older `zv1-*` route labels survive only as human-facing `working_name` memory
- already-canonical ToS ids may be reused where they already exist
- candidate meaning nodes use `n.*`
- candidate events and states use `ev.*` and `st.*`
- principles use `pr.*`
- literal rows required for FK closure use `literal.*`

Fixed witness ids for this bounded pack are:

- `w.de.nietzsche.canonical_source.v1`
- `w.ru.dionysus.working_translation.v1`
- `w.en.dionysus.bridge_translation.v1`

## Explicit Workbook Translation

The root workbook is treated as a carrier and review scaffold.
This directory carries the repo-owned translation of that carrier into ToS
surfaces.

That means:

- `22_Канон_контракт` is translated into repo docs and validators rather than
  treated as canon by itself
- `15_Master_*` and `16_Master_*` remain serving and review views rather than
  primary tracked intake files
- `17_nodes_actual`, `18_event_state_nodes_actual`, and `19_principles_actual`
  seed the candidate tables here, but do not become primary repo surfaces on
  their own
- `20_predicates_registry` and `21_class_registry` are promoted into
  `tree/registries/` as governance surfaces

## Promotion Rule

Material in this directory may inform later canonical work, but it does not
become canonical by presence alone.

Promotion requires a separate review pass that explicitly decides what moves
from `intake/` into `tree/`.

Some status fields may therefore record review outcomes such as `promoted` or
`deferred_commentary` without turning this directory itself into canonical tree
law.

For the current dynamic ledger, that now means:

- all 18 `event` rows in `event_state_nodes.csv` are marked `promoted`
- all 9 `state` rows in `event_state_nodes.csv` are marked `promoted`
- `ev.p5.bee_honey_analogy` remains in `intake/` as `deferred_analogy`

For the current support ledger, that now means:

- 19 core route-bearing `n.*` rows in `nodes.csv` are marked `promoted`
- `literal.ten_years` and `literal.too_much` remain in `intake/` as
  `deferred_literal`
- all other `n.*` rows remain in `intake/` as `deferred_residue`

For the current edge ledger, that now means:

- 89 `edges.csv` rows are marked `promoted`
- 33 rows remain in `intake/` as `deferred_residue`
- 3 rows remain in `intake/` as `deferred_literal`
- 2 rows remain in `intake/` as `deferred_analogy`
- 1 row remains in `intake/` as `deferred_commentary`

Those promoted rows now mirror into the route-local canonical relation pack at
`tree/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv`
while this directory stays visibly provisional.
