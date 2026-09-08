# Documentation validation

Use the nearest documentation owner first. Internal route and currentness
commands remain sourced from `docs/validation/validation_lanes.json`.

```bash
python scripts/validation_lanes.py --run cross_corpus_documentation
```

The repository route-docs sequence is owned by [root `VALIDATION.md`](../VALIDATION.md#run).

Decision-record mutation and index regeneration use
the owner builder directly:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

Decision indexes are generated read models; passing parity does not accept the
decision. Release publication uses `docs/RELEASING.md` and remains distinct
from local validation.
