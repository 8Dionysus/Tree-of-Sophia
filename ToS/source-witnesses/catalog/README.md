# Source-Witness Catalog

This directory is the tracked, generated navigation index over authoritative
object records in the speaking `agents/`, `works/`, and `collections/` trees.

| File | Record class |
| --- | --- |
| `agents.jsonl` | people and organizations |
| `works.jsonl` | intellectual works |
| `expressions.jsonl` | language/textual responsibility states |
| `editions.jsonl` | published/edited manifestations |
| `collections.jsonl` | aggregate publications |
| `items.jsonl` | acquired physical/digital copies or containers |
| `catalog.manifest.json` | counts, paths, digest, and generation boundary |

Object records own identity and descriptive claims. These JSONL files are
rebuildable indexes and include each source record path and canonical digest.

Regenerate or check with:

```bash
python scripts/build_source_witness_catalog.py
python scripts/build_source_witness_catalog.py --check
```

Use `python scripts/validate_source_witness_foundation.py` for schema,
reference, companion, fixity, and optional local-payload checks. A green result
does not review the truth of metadata, rights, OCR, translation, or semantics.
