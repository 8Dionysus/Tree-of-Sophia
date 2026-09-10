# Partitioned projections and explicit query compilation

## Index Metadata

- Decision ID: TOS-D-0059
- Original date: 2026-09-10
- Surface classes: derived-export, contract, access, scripts/validation, docs
- ToS layers: derived-export, contract, validation, docs
- Tree classes: corpus, graph projection, generated companion
- Guard families: source-returning projection, deterministic generation, repository portability, runtime boundary
- Posture: accepted

## Context

Rapid source planting grew the corpus index to 174,073,880 bytes and the
bibliographic graph to 190,499,116 bytes. GitHub rejected the preceding delivery
because its monolithic generated files already exceeded the per-file limit.
Whole-file readers, graph assembly and query indexes also retained the growing
corpus in memory. Compression of the same monolith would address only storage
and transfer.

The Operator chose to implement a long-term scaling boundary immediately.
The work must preserve source-first records and independent evidence, review,
rights and canon authority. A generated carrier cannot simplify these away.

## Decision

Keep ToS-owned logical projections but store their growing collections in
bounded, independently compressed, content-addressed partitions, reached through
small hierarchical manifests. Partition placement follows stable record keys;
small changes do not renumber the corpus or rewrite unrelated parts. Manifest
closure, byte limits, digests and key placement are validated explicitly.

Compile a separate, disposable SQLite query artifact through an explicit
offline route. Preserve the existing pure normalization and semantic checks,
using disk-backed intermediate collections and indexed passes. Ordinary access
queries consume a completed snapshot selectively and never trigger a hidden
full rebuild. Keep the Worker's existing indexed D1 serving architecture and
feed its importer from compiled row streams.

Bind query revisions to the exact input manifests, registries and compiler
identity. Integrity and source parity remain different checks. Source-owned
meaning, provenance, claim reification, human-form restrictions and admission
posture remain unchanged.

## Alternatives

- A single gzip carrier is smaller and simpler to introduce, but leaves
  whole-document change amplification and eager processing in place.
- Git LFS moves large bytes out of ordinary Git transport but adds a separate
  client/storage dependency and does not improve query or build granularity.
- Fixed consecutive batches are simple, but inserting an early record can
  shift many later batches. Stable-key radix partitions localize that pressure.
- Keeping every compiled runtime snapshot in Git would continue accumulating
  bulky derived history. Query stores are compiled artifacts instead.
- Copying a separate semantic assembler into each backend would risk divergent
  judgments. Shared authored normalizers and preserved validation rules remain
  the construction authority.

## Consequences and boundaries

The two existing `.min.json` entry paths now carry a versioned storage manifest;
readers must opt into that contract. A raw JSON parser no longer receives the
whole logical projection. Full export remains explicit and potentially costly.
Bundles must validate the manifest's exact closure, including corruption and
missing-part failures, rather than discover files by glob.

The current philosophy graph retains its deduplicated representation from
TOS-D-0037. This decision supplies the separate partitioned-transport rationale
that record deferred; it does not reverse its single-materialization rule.
Independent gzip encoding follows the distinction in TOS-D-0058 and creates no
new evidence layer.

Full source verification still examines all relevant inputs. Disk staging
bounds retained material; it does not promise constant-time full verification.
Exact substring and arbitrary property semantics may require disk scans where
no selective index applies. A record that exceeds the declared individual
bound requires owner restructuring, not silent truncation.

Historical oversized blobs must be absent from the history prepared for GitHub
delivery. Preparing that history must preserve source evidence and unrelated
work. Local validation does not establish CI, merge, publication, runtime
activation, semantic acceptance or canon.

## Owner surfaces and validation

- `ToS/derived-exports/PARTITIONED_PROJECTIONS.md`
- `ToS/contracts/partitioned-projection.schema.json`
- `scripts/partitioned_projection_common.py`
- `scripts/tos_corpus_index_common.py`
- `scripts/source_witness_bibliographic_graph_common.py`
- `access/src/tos_access/projection_store.py`
- `access/src/tos_access/knowledge_compile.py`
- `access/src/tos_access/query_store.py`
- `access/contracts/runtime-data.v1.json`
- `access/packaging/`
- `access/deploy/cloudflare-worker/scripts/build_runtime.py`

Validate logical source parity, exact manifest closure, corruption and snapshot
failures, localized update behavior, query ABI parity, indexed/bounded reads,
compiler atomicity, and a clean standalone archive. Run affected source,
generated, access, Worker and documentation lanes before delivery review.
