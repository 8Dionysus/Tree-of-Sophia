# AGENTS.md

This file applies to non-authoritative research packets under
`ToS/research-packets/`.

## Role

`ToS/research-packets/` holds AI-assisted, secondary, or provisional research
scaffolds that may help grow the philosophy tree. It does not hold source
witness authority, doctrine, canon, or final graph truth.

## Operating Card

| Field | Route |
| --- | --- |
| role | non-authoritative research packet holding area |
| input | AI-generated research scaffold, secondary synthesis, capture metadata, or unpacking notes |
| output | bounded packet metadata and reviewable leads for `ToS/philosophy/` |
| owner | `ToS/research-packets/AGENTS.md` and the nearest packet `AGENTS.md` |
| next route | packet -> philosophy branch review -> source anchoring -> graph workbench -> canon review |
| tools | packet manifests and philosophy topology validator |
| check | `python scripts/validate_philosophy_topology.py` |

## Boundary Routes

- Classify these packets as research leads, not source witnesses.
- Cite author, work, school, doctrine, and canon authority from source-owned
  or reviewed surfaces.
- Keep capture containers and UI titles as metadata only.
- Route philosophical structure into `ToS/philosophy/` only after branch review.
- Route authority claims to real source witnesses, published works, editions,
  translations, or reviewed canon surfaces.

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
```
