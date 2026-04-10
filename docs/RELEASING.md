# Releasing `Tree-of-Sophia`

`Tree-of-Sophia` is released as a source-first knowledge repository with a bounded public entry route.

See also:

- [README](../README.md)
- [CHANGELOG](../CHANGELOG.md)
- [REVIEW_CHECKLIST](REVIEW_CHECKLIST.md)

## Recommended release flow

1. Keep the release bounded to source-owned ToS meaning.
2. Update `CHANGELOG.md` so the latest tagged section keeps `Summary`, `Validation`, and `Notes`.
3. Run the repo-level verifier:
   - `python scripts/release_check.py`
4. Run federation release audit when the change is part of a wider owner-repo pass:
   - `aoa release audit /srv --phase preflight --repo Tree-of-Sophia --strict --json`
5. Publish only through `aoa release publish`.

## Validation path

`scripts/release_check.py` wraps the current bounded route battery:

- `python scripts/build_root_entry_map.py --check`
- `python scripts/validate_root_entry_map.py`
- `python scripts/validate_tiny_entry_route.py`
- `python scripts/validate_kag_export.py`
- `python -m unittest discover -s tests`
