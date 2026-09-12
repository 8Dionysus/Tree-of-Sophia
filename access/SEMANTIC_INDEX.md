# Bounded auxiliary semantic index

`tos_access.semantic_index` is an explicit offline, prepared-store companion.
It computes the existing `validate_knowledge_semantics` report; it does not
accept source, semantics, rights, canon, or publication. It never assembles
sources, selects a consumer binding, begins/commits/rolls back a transaction,
or invokes a hidden full-graph fallback.

The full validator remains available. Its node, endpoint, relation, and Claim
kernels and assertion-scoped maximum helper are shared with the addressed
path. The extraction **changes the normalization processor fingerprint**.
Existing publications/cache bindings are not silently migrated; an explicit
owner bootstrap with the new processor is required. Existing or historical
full-run evidence is not invalidated or relabeled as this new profile.

## Existing prepared bytes and order

The auxiliary tables store exact carrier digests, source-order tokens,
registry/projector/normalization dependencies, reverse validation dependencies,
Claim winner candidates, assertion-scoped counters, per-row report counts,
and diagnostic fragments. They do not duplicate normalized rows or catalog
payloads. Rows are loaded by exact ID from the existing prepared store and
checked against both emitted-row metadata and the semantic index digest.

`prepared_documents.source_order` is the owner order, independently for nodes
and relations. It is not `doc_id`, search rank, native-ID lexical order, or
catalog canonical order. Bootstrap requires unique, nonnegative safe-integer
tokens per kind. `PreparedChange.source_order` supplies the same token to both
lanes: insertion requires an explicit token; update `None` preserves it;
explicit update reorders. Simultaneous order swaps are supported.

The index profile requires canonical nonempty IDs whose value is unchanged
by the full validator's `_string` trimming. Otherwise two different prepared
primary keys could become the same normalized lookup ID. Such a baseline or
change is explicitly refused, not normalized or silently given a different
report. The full owner route remains available for these unsupported inputs.
The literal registered relation ID `<missing relation id>` retains the full
wrapper's diagnostic. Stable IDs and these mechanical checks are not authority.

## Caller-owned transaction API

```python
from tos_access.semantic_index import (
    SemanticIndexLimits,
    bootstrap_semantic_index_transaction,
    apply_semantic_delta_transaction,
    verify_semantic_index_binding_transaction,
)

# Existing prepared snapshot; its exact full report is already in the header.
report = bootstrap_semantic_index_transaction(
    db, binding=known_binding, entity_registry=entities,
    relation_registry=relations, ordered_rows=optional_row_factory,
    limits=semantic_limits,
)

# Later, in ONE caller-owned transaction, before prepared replacements:
report = apply_semantic_delta_transaction(
    db, expected_binding=old_binding, new_source_revision=new_revision,
    changes=prepared_changes, entity_registry=entities,
    relation_registry=relations, limits=semantic_limits,
)
# Caller places this exact report at header.counts.semantic_validation,
# updates catalog, and publishes the SAME changes through the prepared owner.
# That owner updates rows, row digests, search, lens, descriptor, header, epoch.
receipt = verify_semantic_index_binding_transaction(
    db, new_binding, limits=semantic_limits,
)
# Only the caller may now commit. Any exception requires complete rollback.
```

The optional bootstrap `ordered_rows(kind)` yields normalized dictionaries in
owner order. Every item is checked against the existing prepared row digest
and increasing prepared token; omission, substitution, duplicate tokens,
extra carriers outside the prepared map, and incomplete metadata refuse.
Without a supplied stream, the explicit full bootstrap collects only bounded
`(id, source_order)` metadata for ordering, never a complete graph. Existing
semantic tables are refused, not replaced. Bootstrap recomputes its report and
checks the exact report in the prepared descriptor; it never copies a `valid`
flag from a header or from supplied rows.

An addressed delta verifies the old binding and exact registry/projector
dependencies before writing auxiliary state. It prepares a **pending** state
for `expected_binding.publication_epoch + 1` and the explicit new source
revision. No new prepared descriptor is guessed: that descriptor contains
the new semantic report and belongs to the prepared publisher. Another delta
against pending state refuses. Final verification checks the actual prepared
binding, exact pending and loaded-dependency row digests/order, descriptor
digest, source revision, normalization, epoch, and computed report digest.
Only then does the auxiliary binding become final. A caller that catches an
error must still roll back the entire transaction, not continue another lane.

## Preserved semantic behavior

The report has exactly the full wrapper's fields, sorted/deduplicated
violations, integer counts, and ordered gaps: relation gaps in relation owner
order followed by Claim gaps in node owner order. Claim lookup is **last node
in owner order per `str(entity_id)`**, not an invented uniqueness constraint.
Reordering/removing that winner revisits its dependent relations. Node,
supporting-Claim, evidence, exact-version review, and Claim outgoing-edge
lookups record reverse dependencies, including unresolved node references.

Cardinality retains the exact registered relation scope. Reified relations
are grouped by raw `attributes.claim_ref`; other relations use one `None`
scope. Numeric/Boolean scalar equality matches Python `Counter`, including
`True == 1 == 1.0`; string `"1"` is distinct. Unhashable list/dict scopes refuse,
as the full validator cannot count them. Only the existing maxima are checked;
generic minima are not invented. Claim subject/object exactness remains its
separate kernel. No normalized graph-DAG/cycle invariant is invented: registry
hierarchy and supersession checks remain registry-owner semantics.

## Bounded work and refusal

`SemanticIndexLimits` is explicit and operation-wide: change count, rows read,
SQL calls, affected-row writes, read bytes, JSON input values/bytes/depth,
single carrier bytes, diagnostic generation and final report items/bytes,
and the **whole existing database** byte cap. Limits are not reset per carrier,
dependency, registry, diagnostic fragment, or posting group. The caller also
owns a combined whole-transaction budget across semantic/catalog/prepared
lanes; final verification is its own bounded API call.

Queries use primary/indexed exact lookups, indexed bounded ranges, or capped
bootstrap/report streams. There is no implicit SQL temporary sort or global
delta `COUNT/SUM` scan. Addressed deletion fanout is probed with a capped query
before a broad `DELETE`; SQL/write/row budgets include that probe. Stored JSON
byte lengths and cumulative read budget are checked before fetching/decoding
the payload. Input shape/depth/value/byte checks precede broad JSON encoding
and registry indexing; input bytes use a conservative JSON-escape upper bound
without allocating an encoded long string. Diagnostic collection is capped across kernel calls,
and final full-report serialization is independently capped by the same
ceiling. This cannot silently truncate gaps or return a partial report.

The helper lowers `PRAGMA max_page_count` to the stricter existing cap and the
declared whole-database cap; it never raises a retained cap. SQLite cap state
can outlive transaction rollback, so the caller must choose the shared storage
budget deliberately. No progress handler, connection-wide length limit,
journal mode, or caller callback is replaced. Explicit indexed-query and
returned-row bounds provide the SQL work envelope instead of a hidden VM
handler. Registry computation is bounded by the input envelope, not claimed
to be independent of registry size.

Large invalid graphs can have an unavoidably large full report even when one
row changed. Exceeding the output or dependency budget refuses; it never drops
diagnostics or calls a partial result current. Registry/normalizer/projector
changes require an explicit full rebootstrap; they are not addressed deltas.
Default limits are small-fixture defaults, not a full-corpus capacity promise.
No deployment, selection, full-corpus run, or new admission follows from green
local tests.
