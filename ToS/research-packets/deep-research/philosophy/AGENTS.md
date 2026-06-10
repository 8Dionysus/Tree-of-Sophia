# AGENTS.md

This file applies to Deep Research packet metadata for `ToS/philosophy/`.

## Role

This directory records a non-authoritative AI-generated research packet that
can help grow the philosophy domain branch. It preserves capture identity and
child-page pointers without treating Notion, GPT output, or UI labels as
philosophical source authority.

## Operating Card

| Field | Route |
| --- | --- |
| role | non-authoritative research packet route for the philosophy domain tree |
| input | AI-generated research scaffold metadata, capture page identity, titles, and child-page pointers |
| output | research packet metadata that can feed branch review without becoming source, canon, or doctrine |
| owner | `ToS/research-packets/deep-research/philosophy/AGENTS.md` and `research.manifest.json` |
| next route | research packet -> `ToS/philosophy/` branch review -> source anchoring -> graph workbench -> canon review |
| tools | research packet manifest and philosophy topology validator |
| check | `python scripts/validate_philosophy_topology.py` |

## Boundary Routes

- Keep capture page names in metadata, not in path names.
- Cite source authority from real source witnesses, authors, works, editions,
  translations, or reviewed canon surfaces.
- Route branch-shaped philosophical material into `ToS/philosophy/`.
- Route claims that need authority to real source witnesses, works, editions,
  translations, or reviewed canon surfaces.
- Route canonical authored nodes and relation packs into `ToS/canon/` only
  after review.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
```
