# Managed local native access v1

This Linux consumer profile ports the existing `ReleaseStore`/`DataGuard`
release-holder role. It selects an already admitted projection. It does not
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
or source-byte operation. Exact source record/text remains the separate opt-in
`SelectedSourceReadService`/`SourceOwnerBinding` contract. Indexed transport
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
