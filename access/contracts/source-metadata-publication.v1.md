# Initial metadata to prepared publication

`scripts/source_metadata_catalog.py` and `scripts/source_metadata_publication.py`
extend the existing source-owner publication path with one **initial standalone
metadata subject**. The source command commits first; the publisher observes
its exact current receipt, request, record, human forms and provenance. The
type comes from the registered source profile or native standalone creation
contract, not an Agent-only branch or a name supplied by a browser.

This operation supports separately authorized public-metadata `source.create`
for declared profiles and native standalone identities. It does not authorize
Sign issuance, Artifact intake, native text, compound bibliographic attachments,
record correction, identity replacement or semantic admission. Those routes
have stronger source contracts. A new profile still needs its actual schema,
reader and registry admission; this is not runtime schema invention.

## Addressed operation

`metadata_catalog_addition` takes the unchanged protected creation configuration,
an explicitly admitted catalog predecessor and exact request/receipt digests.
It verifies historical creation scope at the receipt's recorded instant and
unchanged initial output bytes under the source writer lock. Natural expiry of
that write delegation does not invalidate already committed metadata. The
receipt must predate expiry, must not be future-dated, and must match the exact
configuration, principal, authority and source path. Changing or revoking the
configuration still refuses this selected route. New source commands continue
to require a current grant; the read-only evidence inspector is not a command
or renewed authority. The caller separately owns derived publication and
consumer selection, just as for full snapshot construction.
It stages the new metadata row and its captured provenance
slot in immutable catalog parts. It does not scan source directories, rebuild
the whole catalog or select that candidate for a consumer. Existing source
metadata publication tokens are retained; no fictitious revision transaction
is created for the addition.

`metadata_addition_publication` joins that candidate with the existing prepared
root-vector, dependency and context indexes. The caller provides the exact
predecessor binding, catalog inputs, progress owner and separate work budgets.
It uses the existing source metadata reader, source-navigation and bibliographic
identity renderers, then the complete pure normalization kernel on only the
bounded new carrier cohort. It selects those exact carriers and their complete
incidence, excluding unrelated repository scaffolding. This preserves the
shared-identity `projects` edge without copying its rule or normalizing the
existing corpus. The current RecordVersion and its edge remain distinct from
the subject. Human forms, original fields and source provenance are preserved
by those shared renderers. The ambient incremental cache is not used.

New carrier IDs and version edges must be absent in both raw and prepared
predecessors. The initial subject must have exactly its current record version,
without previous revisions or dependent external navigation. Source-cited
canon grounding requires its external closure and is not silently omitted.
An existing
reverse identity dependency, unresolved reference to the selected identity or
unresolved dependency with no address requires a broader dependency closure;
the publisher does not silently repair it. A specifically addressed unrelated
unresolved reference is not selected by this addition. Source dossier membership
is derived by the existing shared rule, not a profile-specific UI case.

Only changed immutable parts and affected prepared rows are written. Normalized
rows, search, catalog, semantic and lens indexes, paired source roots and
private context binding advance in one caller-owned SQLite transaction. A new
metadata publication implementation receives its own dependency digest; it
does not rewrite old Agent or Claim execution profiles. Changing an already
bound implementation requires a separately reviewed compatible transition or
actual migration, never a copied current hash.

## Commit, failure and delivery

Inside the context manager, call `BEGIN IMMEDIATE`, `apply_transaction`, and
`commit_transaction`, or explicitly abandon the candidate with
`rollback_transaction`. An explicit rollback is a valid terminal outcome, not
an obligation to commit a rehearsal; the closed candidate cannot be reused.
The source lock and current source guards remain held
through commit. Source/creation-evidence drift, a caller DML/DDL change after apply,
stale paired inputs, unsupported scope or exhausted work budgets refuse the
transition. Roll back the complete SQLite transaction on failure. The source
command remains committed and inspectable; unselected immutable parts may
remain, without automatic deletion. Source and SQLite commits are not one
cross-filesystem transaction.

Readers selected at the old binding see the predecessor until commit and
report stale binding afterward. No running reader, HTTP service, D1 database
or UI is automatically switched. The returned receipt separates source commit,
prepared commit, bounded initial metadata closure, consumer activation and
semantic acceptance. The receipt retains the paired source header and semantic
report digest, not additional copies of the whole catalog and semantic report;
the selected reader supplies the catalog. `global_source_currentness_verified=false` explicitly
retains the bounds of the supplied predecessor; this addition does not audit
unrelated source files or turn metadata visibility into text access.

The default cohort limits reuse the shared publication budget: 512 nodes,
1024 relations and 16 MiB material. Each source, projection and SQLite reader
retains its own limit. These are work bounds, not an unmeasured latency promise.
Focused tests compare all prepared rows and query results with full small-fixture
normalization, forbid full materialization on the addressed path, and exercise
concurrent predecessor reads, source revocation and late rollback/retry.
