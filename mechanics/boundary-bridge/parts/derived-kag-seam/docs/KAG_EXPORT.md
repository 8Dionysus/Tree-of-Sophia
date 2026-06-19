# ToS KAG Export

This document records the current source-owned tiny KAG export posture for Tree
of Sophia.

The export is deliberately narrow.
It exposes one bounded source-node capsule for downstream KAG consumers without
replacing ToS-authored authority.

## Current pilot

The current pilot stays on the Zarathustra prologue route only:

- one exported object: `tos.source.thus-spoke-zarathustra.prologue`
- one canonical authored source node: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- one public compatibility entry surface: `ToS/public-compatibility/source_node.example.json`
- two supporting doctrine surfaces:
  - `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
  - `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- one compact consumer export: `ToS/derived-exports/kag_export.min.json`

## Core rule

The export is a source-owned guide surface, not a new authority layer.

It may expose a bounded question, summaries, interpretation-layer handles, and
current route refs for downstream consumption, but authored ToS authority
remains in the canonical tree node and its supporting ToS docs, while the public
entry surface remains a compatibility mirror for the current tiny-entry seam.

## Current files

- `ToS/derived-exports/kag_export.json`
- `ToS/derived-exports/kag_export.min.json`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py`
- `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`

If you edit supporting surfaces in `docs/`, `ToS/public-compatibility/`, `ToS/derived-exports/`, `ToS/contracts/`, or `scripts/`, also follow the nested `AGENTS.md` in that directory.

## Current verification

For the current bounded export seam without regeneration, use:

```bash
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
python -m unittest discover -s tests
```

`python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` checks generated export parity and
payload structure for this seam.
The release lane pairs it with canon, intake, route-card, and public-entry
validators when broader route assurance is needed.
`python -m unittest discover -s tests` strengthens repo-local contract and
schema coverage around that same bounded route.

## Regeneration

If you change export inputs or generation logic, use:

```bash
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
python -m unittest discover -s tests
```
