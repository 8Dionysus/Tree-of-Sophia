# Managed local native access v1

The native Rust owner implements the managed release lifecycle and holder
checks. It selects an already admitted projection. It does not
admit a source, grant rights to source records or payloads, or issue a public
service credential. The first execution candidate is an isolated software
fixture; this contract does not activate an installed/public release.

The native entrypoint accepts a leading `--release-root ABSOLUTE_DIRECTORY`
(or `--release-root=ABSOLUTE_DIRECTORY`), overriding `TOS_RELEASE_ROOT` when
both are present. With neither selection it retains `NoOwner`. A selected
root that cannot be admitted produces an explicit unavailable refusal, exit
3. Invalid command/serve options are refused with exit 2 before cold opening.
Missing data never discovers another checkout.

The root reuses `tos_access_release_pointer_v1`,
`tos_access_release_pair_v1`, immutable pair and binding records, the existing
three data/corpus/software revocation directories, and `.release.lock`.
Metadata uses the existing sorted compact UTF-8 JSON with one final LF and
strict duplicate-member rejection. Pair identity is SHA-256 of those complete
canonical bytes. Existing normalized absolute path and no-symlink rules apply.
The bootstrap parser uses the shared foundation metadata profile (1 MiB,
64 levels, 300,000 visits, 4,300 integer digits); that cap supplies neither
model nor query nor process admission.

The native data manifest schema is `tos_access_native_data_snapshot_v1`.
Its exact field set is the existing snapshot fields `schema_version`,
`corpus_revision`, `input_bindings`, `compiler`, `members`, `data_revision`,
plus `native_selection`. The selection names one declared relative member.
Members remain sorted and unique, under `data/`, with exact `path`,
`size_bytes`, `sha256`. The manifest is paired by its complete SHA-256;
`data_revision` is SHA-256 of canonical manifest bytes excluding that field.
The compiler schema equals the selected model ABI; compiler version equals
the installed native producer version. In this native profile, compiler fingerprint
inputs name the code-owned native program members and their actual digests; they
do not relabel the produced model as a compiler input. The old Python manifest and eight-field
prepared-binding formats keep their existing meanings and are not ABI3.

The producer companion uses `tos_access_native_knowledge_selection_v1` for
ABI2/3 and `tos_access_native_knowledge_selection_v2` for ABI4, with the same
`managed-local-linux-fsverity-v1` profile. The compiler decoder owns both versioned
shapes. It persists actual stage/seal, optional navigation-original and
philosophy-original receipts, the independently
produced `KnowledgeSelectedExpectation`, exact model/descriptor/entity-registry/
relation-registry member paths, fs-verity measurement, `ColdOpenLimits`, and
process limits. It is decoded by the compiler against the exact selected
metadata bytes and actual supported native adapters. Consumers never recover
expected roots from checked SQLite or infer supported adapters from a selected
descriptor. The model manifest member size/SHA must equal the independent
expectation; descriptor/registries must match their declared member digests.
Whole snapshot compilation and publication remain the release builder's job.

The existing public-ledger query/http subset in `runtime-data.v1.json` can be
included as sorted declared `data/<source_path>` members. The original
`source_path` SHA and the exact runtime-data declaration SHA are retained in
manifest `input_bindings`. All owner-declared public members, their order,
SHA/size and original source bindings must match under the same release hold;
missing, extra or changed ledger members refuse the read. The producer derives
identities and paths from the authored declaration, not a copied name/count list.
This layout adds no corpus payload or ambient tree access. The maintained
`/api/source-gaps` GET/HEAD route passes the verified bytes to the shared QRY
string-only public-safe projection and retains the same hold through flush.
No source-gap CLI or MCP tool is introduced. The managed entrypoint retains its
existing cold-admission and kernel custody prerequisites for the whole selected
profile; this additional public projection does not bypass them.

Kernel custody requires fs-verity on the selected artifact, a retained FD and
matching fs-verity measurement. Chmod/stat or a one-time hash is insufficient.
Cold resource admission checks finite live soft `RLIMIT_AS` and `RLIMIT_FSIZE`
bounds no greater than the selected producer profile. AS bounds address space;
FSIZE bounds each written file, not aggregate SQLite temporary storage. The
compiler's existing bounded private working-set checks remain necessary. Missing
or unsupported custody/enforcement refuses cold admission; the consumer never
enables fs-verity or changes host limits itself.

Every query obtains a fresh shared `.release.lock` on the selected holder.
The retained lock serializes publication changes/withdrawal through final
transport flush. Root, data-root and lock inode identities, exact current
pointer, immutable pair/binding/manifest and all three revocation states are
checked under that hold, before query use and final disclosure. A changed
selection refuses; this profile does not silently follow a new pair. A busy
or absent holder is unavailable, not authority. The current-policy identity
names this real local holder and release pair; it is not a raw-source rights
receipt. The model is cold-admitted once, then bounded readers retain the same
kernel custody. No global cache or issuer is introduced.

