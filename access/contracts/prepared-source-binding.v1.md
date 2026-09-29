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


## Offline publication execution

### Native normalization implementation transition

The Rust joined owner exposes
`prepared_maintenance::transition_prepared_normalization_transaction` for an
explicitly reviewed compatible implementation change. Its
`ReviewedNormalizationTransition` names the exact old/new processor digests
and a nonempty review reference. The executing source owner independently
verifies the current native artifact identity; an arbitrary request digest or
the predecessor Python implementation digest is not that identity. Reuse an
already verified held artifact digest, or account for its complete read before
entering the transaction.

This route supplies no normalized row changes. Registry bytes, configuration,
lenses, source-order profile and header meaning remain unchanged; only the
normalization processor and source revision may move. Catalog contribution
state, prepared descriptor, search/lens selection and semantic state are paired
in the caller's transaction, with predecessor checks, CAS and final readback.
The ordinary delta APIs still reject a changed normalization binding.

The source owner must pair its source-root, reverse-dependency and context
selections before committing that same transaction. A returned maintenance
receipt alone is not permission to commit an unpaired source state. Every error
requires rollback of the entire caller transaction. Naming a compatibility
review does not prove unchanged algorithm semantics or admit source material.

This implementation transition does not adopt an old auxiliary projector by
rewriting its fingerprint. The catalog and semantic indexes must already have
the current native projector identities, obtained through their actual native
bootstrap or a separately implemented, reviewed migration. A changed registry,
normalization rule or source contract requires the corresponding real data
migration, not this compatibility route.

Before a controlled cutover, retain the predecessor software/data pair and its
immutable source parts. The new reader selects the returned new binding against
the same committed database. An old binding against that advanced database must
refuse; it is not a restore operation. Recovery selects the preserved compatible
pair through the serving owner's existing restore/clock procedure. These APIs
neither switch a running consumer nor authorize deletion of the predecessor.

Cost includes bounded metadata parsing and copies, catalog aggregate rendering,
semantic report/finalization work, search/lens resealing, source-owner finalizers
and the SQLite rollback journal. The combined mutation allowance covers all
lanes; “no row changes” does not mean zero work or a measured constant-time bound.

The maintained file owners `publish_prepared`, `publish_prepared_rows`, and
`apply_prepared_delta` select the installed Rust `tos-access` executor. Explicit
`native_executable` overrides `TOS_PREPARED_EXECUTOR`, which overrides installed
PATH discovery; a selected override must be absolute. Data/source selection never
selects executable code. Missing or unusable code refuses before target creation;
there is no build on call or Python fallback. A positive whole deadline is supplied
as `native_timeout` or `TOS_PREPARED_MAX_SECONDS`. The offline `prepare` consumer
also exposes `--native-executable` and `--max-seconds` and resolves both before
creating its output directory. This deadline covers the native publication, not
source assembly or the separate maintenance attachment.

Explicit `reference_publish_prepared`, `reference_publish_prepared_rows`, and
`reference_apply_prepared_delta` retain the independent Python implementations as
oracles. Live `sqlite3.Connection` transaction APIs remain distinct until their
complete catalog/semantic/prepared operation has a Rust-owned transaction; a
subprocess never receives ownership of a Python connection.

Donor progress uses the existing five report phases. The framed native command
emits bounded progress reports and waits for acknowledgement on a separate private
pipe before proceeding. Callback refusal returns through whole transaction
rollback and owned-new-file removal; the read-only donor is retained unchanged.
The final successor report says `committed:false`. Native acknowledgement waiting
uses the same absolute deadline. Python callbacks execute synchronously and must
cooperate with the caller deadline; an arbitrary blocking user callback is not a
bounded Python supervisor. Compound index carriers use Python-compatible nested
representation with the maintained foundation number codec and pinned Unicode16
printable categories; they do not become JSON strings or silently disappear.

## Joined offline maintenance ownership

`prepare` attaches catalog and semantic maintenance through one native file-owned
transaction after publication. It supplies the exact selected owner inputs and
normalization processor identity, checks its source state before the call, and
acknowledges the actual source check before native commit. Publication and attachment
retain separate named mutation budgets. The native projector records its own exact
implementation digest; a Python source digest remains normalization provenance,
never an identity for native code. Existing Python auxiliary projector state requires
explicit offline bootstrap/rebuild before native delta, with no hidden full rebuild.

Native transaction functions accept a real Rust `rusqlite::Transaction` and join
catalog contributor facts, semantic diagnostics and prepared publication. Whole
source pairing must include `prepared_source_state` in that same transaction.
Generic Python Connection APIs remain reference paths until their complete native
successor owns the operation; an individual inner call cannot cross a subprocess
boundary while leaving its outer Python transaction open. `reference_attach_maintenance`
retains the independent Python attachment oracle.

A prepared public binding requires an exact integer publication epoch within the
safe integer bound. Incidental Python dictionary equality between malformed boolean
or float epochs and integers does not expand this typed reader ABI. Independent
Python reference behavior is retained.
