# Retained processing dependency navigation

The offline `ProcessingScheduler` records a DAG of evaluated `Input` and `Task`
identities. `tos_access.processing_closure.processing_dependency_closure`
provides read-only reverse navigation of **one explicitly named completed run**.
The scheduler's explicit cache initialization installs the reverse index; an
older cache without that index is refused by the reader, never migrated during
a query.

```python
import sqlite3
from pathlib import Path
from tos_access.processing_closure import processing_dependency_closure

cache = Path("/explicit/offline/normalization.sqlite")
db = sqlite3.connect(cache.resolve().as_uri() + "?mode=ro", uri=True)
try:
    db.execute("BEGIN")
    impact = processing_dependency_closure(db, exact_completed_run_id,
                                          ["source-node:philosophy:example"])
finally:
    db.close()
```

The caller owns the read transaction and its lifetime. The function neither
commits nor rolls it back. The packet names the run, changed identities,
reachable task/input identities with output digests, and reverse dependency
edges. Multiple paths or seeds do not duplicate a reachable node. Unrelated
inputs are not read or reported as removed. A retained historical run remains
historical even if a newer publication exists.

Default refusal limits are 4096 visited identities, 16384 dependency edges,
4096 UTF-8 bytes per identity, and 1 MiB for the compact UTF-8 result. Each reverse
lookup is an indexed seek with a remaining-budget limit; overlarge identifiers
and digests are excluded by SQL projections before delivery to Python. Work is
bounded by the selected closure, not a count/diff of all tasks. Malformed or
cyclic retained evidence, missing seed identities, retired/incomplete runs and
exhausted budgets raise an exception. **No partial result is a complete closure.**

The normalizer retains these concrete producer links within the same run:

- `source-node` -> `node` -> `final-node` -> `readable-node`;
- `source-relation` -> `relation` -> `readable-relation`;
- `node` -> `endpoint-title` -> incident `relation`;
- `relation` -> `relation-views` -> endpoint `inherited-views` -> `final-node`;
- reified claim `node` -> `claim-context-source` -> ordered
  `claim-context-group` -> referenced `relation` and literal-context reducers;
- `source-claim-trace`, base claim/endpoint nodes, `claim-association` and
  registry-backed `claim-policy` -> `claim-update` -> `claim-finalization` ->
  `final-node`;
- source trace/association, raw literal-kind envelope and shared claim-context
  group -> `literal-claim-contribution` -> `literal-claim-contexts` -> `final-node`.

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
`ProcessingScheduler.finish()` still describes a complete visited graph, not a
partial-update API. Source change detection, dependency completion, normalization,
validation and publication remain separate obligations.

## Bounded direct-carrier computation

`tos_access.addressed_replacement.replace_direct_carrier_candidate` accepts one
exact prior normalized direct carrier, its complete replacement source record,
supplied incident relation rows, all their other endpoint rows, and the exact
normalization binding and registries. It uses the same local implementation as
`addressed_update_knowledge_graph`: direct-source/claim/dossier/reference-context
guards, endpoint statements, inherited views, readable context and content
revisions remain shared. It does not discover source records or relations.

The candidate entry checks normalized identities, source-envelope digests and
content revisions. These are self-consistency checks, not independently admitted
baseline provenance. Endpoints exclude the prior carrier; missing required
endpoints, duplicate rows and nonincident supplied relations fail. Empty supplied
incidence is permitted but **never verified complete**. Identity/type changes
retain the local computation's previous behavior; a candidate cannot authorize
them or replace the full wrapper's global semantic validation.

`ReplacementLimits` defaults to 1024 relations, 2048 other endpoints, 16 MiB of
compact UTF-8 JSON input (including both registries and binding), and 16 MiB of
output rows. Counts and input bytes are checked before cloning or normalization;
output bytes are checked after each computed row before it joins the result.
Only exact JSON types, finite numbers and at most 64 container levels qualify.
Limits refuse work; they are not a process RSS bound. No partial result escapes.

The result contains detached candidate node/relation rows, accounting and an
explicit `supplied-neighborhood-local-only` scope. It provides no source revision,
catalog, global semantic validation, incidence completeness or current publication
claim. Ambient normalization-cache writes are disabled and restored on exit; no
partial processing run is published. The full wrapper still extracts incidence
from complete arrays and performs its original global scans/validation, without
acquiring this new public entry's refusal caps. It shares the computation, not
the narrower admission profile. Source-owner paired before/after membership and
source-to-COW-to-prepared publication remain unimplemented.

Focused tiny-fixture checks: `PYTHONPATH=access/src:access/tests python -m unittest
test_addressed_replacement test_processing test_builder_ownership`.

Focused checks: `PYTHONPATH=access/src:access/tests python -m unittest
test_processing_closure`. Existing processing tests retain ownership of full-run
evaluation, cache reuse, failure recovery and publication conflict behavior.