Native CLI, loopback HTTP and MCP reuse the existing operation descriptor and
QRY engine. Catalog/inspect, temporal, lens/focus/stored lens, exploration,
legacy search, capabilities and contracts use this held projection authority.
Dossier GET/HEAD `/api/source/dossiers/{object_id}` and MCP
`tos_dossier_inspect` require the exact persisted original component receipt;
there is no dossier CLI command. Its original component only computes the
maintained admitted dossier fields. ABI4 philosophy GET/HEAD and the existing
`tos_philosophy_graph_*` MCP tools require the independently persisted philosophy
original component receipt. Node, edge, neighborhood, path, view, views, layers,
clusters, review packet, snapshot and unresolved reads use the common QRY kernel
and its bounded ordered original rows. The same release hold covers every
consulted header/node/edge grant through final flush. Missing phi originals
leave those tools unavailable; the consumer does not derive them from normalized
knowledge rows. No philosophy one-shot CLI is added. It does not expose a new raw header/rights
or source-byte operation. Exact source record/text remains the separate opt-in native source-owner
contract in `source-read.v1.schema.json`. Indexed transport
continuations and compressed publication require their respective real owner
seams; readiness is not inferred from an engine capability packet.

The binary retains the 1 MiB query-packet and 64 KiB request/line caps. Its
explicit MCP frame allowance is the checked bound from those declared caps,
including text escaping, structured copy, request ID, envelope and final LF.
A frame still exceeding its declared allowance receives the existing bounded
JSON-RPC refusal before packet disclosure. Library defaults retain equal packet/
frame caps. Cancellation, deadline and current-release refusal remain independent.
Disposable exploration retains the maintained 900-second TTL, 128 checkpoints,
32 MiB checkpoint cap, 512 work units per page, 10,000 session nodes and 20,000
session relations; selected cold work caps additionally bound these allocations.

Source implementation, isolated filesystem/process admission, actual installed
entrypoint execution, public activation, Worker response-body lease and TS
retirement are separate claims. This profile closes none of those later gates
merely by declaring the format.


Corpus projection uses the CMP ABI5 original component and versioned native selection companion v3. The captured-public origin records software Git commit/tree/capture manifest identity separately from the authored source cut; their joint selected release does not assert native builder ancestry. The original index source path must be the corpus-index subject declared by runtime-data.v1.json, with its existing query-core/http-reader/native-mcp roles. The data snapshot declares `data/<source_path>` and every original captured member in the receipt, with exact size/SHA and the original source SHA in input_bindings; input_bindings also binds the exact runtime-data declaration. Missing or differing members refuse admission and availability. Corpus payloads and ambient source reads are not admitted.

Under the same ReleaseLease, cold admission checks the complete raw original member closure once. Each later addressed corpus query keeps DataGuard identity `(device,inode,size,mtime,ctime)` for these members and checks it under the held release lock through final flush. The retained immutable selected model owns original row bytes; ordinary addressed reads do not reread the entire corpus. Status display paths identify the actual declared index member and its selected data root. SQL component presence alone cannot assert index_exists. The eight maintained MCP tools and six existing HTTP GET/HEAD routes call the same QRY corpus kernel; resources and packet remain MCP-only, and no corpus one-shot CLI is introduced. This profile still requires the managed native cold custody/resource prerequisites before its installed entrypoint serves data.


## Native pair publication

On Linux, `native-release-promote --request ABS --request-sha256 HEX
--work-deadline-ns ORIGINAL_NS` publishes the existing NativeData pair format.
The request is independently pinned canonical JSON. It selects the exact
candidate pair, bindings, candidate manifest and producer selection digests,
installed software prefix, expected current pair (or null for first publication)
and finite resource profile. It neither discovers a root nor accepts a
caller-supplied verification boolean or Python callback.

Before taking the exclusive release lock, the native owner verifies the
complete declared candidate closure, shared manifest and Original provenance
law, exact producer selection, immutable selected model cold admission, accepted
software archive and both installed Access and Owner command roles. Archive,
installed manifest and build proofs must describe the same software. The
candidate remains independent of `current.json`; first publication does not
open a fabricated current release. Verification retains file identities and
rechecks them around publication. Source, rights, canon and semantic acceptance
are not issued by these mechanical checks.

The filesystem owner takes the existing process-owned exclusive lock and
revalidates layout, current pointer, immutable pair and bindings, all three
revocation kinds and expected-current CAS. Identical immutable records may be
reused; differing records refuse without overwrite. Same-current publication
still verifies bindings and revocations. Immutable records use a fsynced
exclusive temporary, atomic Linux `renameat2(RENAME_NOREPLACE)` and parent
directory fsync. This preserves immutable no-clobber publication without a
hard-link/unlink crash interval that would leave link count two. A host or
filesystem lacking the atomic no-replace operation refuses; it does not fall
back to overwrite. The
current pointer uses a fsynced temporary, atomic replacement, root fsync and
canonical readback. A failure after replacement preserves the committed state
and reports durability separately; it does not roll back another publication.

The promotion scope accepts at most 3600 seconds remaining. A longer admitted
Site attempt narrows this scope to `min(site_deadline, scope_entry + 3600s)`
and pins that same monotonic value in the request and CLI. This narrows the
existing attempt; it does not renew its overall clock.

All stages consume the original monotonic deadline and declared metadata,
state, IO, descriptor, archive, candidate and model bounds. Explicit
`max_installed_io_bytes` and `max_installed_state_bytes` reserve slices of the
same aggregate budgets before installed verification; both roles and their
post-publication rechecks consume those same slices. Filesystem reads, writes
and bounded metadata decoding charge the remaining shared budget. Installed software
checks and publication checkpoints observe that deadline directly. Existing
archive and selected-model cold owners additionally enforce finite member,
byte, row and SQLite-work limits, but their current public scan APIs have no
inner cancellation hook. Their external admitted process/unit remains the hard
wall-time owner; a completed scan must pass the same original deadline before
publication. These structural counters do not certify allocator RSS or host
memory. Publication, durable readback and subsequent installed cold reopen are
separate evidence claims.
