# Selected Agent to prepared publication

`scripts/source_agent_publication.py` is an **offline**, explicit source-owner
composition. It does not add a query-time compiler, CLI default, watcher,
deployment, source authority or consumer switch. It implements one existing
native Agent's immediate selected descriptive correction against an explicitly
bootstrapped prepared predecessor. It is not an adoption route for an arbitrary
legacy full snapshot.

## Independent source identity

`source_vector_inputs` derives a revision from a canonical descriptor containing
`schema=tos_agent_source_root_vector_v1`, the exact role-to-root digest map, the
participating publication token and named dependency digests. Required roles
are `source-catalog`, `source-navigation` and `bibliographic-claims`; additional
frozen inputs retain their explicit roles. Raw roots precede this revision and
cannot contain a derived `source_revision` in their headers. Absolute part
namespace paths are private locators, excluded from the revision descriptor.
Changing a namespace alone does not change the logical revision.

The navigation row root has only
`schema_version=tos_agent_source_navigation_rows_v1` in its header and keyed
`nodes(node_id)`/`edges(edge_id)` collections. The bibliographic row root has
`schema_version=tos_agent_bibliographic_rows_v1` and additionally
`claim_traces(claim_ref)`. Each collection explicitly orders by its key. These
are private independent row inputs, not whole public graph exports with stale
global counts or copied global input-digest headers. The source-catalog root
retains its existing `tos_source_catalog_projection_v2` contract.

Dependencies bind the declaration enumerator, orchestration/assembler/source
navigation/metadata reader/normalization implementations, both registries and
the normalization binding. `nonparticipating-profile` records the separately
admitted frozen input scope. Unchanged nonparticipating inputs are not freshly
recompiled or asserted globally current by this correction. A changed profile,
registry, raw-root role or normalization implementation requires an explicit
new bootstrap, not a copied legacy five-file hash.

The new full bootstrap uses
`build_source_navigation(..., catalog_snapshot=...)`. This selects the actual
catalog-backed metadata reader. Its per-record provenance contains catalog
namespace/profile, record key, row digest, exact source ref, raw bytes/digest
and record ref. It contains no whole-root digest or publication token. The
snapshot envelope and live source guards retain those currentness checks.
Legacy `build_source_navigation(...)` remains unchanged and retains its legacy
catalog-file provenance; the two profiles are not byte-equivalent.

## Explicit bootstrap and bounded closure

The prepared file first receives its full rows/search/lens, semantic/catalog
indexes, source roots and source dependency index through their existing
explicit bootstrap APIs. SQLite **WAL must already be selected**; this helper
does not change journal mode. WAL lets independent readers continue observing
the predecessor while the writer stages the successor. A file-only copy of an
active WAL database is not a valid backup or portable snapshot.

`bootstrap_agent_context_index_transaction` then checks the complete prepared
node/relation streams, exact dossier membership and every source-catalog Claim
against its real assembler enumeration and retained declaration. Node context
positions are the full builder's original encounter order, not hash traversal
or an inferred new-member order. It attaches only private derived lookup state
to that same file. This is full offline bootstrap; capture/publication never
call it implicitly.

The context bootstrap accepts optional, caller-declared full-corpus budgets for
catalog-part reads, Claim assembly and source-slot reads. Omitting them retains
the correction-safe defaults; a full caller must provide its own explicit
budgets rather than widening the delta/capture profile globally. The budgets
bound selected work only and do not make an unsupported historical Claim
producer available.

The bounded profile requires self-owned bibliographic Claim context groups and
reified bibliographic context-consuming relations outgoing from the governing
Claim. A nonincident context consumer or a foreign contribution requires a
different explicit profile. Literal nodes are Claim-scoped and must be rebuilt
when their governing Claim context changes even if their raw value is equal.
The index keeps exact context membership/order with per-node and complete-group
checksums. This is mechanical dependency coverage, not semantic admission.

`capture_agent_correction` obtains the existing source writer lock and a short
read transaction, checks the selected roots/profiles, verifies complete reverse
identity fanout, and assembles the real Agent history and dependent Claims
**before** the separately authorized `record.revise` command. It compares the
raw cohort and traces with the selected immutable roots. Missing declarations,
unresolved dependencies, stale bytes, omitted source rows and exhausted budgets
refuse capture. It neither executes nor authorizes the source command.

The correction permits `preferred_label`, `notes` and `field_languages` only.
Identity, source references, source membership, existing relation endpoints,
views and context membership cannot change. The selected command's exact
request/archive/current bytes and immediate predecessor publication remain
checked by the source owner. All retained history versions and their new
current/historical posture participate; they are not discarded to shrink a
delta.

The successor obtains complete changed-node incidence through the prepared
carrier's physical seek indexes, supplies unchanged endpoint/context rows and
all governing traces to the shared normalizer, and reproduces the exact old
prepared rows before accepting replacements. Support endpoints are not
recursively expanded. New history rows use sparse integer order between their
canonical `(source_graph,id)` neighbors. An exhausted gap refuses the whole
operation; no global relabeling or append-order guess is performed.

## Commit and failure boundary

After the source command, `agent_correction_publication` reacquires the existing
source writer lock and keeps it until scope exit. The caller begins a SQLite
transaction, calls `apply_transaction`, then explicitly calls
`commit_transaction` inside that scope. The latter rechecks actual source
readers and retained transaction evidence immediately before committing.
The caller must not execute additional DML or DDL after `apply_transaction`;
guarded commit refuses a changed SQLite mutation counter or main schema version.
The schema check covers ordinary index/table/trigger changes, which do not
increment the mutation counter; it is not a sandbox for hostile SQL callers.
Returned receipt dictionaries
are detached observations, not mutable commit authority. Private context tables
must retain their exact declared schema without additional triggers.

All source roots, unchanged/changed declarations, full normalized rows,
semantic report, catalog, lens, compressed search and private context binding
advance in one SQLite transaction. No second mutable root JSON is selected.
Old readers see the predecessor until commit and reject their stale binding
afterward. No live consumer is automatically rebound.

Every failure requires the caller to roll back **the complete SQLite
transaction**, including its own earlier writes. Immutable staged parts may
remain unselected; this API never deletes them. The already committed source
command is not rolled back. Its exact transaction/token and captured
predecessor form an explicit retry condition while that source successor stays
current. Source and prepared commits are not cross-filesystem atomic.

The default orchestration bounds are 64 reverse Claims, 512 closure nodes,
1024 relations and 16 MiB cohort material. Source readers, COW mutation,
normalization and each SQLite lane retain their separate limits. These are
declared work bounds, not a claim of bounded RSS or latency for every corpus.
The supplied publication mutation cap includes the context-binding finalizer;
source filesystem staging is accounted separately. Unsupported or oversized
closure fails closed without full-build fallback.

Receipts separate source-command commit, prepared commit, bounded descriptive
closure and semantic report. They grant no textual/semantic acceptance,
rights, consent, canon, global source currentness or runtime health. Focused
tests use real source schemas/commands in a tiny disposable corpus, independent
full new-profile normalization, concurrent readers and late-failure rollback.
