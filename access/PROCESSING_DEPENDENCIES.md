# Retained processing dependency navigation

The offline `ProcessingScheduler` records a DAG of evaluated `Input` and `Task`
identities. `tos_access.processing_closure.processing_dependency_closure`
provides read-only reverse navigation of **one explicitly named completed run**.
The scheduler's explicit cache initialization installs the reverse index; an
older cache without that index is refused by the reader, never migrated during
a query.

```python
import sqlite3
from pathlib import Path
from tos_access.processing_closure import processing_dependency_closure

cache = Path("/explicit/offline/normalization.sqlite")
db = sqlite3.connect(cache.resolve().as_uri() + "?mode=ro", uri=True)
try:
    db.execute("BEGIN")
    impact = processing_dependency_closure(db, exact_completed_run_id,
                                          ["source-node:philosophy:example"])
finally:
    db.close()
```

The caller owns the read transaction and its lifetime. The function neither
commits nor rolls it back. The packet names the run, changed identities,
reachable task/input identities with output digests, and reverse dependency
edges. Multiple paths or seeds do not duplicate a reachable node. Unrelated
inputs are not read or reported as removed. A retained historical run remains
historical even if a newer publication exists.

Default refusal limits are 4096 visited identities, 16384 dependency edges,
4096 UTF-8 bytes per identity, and 1 MiB for the compact UTF-8 result. Each reverse
lookup is an indexed seek with a remaining-budget limit; overlarge identifiers
and digests are excluded by SQL projections before delivery to Python. Work is
bounded by the selected closure, not a count/diff of all tasks. Malformed or
cyclic retained evidence, missing seed identities, retired/incomplete runs and
exhausted budgets raise an exception. **No partial result is a complete closure.**

This operation does not prove that the normalizer recorded all dependencies
needed by the source owner. In particular, independent context/finalization
`Input` records are not magically connected to their source tasks. The owner
still must supply complete source-to-final-carrier mappings, including Claim,
assessment, rights, placeholder and topology dependencies. A newly added source
identity absent from the retained run needs that owner's explicit mapping; it
does not receive a guessed empty impact set.

The result does not evaluate tasks, authorize partial execution, publish a
prepared snapshot, certify global semantic invariants, or confer acceptance.
`ProcessingScheduler.finish()` still describes a complete visited graph, not a
partial-update API. Source change detection, dependency completion, normalization,
validation and publication remain separate obligations.

Focused checks: `PYTHONPATH=access/src:access/tests python -m unittest
test_processing_closure`. Existing processing tests retain ownership of full-run
evaluation, cache reuse, failure recovery and publication conflict behavior.
