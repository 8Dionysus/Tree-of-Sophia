# AGENTS.md

This file applies to Notion witness surfaces for `ToS/philosophy/`.

## Role

This directory records Notion as a source witness for the philosophy domain
branch. It preserves page identity, ancestor route, source timestamps, and
child-page pointers without letting Notion UI labels become repository
topology.

## Operating Card

| Field | Route |
| --- | --- |
| role | Notion witness route for the philosophy domain tree |
| input | Notion page identity, title, ancestor path, source timestamp, and child-page pointers |
| output | witness metadata and page pointers that can feed `ToS/philosophy/` without becoming topology |
| owner | `ToS/source-witnesses/notion/philosophy/AGENTS.md` and `witness.manifest.json` |
| next route | witness metadata -> `ToS/philosophy/` branch -> graph workbench -> canon review |
| tools | Notion fetch/search, witness manifest, philosophy topology validator |
| check | `python scripts/validate_philosophy_topology.py` |

## Boundary Routes

- Keep source page names in metadata, not in path names.
- Route branch-shaped philosophical material into `ToS/philosophy/`.
- Route canonical authored nodes and relation packs into `ToS/canon/` only
  after review.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
```
