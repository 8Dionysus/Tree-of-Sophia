# AGENTS.md

This file applies to Notion witness surfaces for `ToS/philosophy/`.

## Role

This directory records Notion as a source witness for the philosophy domain
branch. It preserves page identity, ancestor route, source timestamps, and
child-page pointers without letting Notion UI labels become repository
topology.

## Rules

- Keep source page names in metadata, not in path names.
- Do not use this directory as the object home for philosophy branches.
- Route branch-shaped philosophical material into `ToS/philosophy/`.
- Route canonical authored nodes and relation packs into `ToS/canon/` only
  after review.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
```
