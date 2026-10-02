# tos-compiler

The first Rust compiler family materializes source-navigation nodes, edges and
rights from one exact partitioned corpus projection into a private SQLite read
model. Its visible bibliographic profile excludes packet-member nodes and
their incident edges. IDs and predicates come from owner records; paths are
provenance only.

The legacy carrier adapter verifies the root digest, collection policy, every
selected index/data part, decompressed bytes, row placement/count, navigation
header counts and a second sealed-cut verification pass. Linux input opens
walk directory descriptors without following symlinks or blocking on FIFO
replacements. It is a compatibility input for a trusted local source cut.
STO.2 byte custody alone does not supply the coordinator-published sealed
membership/index cut; the legacy projection itself does not attest admission.

The candidate contains indexed rows, emitted carrier digests/lengths, the exact
source-owned navigation authority string and a per-visible-source adjacency
count/digest, including explicit zero-edge certificates. A separate local selector requires
an owner implementation of PublicationAuthority with a fence held through
the pointer decision, compares the expected
selected pointer and atomically switches it. The verified pinned candidate is
copied, hashed and durably installed before the short selection lock and owner
fence; the final decision rechecks the installed inode and held authority. The
private candidate remains for owner-stage cleanup; the publication directory
must be owner-controlled while the selector runs. This does not check current rights at query time; the
source owner does so separately. No permissive authority implementation is
included.

The crate currently implements a full source-navigation candidate build. It
does not implement affected compilation, all knowledge/catalog projections,
the old knowledge.sqlite3 schema, D1 SQL producer, remote page proofs, or
production pin/restore. See the CMP execution evidence for the coverage map.

## Build resource boundary

`Limits` rejects oversized rows, row counts and cumulative emitted input bytes.
The legacy adapter separately counts its verified reads against `max_work_bytes`.
SQLite uses file-backed temp storage, a configured page-cache target, a
`max_page_count` main-database cap, and a connection-local progress callback
that interrupts cumulative SQL virtual-machine work at `max_sql_vm_steps`.
The final database size is checked before a candidate receipt is returned;
failure removes the private candidate. These are separate limits: an SQL VM
step is not a byte of source work, and `cache_size` is not a process memory cap.

SQLite's rollback journal and external sorter spill files are **not** covered
by `max_page_count` or `max_work_bytes`. SQLite can select a process-global temp
location before this library opens its connection. Consequently, a target-scale
job must be launched in its own process with `SQLITE_TMPDIR` and `TMPDIR` set
to the same private path before SQLite is
initialized, with the SQLite temp path and candidate output on separately
quota-backed private filesystems or mounts. The launcher must verify both
quotas and free space, make SQLite's fallback temp directories inaccessible
outside the same quota, force a spill probe that confirms the effective temp
path, reserve the worst-case bytes through the host storage route, and refuse
the job if that isolation cannot be established. Set the
temp quota to the admitted spill allowance and the candidate filesystem quota
to cover the main database plus journal; monitor actual peak usage and retain
the exact quota/failure receipt. This library does not certify that launcher
contract, so a local successful compile alone does not establish bounded
spill behavior or billion-record admission.


The managed native release companion `tos_access_native_knowledge_selection_v1`
binds independently retained stage/source binding, full seal, navigation-original
receipt and `KnowledgeSelectedExpectation` to separately declared model,
descriptor and registry members. Its decoder verifies those exact bytes without
using the checked SQLite file to manufacture expected roots. ABI3 requires the
independent original-component root; older ABI2 expectations omit it. The release
holder supplies current-selection/disclosure authority separately.

`managed-local-linux-fsverity-v1` currently supports Linux x86_64. The producer
prepares a private artifact with SHA-256 fs-verity (4096-byte blocks, no salt or
built-in signature) and verifies its frozen raw bytes against the stage receipt.
The reader retains the FD and kernel measurement, verifies live finite
`RLIMIT_AS` and `RLIMIT_FSIZE` soft bounds against declared process limits, and
shares owned custody across warm forks. FSIZE is a per-file write bound, not an
aggregate SQLite temporary-space quota; existing query/work/private-temp budgets
remain required. Missing kernel support or wider/unlimited bounds refuses. This
software preparation neither selects a public release nor admits source/rights.

The maintained DTA Parts 1–4 lexical producer is exposed by
`zarathustra_lexical::build_from_cut` and the installed
`tos lexical-index build|validate|validate-legacy` command. It reads the exact
plan, manifests, resource inventories, rights states and schema closure from an
explicit authenticated corpus revision. The operator supplies a software source
capture, the four existing fixity-bound private TEI payloads, an exact verified
schema worker and finite JSON resource declarations. The native diagnostics-v2
adapter evaluates the complete projection under its existing 32 MiB raw-instance
profile; it never substitutes a small projection probe.

`build` requires a fresh private candidate root and writes the plan-declared
relative projection and SQLite paths below that root, plus
`native-lexical-provenance.jsonl` and `native-lexical-build.json`. The provenance
companion preserves the authored chain and appends one private native export
observation. `validate` checks that explicit candidate, its complete provenance
and the source/usage/morphology validation closure. `validate-legacy` checks an
explicit retained legacy projection against the captured authored provenance;
`--local-output-root` adds the private SQLite verification.

Whole validation also reads exact retained generated controls through an explicit
`--derived-input-root`; they remain weaker companions and never become authored
cut members. Recorded historical Python inputs resolve only by original script
path and exact SHA-256 through the existing retained-builder source route.

All three actions require `--source-store`, `--source-revision`,
`--software-root`, `--schema-worker`, `--schema-worker-sha256` and `--limits`.
`build` additionally requires `--payload-source-root`, `--candidate-root` and
`--event-time`; `validate` requires `--candidate-root`; `validate-legacy`
requires `--projection`. Paths must be absolute. The limits JSON has explicit
`max_seconds`, manifest/cut read ceilings, a `lexical` object matching
`LexicalLimits`, and a `schema` object matching `LexicalSchemaLimits`.
This native limits declaration remains provisional and bound to the selected
software source; it has no stable external schema version yet. Historical runtime
profiles retain their original source binding. A newly selected profile must
explicitly include the working database ceiling; acceptance of the producer and
its installed consumer remains separate from accepting this source interface.
`lexical.max_working_database_bytes` bounds the main SQLite file during row and
index construction; it must be page-aligned, at least `max_database_bytes`, and
within the same supported one-GiB ceiling. `max_database_bytes` remains the
post-`VACUUM` candidate and validator ceiling. The caller must separately admit
the working file, `VACUUM` temporary space, and all other live outputs and memory;
these two file ceilings do not grant that whole-operation budget.
`schema.max_preparation_bytes` admits the metadata-derived schema/parser/image
preparation upper bound before constructing the worker; `max_state_bytes`
separately admits diagnostics controller state. These are finite computational refusal boundaries; the host launcher separately admits
storage and process resources. No command writes the canonical projection or
source provenance, changes data selection, accepts German, supplies a lemma or
sign, clears rights, admits canon, or grants publication.

The Python builder/validator remains the distinct compatibility oracle until
real-input parity and the installed native producer/validator consumer have been
accepted. Archived source copies and historical lexical artifacts retain their
original identities. The native reading/word-search API is a consumer of reading
data and does not replace this lexical producer.
