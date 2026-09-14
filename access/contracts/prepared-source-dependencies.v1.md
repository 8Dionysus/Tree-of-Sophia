# Private prepared source-Claim dependency index

`tos_access.prepared_source_dependencies` stores explicit source-Claim
declarations and their reverse typed references in the **same prepared SQLite
file** as the [selected source roots](prepared-source-binding.v1.md). It does
not derive dependencies from graph incidence, inspect source files, normalize
Claims, discover missing Claims, or grant source/semantic admission. It is an
offline owner API, not a consumer packet or automatic CLI attachment.

## Declaration and selection

The assembler supplies `SourceClaimDependencies(claim_id=..., source_entry=...,
input_sha256=..., dependencies=...)`. `dependencies` is the complete supplied
output of `enumerate_bibliographic_claim_dependencies` in the source graph
owner: sorted distinct `(kind, ref)` rows with sorted `field_paths` and
`reasons`. Kinds are `identity`, `provenance_event`, `claim`, `path`, and
`unresolved`. Null refs are permitted only for `unresolved`; the original
explicit reason/path is retained, not converted to a resolved fact.

`input_sha256` names the exact **source Claim**, not its projected endpoint
cohort. It must equal `source_entry.claim_sha256`, and the entry must bind the
same Claim ID and a positive exact source-file line. A metadata-only Agent
change can therefore preserve these declarations even while Claim descriptors
and relation displays need recomputation. The assembler owns that separate
metadata/normalization closure.

The declaration retains detached canonical bytes and a separate computed
`source_entry_sha256`. Its canonical format is UTF-8 JSON with sorted keys,
compact separators and a final newline. The declaration digest covers every
entry field, source Claim digest, typed reference, reason and field path.
`value()` returns a detached JSON object; `digest` identifies the complete
stored declaration. Neither construction nor parsing verifies source bytes or
that the enumerator saw every actual dependency.

Every operation independently receives the selected prepared binding, exact
`source_inputs_sha256`, and `declaration_profile_sha256`. The latter is an
assembler-owned digest binding its enumerator and source-contract profile.
The kernel checks current prepared descriptor/metadata, exact private source
root bytes/digest/revision, its physical table/index definitions and a
conservative executable digest. Profile/schema drift requires explicit owner
bootstrap, never silent adoption. There is no separate mutable root file.

## Operations

- `bootstrap_source_dependency_index_transaction` takes an explicit full
  supplied `claims` stream. It creates fresh private tables, refuses duplicate
  or invalid declarations and leaves the prepared binding unchanged. Exhausting
  that stream is not proof of source coverage. There is no request-time or
  graph-derived bootstrap.
- `lookup_source_dependencies_transaction(kind=..., ref=...)` performs an
  exact typed indexed lookup and returns all dependent `claim_ids` plus their
  full declarations/digests, or refuses. It never truncates or falls back to a
  Claim/graph scan. Each returned declaration is checked against its complete
  per-Claim reverse rows and the complete selected address aggregate.
- `read_source_claim_dependencies_transaction(claim_id=...)` returns the exact
  declaration, or explicit absence, with the same per-Claim integrity check.
- `apply_source_dependency_delta_transaction` stages `SourceDependencyChange`
  values. Insert requires absence and no predecessor digest; update/delete
  require the exact old declaration digest. Delete cannot carry a replacement;
  update/insert must preserve the target Claim ID. Duplicate targets and stale
  selections refuse. All target inputs/preconditions are captured before
  lane writes, and selection is rechecked afterwards.
- `verify_source_dependency_binding_transaction` finalizes only after the
  caller has published both prepared rows and the paired source roots to the
  specified successor binding in the **same transaction**. It checks the next
  source digest/revision, epoch, unchanged normalization binding and exact
  staged declarations before clearing pending state.

A staged index refuses ordinary reads until finalized. Even an empty Claim
delta enters pending state and advances its selected binding; that path reads
no Claim declarations or graph rows. It does not discover which descriptors
need rebuilding. Committing pending state is a caller error, not publication
success or a resumable transition protocol.

The caller sequence is: begin the transaction; stage declaration changes;
execute the existing source-bound prepared publisher; verify the dependency
index against its returned binding and successor source input digest; recheck
source-owner guards; commit. Any exception requires **whole transaction
rollback**, including caller work and every prepared/source/index lane. These
functions never begin, commit, roll back, close or switch a consumer. SQLite
itself may abort a transaction on an interrupt or storage failure; callers must
not continue publication after either result.

