# Partitioned projections

The corpus index and bibliographic claim graph use
`tos_partitioned_projection_v1` storage. Their existing `.min.json` entry
paths now contain small manifests. They are not the logical JSON documents.
The logical schemas still own record meaning and all source/claim boundaries.

## Storage contract

Each manifest carries the logical header, explicit collection descriptors and
format limits. Growing arrays and the bibliographic input-digest mapping live
in separate collections. Collection keys are stable record identities; array
positions never determine partition placement. Scoped identities use an ordered
list of key fields, encoded as a compact JSON string array; corpus relation
edges use `[pack_id, edge_id]` because edge IDs are package-local. Enumeration order inside the
transport is not query order. Explicit full export uses each collection's
declared ordering fields.

A SHA-256 radix tree maps each key to one independently compressed JSONL part.
Small directory objects name their children by the next key-hash digit. Every
descriptor binds the relative path, stored and decoded SHA-256, stored and
decoded byte counts, prefix and row count. The path itself contains the stored
content digest. Updating one record replaces its leaf and ancestor directories;
unrelated content objects remain unchanged. Splitting an over-target leaf is
deterministic and local to that prefix.

The root is limited to 256 KiB, a directory to 128 KiB, and a decoded data part
to 8 MiB. The normal target is 1 MiB per leaf. An individual logical record must
fit the hard part limit; exceeding it is an explicit owner error, not permission
to drop fields or publish a partial record. Growing collection directories are
hierarchical, so the root does not accumulate an unbounded list of leaf paths.

The writer stages rows on disk, writes immutable parts, then atomically
replaces the root. A failed build never publishes a partial root. Owner
builders may retire only correctly named, digest-matching, unreferenced objects
in their exact part namespace. A reader bound to a retired snapshot fails
explicitly if its parts are unavailable; it must never mix snapshots.

`access/src/tos_access/projection_store.py` implements this encoding for both
source builders and portable consumers. `ProjectionReader` exposes metadata,
collection iteration, exact-key lookup and validated closure traversal. Its
cache is bounded by decoded bytes. Missing parts, wrong digests, wrong counts,
duplicate or misplaced keys, path escapes and symlinks are rejected. Checksums
establish integrity against the selected manifest, not independent provenance
or admission of a maliciously replaced manifest.

## Build and query boundary

Source builders retain their authored construction and validation rules, using
disk-backed intermediate maps and sequences. Exact source parity checks remain
complete checks; partition integrity alone cannot establish source parity.
Source parity compares the complete decoded canonical JSON and the exact
collection identity/ordering policy. Stored-byte integrity is checked separately:
compatible gzip implementations may encode the same records differently. Builds
with the same compressor remain byte-deterministic; source parity does not require
identical DEFLATE output across compressor implementations. Source metadata,
human-form context, claim reification, evidence, review and rights fields must
survive unchanged. Physical source payloads remain excluded.

Ordinary access queries use an explicitly compiled, completed SQLite read
model. Compilation is an offline operation, not an automatic side effect of
opening a node or making an HTTP/MCP request. The compiled store binds the
manifest snapshots, semantic registries and compiler identity, and is published
atomically only after validation. A missing or stale store is a build-required
state. Query connections are read-only and create no journal or cache files in
the source tree.

Exact identity and adjacency queries use database indexes. Trigram indexes
accelerate candidate selection for substring search; the existing literal
substring test and ranking still decide the result. Short or NUL-containing
queries and arbitrary unindexed property filters may scan rows on disk. They
do not reconstruct the graph in memory. The compiler requires the advertised
SQLite search capability unless an explicit scan fallback is selected.

Large accepted diagnostic-gap sets remain complete in the compiled diagnostic
table. Compact metadata names their count and detail collection. An omitted
inline list is explicitly marked incomplete; it is not an empty-gap verdict.

The existing deduplicated philosophy projection is a separately bounded legacy
input to this compiler. It is not repartitioned by this change. Explicit full
export APIs may materialize a logical document; ordinary query routes may not
call them implicitly. The repository bibliographic query command retains its
stronger exact source-rebuild check, performed with disk staging. Its graph
digest describes canonical logical JSON; the manifest digest is reported
separately for partitioned storage.

## Packaging and delivery

Portable bundles include only the validated closure of allowlisted manifest
subjects and their explicitly compiled query artifact. A filesystem glob cannot
grant inclusion to an unrelated part or local source payload. Fingerprinting,
copying and archive validation use the same closure.

The current static ABI bundle manifest
(`mechanics/release-support/parts/artifact-bundles/manifests/generated_readmodel.bundle.json`)
lists exact subject roots and has no generated per-snapshot declaration for a
partition manifest's digest-named parts. The parts themselves are generated
companions; their presence does not add them to the static ABI subject list.
Since the OS Abyss resolver does not
recursively follow that ToS manifest, the ToS artifact-bundle validator fails
closed when any closure member is absent from `artifact_subjects.path` and
returns admission to the artifact-bundles owner. The new partitioned corpus
remains deferred from this ABI bundle until exact closure support is reviewed;
source or access build success does not change that boundary.

The Worker importer consumes compiled rows and retains its existing selective
D1 query backend, lossless payload chunking, revision checks and publication
boundary. A successful local bundle build does not deploy or activate it.

Large generated SQLite stores and full assembled JSON are build/runtime
artifacts, not tracked corpus sources. Before Git delivery, unsent history must
also be free of oversized monolithic blobs; replacing only the newest file
does not remove an oversized ancestor. Historical source evidence remains
preserved independently of the prepared delivery branch.
