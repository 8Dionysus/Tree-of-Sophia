# ToS KAG Export

This document records the current source-owned tiny KAG export posture for Tree
of Sophia.

The export is deliberately narrow.
It exposes one bounded source-node capsule for downstream KAG consumers without
replacing ToS-authored authority.

## Current pilot

The current pilot stays on the Zarathustra prologue route only:

- one exported object: `tos.source.thus-spoke-zarathustra.prologue`
- one canonical authored source node: `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- one public compatibility entry surface: `examples/source_node.example.json`
- two supporting doctrine surfaces:
  - `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`
  - `docs/TINY_ENTRY_ROUTE.md`
- one compact consumer export: `generated/kag_export.min.json`

## Core rule

The export is a source-owned guide surface, not a new authority layer.

It may expose a bounded question, summaries, interpretation-layer handles, and
current route refs for downstream consumption, but authored ToS authority
remains in the canonical tree node and its supporting ToS docs, while the public
entry surface remains a compatibility mirror for the current tiny-entry seam.

## Current files

- `generated/kag_export.json`
- `generated/kag_export.min.json`
- `scripts/generate_kag_export.py`
- `scripts/validate_kag_export.py`

If you edit supporting surfaces in `docs/`, `examples/`, `generated/`, `schemas/`, or `scripts/`, also follow the nested `AGENTS.md` in that directory.

## Regeneration

Use:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```

`python scripts/validate_kag_export.py` also checks the nested local guidance surfaces for the current tiny export seam.
