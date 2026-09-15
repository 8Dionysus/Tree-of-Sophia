# Initial Claim to prepared publication

`scripts/source_claim_publication.py` composes the existing initial
`claims.create` owner package, addressed catalog addition, source assembler,
normalizer and prepared transaction lanes. It is offline and explicitly
selected; it does not create source, serve writes through access, bootstrap a
legacy reader or switch a running consumer.

## Selection and scope

`claim_addition_publication(owner_config, source_inputs=...,
expected_binding=..., catalog_inputs=..., expected_receipt_sha256=...,
expected_request_digest=..., progress_owner=...)` holds the existing source
writer lock for its scope. It inherits the exact initial identity-relation
and public metadata evidence boundary of `source_claim_catalog.py`. The source
command has already committed and is never replayed by this publisher.

The unchanged creation configuration is evidence of its scope at the exact
receipt's recorded instant, not a grant to execute another command now. As in
the [metadata publication route](source-metadata-publication.v1.md), natural
expiry does not invalidate the committed source package. Receipt hash, original
configuration/principal/authority/path, pre-expiry creation time and complete
current package are checked; future-dated evidence or a changed/revoked
configuration refuses. Writable command dispatch still checks current time.
Derived publication and consumer activation remain separately caller-owned.

The predecessor must have the admitted independent source-root vector,
canonical prepared row order, WAL, catalog/semantic/search/lens maintenance,
complete source dependency declarations and existing context index. No
missing index is silently attached. Catalog source/processor profiles and the
existing Agent publication profile remain checked. The first Claim addition
records its separate `claim-publication-profile` digest in the source vector;
later additions require that same implementation or an explicit migration.
Newly consumed Claim schema bindings are retained in the successor catalog.

The source assembler verifies exact current Claims, endpoint versions,
evidence, provenance and forms. Existing shared raw carriers must reproduce
their predecessor bytes; an unrelated source correction cannot hitchhike on
an addition. New Claims, edges, traces and declarations must be absent.
Unresolved source declarations or existing dependencies on a new Claim refuse
this profile rather than silently widening the transition.

## Closure and one transaction

All incidence of shared/new cohort nodes is selected through the prepared
physical seek indexes. Exact retained endpoint rows, governing traces and
context contributors are supplied to the shared normalizer. It must reproduce
the complete old affected normalized rows before it can construct successors.
This includes inherited views and the existing neighbors affected by a new
edge; reading one new Claim alone is not a closure check. Support endpoints
are not recursively expanded. Oversized closure refuses without a full-build
fallback.

Within one operation, checked immutable raw-part bytes are reused across
addressed lookups. Cache misses retain the original stored/decoded/key/part
budgets and the total cache is bounded by that decoded-byte allowance. Cache
keys include the full descriptor and hash prefix within one exact namespace;
each lookup still validates its descriptor and decodes a fresh value. This
does not cache live source files or bypass their final currentness checks.

Every new Claim owns an independent singleton context group. Existing group
membership and relative contributor order cannot change. The private context
index allocates new unique positions after its existing maximum: ordering
between these independent singleton groups has no reducer meaning and is not
claimed to be historical order or a new global source ordering policy.
Prepared row insertion independently retains the canonical sparse-order
algorithm and its refusal on an exhausted gap.

The caller begins a transaction, calls `apply_transaction`, then explicitly
calls `commit_transaction` in the same source-lock scope. The transaction
joins new raw roots and catalog, source declarations/reverse indexes, normalized
rows, semantic report, catalog/search/lens indexes, paired source inputs and
the new context groups. The complete mutation allowance reserves the context
finalizer before invoking inner lanes. Scope/processor/source bytes are
rechecked before commit; caller DML or schema changes after verified apply
refuse guarded commit. Returned observations are detached, not commit authority.

Independent readers continue seeing the predecessor until commit and reject
its stale binding afterward. Consumer switching remains an explicit next
step. On any failure, the caller rolls back the **entire** SQLite transaction,
including earlier caller writes. The source package survives; a failed
publication can be retried against the same still-current source and prepared
predecessor. A stale predecessor after a successful commit refuses rather
than inserting duplicates. Immutable staged parts may remain unselected; this
API never deletes them. Source and SQLite commits are not cross-filesystem
atomic.

Default cohort bounds are 512 nodes, 1024 relations, 512 traces and 16 MiB;
source creation still has its independent 32-Claim limit. Assembly, source
slot, COW, dependency, semantic and publication lanes retain their separate
limits and receipts. These are work/refusal bounds, not a whole-corpus latency,
RSS, disk-reservation or recovery-time promise. The host owns storage/resource
reservations, including backups and SQLite WAL growth.

The profile is not temporal/structured Claim insertion, correction, deletion,
assessment/revocation or a general identity membership migration. Those goal
requirements retain their own pending integration work. Green local tests,
mechanical source checks and prepared commit never grant meaning, admission,
rights, canon, deployment or whole-foundation completion.
