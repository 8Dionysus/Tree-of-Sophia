# AGENTS.md

This card applies to `ToS/zarathustra/` and its nested route surfaces.

## Role

`ToS/zarathustra/` is the golden route branch for the current Tree of Sophia
axis. It holds the bounded *Thus Spoke Zarathustra* public-entry capsule and
route surfaces that orient the first real authored path through witness,
canon, bounded concept hop, and derived read models.

It is not a generic canon bucket, not a mechanics package, and not a source
witness store. Source text remains in `ToS/source-witnesses/`; canonical
authored nodes remain in `ToS/canon/`; operational process docs remain in
`mechanics/`.

## Operating Card

| Field | Route |
| --- | --- |
| role | golden Zarathustra route branch for the current ToS axis |
| input | bounded Zarathustra witness route, public-entry capsule, source/canon refs, and tiny-entry orientation |
| output | route surfaces that keep the current Zarathustra path legible without becoming source or canon authority |
| owner | `ToS/zarathustra/AGENTS.md` |
| next route | witness -> Zarathustra capsule -> canon source node -> bounded concept hop -> compatibility -> derived export |
| tools | `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`, `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`, source-home manifest, tiny-entry and KAG validators |
| check | `python scripts/validate_tos_source_home.py`, `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_kag_export.py` |

## Boundaries

- Keep Zarathustra route surfaces stronger than generic orientation, but lower
  than the actual source witness and canonical authored node.
- Do not move source text, translation witness material, or canon payloads into
  this branch.
- Do not move mechanics, schemas, generated payloads, or review checklists into
  this branch.
- Keep the first public route narrow: one bounded source route, one capsule,
  one source authority surface, and one bounded concept hop.

## Validation

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
python scripts/validate_nested_agents.py
```
