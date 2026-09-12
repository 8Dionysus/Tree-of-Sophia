# Joined prepared maintenance

`tos_access.prepared_semantics` composes the existing semantic index, exact
catalog and prepared publisher in one caller-owned SQLite transaction. It is
an offline owner API, not an access request or source command. It never starts,
commits, rolls back, or closes that transaction and never activates a reader.
On **any** exception, the caller must roll back the entire transaction.

## Explicit bootstrap and addressed changes

```python
from tos_access.prepared_semantics import (
    bootstrap_prepared_maintenance_transaction,
    apply_semantic_prepared_delta_transaction,
)

# Once, over an already published coherent normalized snapshot:
receipt = bootstrap_prepared_maintenance_transaction(
    db, expected_binding=binding, inputs=catalog_inputs,
    limits=publication_limits, catalog_limits=catalog_limits,
    semantic_limits=semantic_limits,
)

# Later, in a new caller-owned transaction:
receipt = apply_semantic_prepared_delta_transaction(
    db, expected_binding=binding, before_inputs=before_inputs,
    after_inputs=after_inputs, changes=prepared_changes,
    limits=publication_limits, catalog_limits=catalog_limits,
    semantic_limits=semantic_limits,
)
# Caller may commit only after successful return and its own source guards.
```

Inputs are [`CatalogInputs`](EXACT_CATALOG_INDEX.md) and
[`PreparedChange`](LOCAL_PREPARED_PUBLICATION.md). Bootstrap independently
recomputes the semantic report and catalog from selected prepared bytes and
requires exact agreement with the already selected header/catalog. It adds
auxiliary indexes without changing that publication or its binding. An optional
`ordered_rows(kind)` stream uses the semantic index's exact source-order checks.
Bootstrap is an explicit full pass, never a missing-index request-time fallback.

Delta captures a bounded, detached change sequence once, then:

1. Computes the exact semantic report using the addressed dependency index.
2. Replaces only `after_header.counts.semantic_validation` with that report.
3. Updates exact catalog contributors, final header counts, full rows, lens,
   search and publication epoch using the same captured changes.
4. Verifies the actual new descriptor, change sequence, dependency digests,
   source order and report against the pending semantic index before returning.

The returned `source_header` is the final header for the next transition.
Unknown header/count fields, native row values and the existing semantic
validator's ordered diagnostics are retained. A caller-supplied `valid` field
does not override the computed report. Neither a report with violations nor a
green report grants semantic acceptance; source/admission policy remains with
the source command owner. This API does not infer complete normalization
dependencies from a supplied replacement list.

## Budgets and compatibility

`PublicationLimits.max_mutations` counts actual SQLite changes across all three
lanes, including final semantic binding verification. Each later lane receives
only the remaining allowance. The semantic byte cap cannot exceed the declared
whole-publication cap; neither index may raise a stricter existing page cap.
The catalog and semantic APIs also enforce their own explicit input, dependency,
read and report bounds. Final semantic verification has its own read envelope;
the combined allowance here is for writes, not a claim that every independent
read budget is shared. Complete reports may require work proportional to all
reported gaps even for one changed row; no diagnostics are silently truncated.

Registry/normalizer/projector or incompatible search storage changes require
explicit bootstrap. This API does not migrate old publication profiles, take
source-file locks, replace selected projection roots, write a source journal,
or solve cross-file atomicity. The existing source `PublicationSnapshot`, exact
nonparticipating-file guards and source command authority remain obligations of
source-to-prepared integration. Returned flags explicitly remain
`source_transition_verified=false`, `semantic_acceptance=false` and
`consumer_switched=false`.

Focused checks: `PYTHONPATH=access/src:access/tests python -m unittest
test_prepared_semantics test_semantic_index test_prepared_catalog
test_prepared_publication`. Test inventory is descriptive; the standalone lane
manifest remains command authority.
