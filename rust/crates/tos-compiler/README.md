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
selected pointer, copies from a verified pinned candidate descriptor, verifies
installed bytes against the digest and atomically switches the pointer. The
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
