# Retained processing dependency navigation

The native Rust source compiler can opt into `tos_compiler::processing_cache`
for deterministic normalization and finalization work. `ProcessingCache::open`
creates or opens its explicitly selected database and owns the
`tos_rust_processing_*` namespace; it ignores the retired Python scheduler
tables, which are never migrated or reused. Each `ProcessingRun` records its
inputs, completed tasks, exact dependency edges, and durable pure outputs.
Publication of that run's dependency snapshot uses compare-and-swap against
the active baseline. A failed or superseded run does not replace the active
snapshot. The cache is optional and does not change source, review, rights,
canon, or publication authorities.

Rust `processing_dependency_closure` and `processing_input_changes` provide
bounded reads. The caller opens the exact cache file read-only and owns a live
SQLite transaction; the read functions do not initialize or migrate schema,
or begin, commit, or roll back the caller's transaction. Closure reads require
one explicitly named completed run. Input-delta reads accept one explicitly
named retained run in running, complete, failed, skipped, superseded, or
interrupted state; they report added and changed inputs from that run, but
report removed inputs only when the run completed. Both fail closed when
required retained evidence is absent.

```rust
let db = rusqlite::Connection::open_with_flags(
    cache_path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
)?;
let tx = db.unchecked_transaction()?;
let impact = tos_compiler::processing_cache::processing_dependency_closure(
    &tx,
    exact_completed_run_id,
    &["source-node:philosophy:example".to_owned()],
    Default::default(),
)?;
tx.rollback()?;
```

The caller owns the read transaction and its lifetime. The result names the
run, changed identities, reachable task/input identities with output digests,
and reverse dependency edges. Multiple paths do not duplicate a reachable
node. Unrelated inputs are not read or reported as removed. A retained
historical run remains historical even if a newer publication exists.
Duplicate changed seeds are rejected, matching the former bounded reader
contract.

Default refusal limits are 4096 visited identities, 16384 dependency edges,
4096 UTF-8 bytes per identity, and 1 MiB for the compact UTF-8 result. Each reverse
lookup is an indexed seek with a remaining-budget limit; overlarge identifiers
and digests are excluded by SQL projections before delivery to Rust. Work is
bounded by the selected closure, not a count/diff of all tasks. Malformed or
cyclic retained evidence, missing seed identities, retired/incomplete runs and
exhausted budgets return an error. **No partial result is a complete closure.**

The source compiler opts in only when its caller supplies a
`ProcessingRunHandle` through `NativeFamilyInputs.processing_run`; the default
build and installed CLI do not create a cache. The native source producers for
Navigation, Philosophy, Canon, Candidate Intake, Claims, Repository and the
shared semantic-join base use the same DAG. Canon and Philosophy family
normalizers keep their source-record decoration inside the cached result;
Candidate Intake contributes relations but no node producer. These producers
record the following concrete links within the same run:

- `source-node` -> `node` -> `final-node` -> `readable-node`;
- `source-relation` -> `relation` -> `readable-relation`;
- `node` -> `endpoint-title` -> incident `relation`;
- `relation` -> `relation-views` -> endpoint `inherited-views` -> `final-node`;
- reified claim `node` -> `claim-context-source` -> ordered
  `claim-context-group` -> referenced `relation` and literal-context reducers;
- source trace and versioned `claim-association`, base claim/endpoint nodes,
  and `source-registry:relations` -> registry-backed `claim-policy` ->
  `claim-update` -> `final-node` -> `readable-node`;
- source trace/association, raw literal `node_kind`, current object and shared
  claim-context group -> ordered `literal-claim-contribution` ->
  `final-node` -> `readable-node`.

The readable stage exists only when the existing context hook needs it; without
that stage, `final-node` or `relation` is the terminal carrier. A hook links only
the exact completed producer task whose output digest matches its input value.
An absent producer remains an explicit standalone `Input`, not an inferred
source link. An incomplete or mismatched producer refuses the run. Fixed stage
identities preserve repeated evaluation without creating a self-dependency.
No extra row-retaining map is created. Terminal binding checks replace the old
disconnected Input's full-value hash on the normal one-pass build route; it
does not remove the cost of hashing a row. Repeating a hook call verifies its
supplied mutable value again even when task execution is reused.

The reducers read resolved dependency values, not actions closing over assembly
maps. Cached grouping retains task references and small context/view projections.
The uncached builder uses the same authored reducer computations with streaming
view/context accumulation and preserves its owned-node finalization path. New
reducer producer checks and projection outputs add work per dependency binding
(including repeated endpoint hashes when several traces share an endpoint);
the prior terminal-hook hash comparison is not a zero-cost claim for this layer.
Assembly helpers participate in the processor fingerprint.

Views include incident relations with empty contributions and count a self-loop
once. Context groups preserve source encounter order, deduplication and conflicting
records, without selecting a winner. Literal delivery uses the preserved raw
`node_kind`, not an inferred semantic type. Trace order and the existing last-wins
rule for multiple traces targeting one claim node remain unchanged. The versioned
`tos_claim_association_v1` Input records supplied claim/subject/object membership;
it does not authorize or discover it. Registry policy requires the exact completed
`source-registry:relations` Input identity and matching output digest. Missing,
incomplete or mismatched source producers refuse the run. Standalone finalization
hooks without reducer producers retain explicitly named standalone Inputs.

This operation still does not prove that the normalizer recorded all
dependencies needed by the source owner. The owner still must supply complete
source-to-final-carrier mappings, including assessment, rights, placeholder and
topology dependencies.
New or retargeted relation endpoints, new claim-group membership, and newly added
source identities absent from the retained run need that owner's explicit
before/after mapping. An absent join remains explicit input data, not a guessed
empty source impact set. A baseline DAG cannot discover new membership.

The result does not evaluate tasks, authorize partial execution, publish a
prepared snapshot, certify global semantic invariants, or confer acceptance.
`ProcessingRun::finish()` completes the graph the producer recorded; it is not a
partial-update API and does not prove source dependency completeness. Source
change detection, dependency completion, normalization, validation and
publication remain separate obligations.

The maintained `build_native_addressed_knowledge_snapshot_from_capture` route
validates a complete source transition and rebuilds the snapshot. Source-owner
paired before/after membership and source-to-COW-to-prepared publication remain
unimplemented.

Focused Rust check: `cd rust && cargo test -p tos-compiler processing_cache::tests`.
The cache tests cover bounded closure, cache reuse, failure recovery and
publication conflict behavior.
