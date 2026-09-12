# Joined prepared maintenance

`tos_access.prepared_semantics` composes the existing semantic index, exact
catalog and prepared publisher in one caller-owned SQLite transaction. It is
an offline owner API, not an access request or source command. It never starts,
commits, rolls back, or closes that transaction and never activates a reader.
On **any** exception, the caller must roll back the entire transaction.

## Explicit bootstrap and addressed changes

Before source-to-normalized assembly, an owner may capture the existing
neighborhood from an explicitly selected local prepared reader:

```python
from tos_access.prepared_neighborhood import capture_prepared_neighborhood
existing = capture_prepared_neighborhood(reader, entity_id="tos.agent.example")
# Alternatively: node_ids=[exact_normalized_id, ...], not aliases.
```

One read transaction selects all representations of that exact entity, all
their incoming and outgoing relations, and all opposite endpoint full rows.
Self-loops and shared edges appear once. Exact emitted row digests, indexed
identity fields, source-order tokens, physical index layouts and the selected
publication are checked. Existing reader row/byte/VM limits and post-read
concurrency/ABA checks apply. `NeighborhoodLimits` additionally caps seed,
node and relation counts; refusal returns no partial neighborhood. No full
graph load, missing-index repair, normalization or source write is performed.

This verifies existing seed incidence in an owner-produced prepared snapshot,
not new source membership, source authority, or claim/reducer dependencies.
Opposite endpoints are returned but their other relations are not expanded.
Checksums detect drift in selected rows, not a malicious/coordinated rewrite
of the store and its digests; producer integrity remains a precondition.
The subsequent writer must compare the captured binding again. A source
command may introduce new versions and memberships absent from this snapshot;
the source owner must assemble and verify them separately.

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

The bounded [source-cohort normalization](contracts/source-assembly-normalization.v1.md)
adapter reuses full-builder kernels for multiple source nodes, Claim traces and
their relations. Its supplied input closure must be established by the source
assembler; a candidate does not prove complete dependency discovery.

The private [source-root pairing](contracts/prepared-source-binding.v1.md)
wrapper can select immutable source projection roots in this same caller
transaction. It stores the exact roots beside the final prepared binding,
without replacing a second current-root file. Source assembly, complete
dependency checks and precommit source guards remain separate obligations.

The [source dependency index](contracts/prepared-source-dependencies.v1.md)
adds exact reverse Claim selection. Its declarations bind concrete source slots
and an executable declaration profile; the index does not infer those slots.
`apply_dependency_bound_prepared_delta_transaction` in
`tos_access.prepared_source_publication` stages those declarations, invokes the
source-root/semantic/catalog/prepared join, and verifies the final dependency
binding in the same caller transaction. It reserves the finalizer's writes
before the publisher runs and accounts for all its SQLite mutations. Each
lane retains its separate read/VM envelope. Its explicit progress-handler owner
restores the caller's handler after success or failure.

This join still requires a source assembler: capture before-images while they
are current, execute the authorized source command, derive the complete changed
cohort and verify source guards before committing the prepared transaction.
A committed source correction is not silently undone when derived publication
fails. The previous reader remains selected until an exact retry/reconciliation;
source commit and prepared commit are not one cross-file atomic transaction.

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