`prepared_source_publication.apply_dependency_bound_prepared_delta_transaction`
implements this joined sequence for the existing source-bound publisher. It
accepts its ordinary before/after source and catalog inputs and prepared
`changes`, plus explicit `dependency_changes`, `declaration_profile_sha256`
and `progress_owner`. `PublicationLimits.max_mutations` covers all writes made
by the joined call, excluding earlier caller work. After declaration staging,
it reserves the exact conservative finalizer bound before assigning the
remaining allowance to semantic/catalog/row/lens/search/root publication.
It then finalizes the dependency binding and checks the complete write count.
The smaller dependency or publication whole-file cap applies throughout.
Other per-lane work/read/output limits remain independent, not a claimed shared
VM or elapsed-time budget.

The join performs no source observation, discovery, normalization or guard
callback. The assembler must still recheck its source transition and hold its
owner locks through the eventual commit. Its receipt separately reports
`source_dependencies_paired_in_caller_transaction=true` and the existing root
pairing flag, while retaining false source completeness/transition, target
closure, semantic acceptance and consumer-switch flags. Empty declaration
changes are supported for metadata-only source updates; the assembler still
supplies every affected normalized row and human form.

## Address integrity and cost

Unique reverse rows have primary key `(kind, ref_key, claim_id)`; `ref_key` is
the exact canonical JSON string or null. A second index supports exact complete
per-Claim reads. No graph ID is parsed to infer a source identity.

Each address head stores a count, XOR and modular sum of domain-separated
SHA-256 contributions over the unique kind/ref/Claim/declaration-digest tuple,
plus a seal of the head fields. This checksum algorithm is explicitly versioned
as `tos_source_dependency_count_xor_sum_sha256_v1`. A small Claim delta removes
and adds only its exact contributions. It never recomputes a popular Agent's
entire address bucket; missing/zero-head guards use at most two addressed rows.
An explicit complete lookup recomputes and checks the aggregate over its full
bounded result. These checks detect stored integrity drift under an admitted,
owner-produced baseline. They are **not cryptographic source trust**, authenticated
absence or protection from an equally authorized writer replacing checksums.

Unresolved declarations remain indexed and addressable. Their presence is not
permission to omit a dependent Claim from recomputation; the assembler must
explicitly handle them and independently establish complete source selection.

## Budgets and connection ownership

`SourceDependencyLimits` uses the shared auxiliary-index operation budget:
rows, queries, reads, input values/bytes, output items/bytes, SQL writes and
whole-file bytes. Additional defaults are 4096 supplied/returned Claims,
4096 dependencies per Claim, 16 MiB retained before-plus-after delta bytes,
20 million conservative VM steps, and 8 MiB state.
The declaration format caps individual bytes at 16 MiB, dependencies at 65536
and identifiers/reasons/field pointers at 4096 UTF-8 bytes. Defaults remain
1 MiB stored declaration rows and 32 MiB operation input accounting. All limits
are refusal caps, not full-corpus admission or RAM/disk forecasts.

Stored wide columns are length-masked before Python transfer. Queries use exact
primary/index prefixes and explicit caps; own-table triggers are outside the
write mask. Whole-file allowance is lowered to the minimum of the existing
SQLite ceiling, prepared owner ceiling and this operation's byte limit. It
does not include rollback journal or source/runtime memory.

Every entry point requires `progress_owner=ProgressHandlerOwner()`, explicitly
asserting no prior connection progress handler, or
`ProgressHandlerOwner(previous_callback, previous_interval)` when one is known.
SQLite exposes no handler getter. The known prior callback is chained during
operation and restored on success/failure. The temporary handler enforces the
VM budget, including conservative per-statement tails. The caller must not
replace callbacks or reenter this lane while it runs. Row factories and
authorizers are not replaced.

Write accounting is per entry point. The stage receipt includes
`finalize_sql_mutations_upper_bound=changed_claims+1`; finalization reports its
actual writes. A joined publisher must reserve and count **all** lane writes
under its overall transaction allowance, not treat these independent caps as
one already-enforced combined limit. All receipts retain false source
verification/completeness/transition, target closure, semantic acceptance and
consumer-switch flags. `complete_stored_address_verified=true` is only the
bounded stored index result, not source-universe completeness.
