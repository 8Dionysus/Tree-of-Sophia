# ToS Examples

This directory holds public compatibility mirrors and reviewable example payloads for Tree of Sophia.

These files are not the canonical authored tree.
Canonical authority remains in `tree/` and the source-owned docs that explain the current route.

## Current role

Use `examples/` when you need:

- a public compatibility entry surface for the current tiny-entry route
- a reviewable example payload that mirrors a canonical tree surface
- a bounded public hop that stays aligned with the current route

Do not treat `examples/` as:

- a second canon
- generated runtime state
- a replacement for `tree/`

## Current bounded route

For the current public route:

- `source_node.example.json` mirrors the canonical source node
- `concept_node.example.json` mirrors the bounded concept hop
- `tos_tiny_entry_route.example.json` records the route from root to capsule, authority, and bounded hop

## How to verify

Use:

- `tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- `tree/concept/becoming/node.json`
- `docs/TINY_ENTRY_ROUTE.md`
- `docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md`
- `python scripts/validate_tree_example_sync.py`
- `python scripts/validate_kag_export.py`
