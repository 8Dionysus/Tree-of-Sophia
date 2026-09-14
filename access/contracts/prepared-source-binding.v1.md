# Private prepared source-root pairing

`tos_access.prepared_source_binding` stores exact immutable projection root
bytes, their part namespaces, source revision, participating source-publication
token and named dependency digests in the **same SQLite file** as normalized
rows. It does not turn a prepared reader into a source writer. These private
locators and bytes are not included in public access packets.

The source assembler constructs `PreparedSourceInputs(source_revision=...,
source_publication=..., dependencies=..., roots=...)`; `roots` maps stable
profile names to `ProjectionSnapshotView` values. The object retains only
immutable encoded bytes; `value()` and `roots()` return detached observations.
The participating token is null for an unversioned baseline or the exact
`sha256:` token, not a replacement for independent source-file guards. A source
revision is selected by the assembler's versioned identity contract, never
misrepresented as the legacy five-whole-input digest.

`bootstrap_prepared_source_inputs_transaction(db, expected_binding=...,
inputs=...)` explicitly creates the private `prepared_source_state` table.
It does not alter the prepared publication, attach semantic/catalog indexes,
verify source completeness or silently adopt an existing table. The caller
must first admit the baseline and retain all required immutable parts.

`read_prepared_source_inputs_transaction(db, expected_binding=...)` checks the
selected prepared descriptor and bounded state row, its stored digest, exact
root bytes and matching source revision. Root-file paths are namespaces only:
their currently selected file need not exist or contain those same bytes.
No selected-root-file read, part traversal or whole-source load occurs.

`apply_source_bound_prepared_delta_transaction` accepts the ordinary
`apply_semantic_prepared_delta_transaction` arguments plus exact
`before_source_inputs` and `after_source_inputs`. It verifies the private
predecessor, applies the semantic/catalog/row/lens/search transition, then pairs
the final binding with the successor roots and reads back that selection.
The root role set, namespaces and logical collection identities cannot change
through this route: those transitions require explicit bootstrap. The final
source revision must match the prepared successor header.

`bootstrap_prepared_source_root_extension_transaction` is a separate explicit
additive addressing transition. It accepts exactly one new root in a fresh
namespace, with a new source revision. All existing root bytes/namespaces,
dependency digests, publication token, catalog inputs and header meaning remain
unchanged. It supplies no normalized row changes. The ordinary delta route
still rejects root-role changes. This primitive verifies storage pairing only:
the stronger source owner must verify what the new root actually addresses.
Its receipt explicitly retains `source_root_admission_verified=false`.

`bootstrap_dependency_bound_source_extension_transaction` additionally pairs
the unchanged reverse-dependency declarations with this same transition; it
does not alter their membership. The selected Agent composition adds its own
context-state finalizer under the combined mutation allowance. Any failure,
including a finalizer failure, requires complete caller rollback.

All entry points require a caller-owned transaction. On **every** exception,
the caller must roll back the complete transaction, including its own work.
They neither commit, roll back nor close it. Before commit the source assembler
must recheck its exact source transition, nonparticipating dependencies and
source-owner locks/guards. There is no cross-filesystem/SQLite atomicity claim.
Staged immutable parts can remain after failure; no part deletion is authorized.

Because the selected roots and derived lanes commit together, another SQLite
reader sees either the old pairing or the new pairing. There is no second
mutable current-root JSON pointer to switch after the database commit. A later
reader switch still uses the exact independent prepared binding; this API does
not activate a consumer. Old readers fail their ordinary snapshot check.

The state is bounded to 1 MiB (or the narrower publication metadata cap), 16
named projection roots and 1024 named dependency digests. SQL masks malformed
or oversized state before delivery. The immutable projection format retains
its own root-size and descriptor checks. Source parts are **not** checked by
this pairing layer; even a well-formed root can name unavailable parts.
Whole-file limits include this private table. The combined mutation allowance
counts all semantic/catalog/prepared writes plus the final source-selection
write; its allowance is reserved before the inner kernel executes.

Receipts say `roots_paired_in_caller_transaction=true` only after successful
readback. They retain `source_transition_verified=false`,
`target_closure_verified=false`, `semantic_acceptance=false` and
`consumer_switched=false`. A caller-supplied source input object or green
storage test is not source authority, completeness or semantic admission.

This primitive is one prerequisite for source assembly. The
[source dependency index](prepared-source-dependencies.v1.md) has a joined
transaction wrapper that pairs explicit Claim declarations with these same
roots and prepared rows under one mutation budget. Neither layer implements
the addressed source-catalog migration, Claim/trace normalization closure or
end-to-end source command publication.
Focused tests cover concurrent visibility, retained root-file bytes, exact
predecessor checks, malformed state, combined limits and late-write rollback.

The separate [selected Agent composition](source-agent-publication.v1.md) now
connects those primitives to real source readers and selected-command evidence
for one explicitly bootstrapped descriptive profile. It does not expand the
authority or completeness guarantees of this storage-only pairing layer.
