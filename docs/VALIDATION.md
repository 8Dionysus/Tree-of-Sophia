# Documentation validation

Use the nearest documentation owner first. Internal route and currentness
commands remain sourced from `docs/validation/validation_lanes.json`.

```bash
tos-validation-lanes --repo-root "$PWD" --run cross_corpus_documentation
```

The repository route-docs sequence is owned by [root `VALIDATION.md`](../VALIDATION.md#run).

Decision-record mutation and index regeneration use
the owner builder directly:

```bash
tos-ops-mechanics-plan --repo-root "$PWD" --decision-index-build
tos-ops-mechanics-plan --repo-root "$PWD" --decision-index-build --check
tos-ops-mechanics-plan --repo-root "$PWD" --decision-records-validate
```

Decision indexes are generated read models; passing parity does not accept the
decision. Release publication uses `docs/RELEASING.md` and remains distinct
from local validation.
