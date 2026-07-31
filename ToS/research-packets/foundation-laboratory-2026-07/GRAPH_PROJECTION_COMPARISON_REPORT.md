# Graph Projection Comparison Report: Foundation Pilot v1

Status: variants A, B, and C mechanically executed and corrected; real human graph review not started
Experiment: `tos-graph-projection-v1`
Frozen input: 13 unreviewed claims and 10 questions, sealed before variant output
Execution date: 2026-07-22

## Result first

All three routes can now reproduce the frozen claim semantics and preserve
their audit paths:

- A reads the canonical JSON/JSONL claim packets directly and remains the
  zero-database reference semantics.
- B projects the same claims into an isolated namespace in resident Neo4j as a
  claim-first property graph.
- C projects them into a private PyOxigraph disk Store as explicit claim
  resources with one named RDF graph per claim.

The corrected variants all:

- preserve 13 claims in four separately queryable layers: 4 bibliographic,
  2 textual, 3 provenance, and 4 interpretive;
- keep both members of two unresolved translation/sign claim families;
- keep stable ToS identity refs separate from two literal deferral values;
- mechanically resolve all 13 claim routes against current tracked ToS owner
  records;
- match all 10 model-proposed frozen expectations;
- remain `awaiting-manual-review`.

This does not establish query-answer correctness. The expectations and the
five source-visible inspections per corrected variant were produced by
`model:codex`; they are advisory, non-human, and cannot authorize promotion.
Every claim remains `unreviewed`, every human-review array remains empty, and
the quality and human-cost metrics remain `null`.

Three real defects were preserved instead of rewritten:

1. the first A runner confused field presence with owner-resolved
   traceability;
2. the first B query projection leaked two literal status values into stable
   identity results and matched only 8/10 advisory expectations;
3. the first C launch followed the ordinary venv `bin/python` symlink into
   `/usr/bin` before checking runtime ownership and rejected the correctly
   installed host runtime before materialization.

No graph-backend winner is declared.

## Subsequent catalog-backed bibliographic projection — 2026-07-30

The later source-witness foundation now has a separate tracked JSON graph at
`ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`.
This is not a fourth backend and does not rerun or rewrite the frozen 13-claim
A/B/C experiment. It applies corrected A's claim-first reference semantics to
the current generated source-witness catalog:

- 31 public-metadata-only bibliographic claims;
- 97 nodes and 202 edges;
- 31 claim trace packets;
- 12 claim-scoped literal objects;
- five resolved provenance events carrying exact time and method;
- zero direct subject-to-object edges;
- 31 explicit `unreviewed` states.

Every structural edge begins at a reified claim node and carries the source
claim digest, source file/line, evidence nodes, maker, provenance event, and
review posture. The projection fails closed when a subject, identity-like
object, evidence reference, provenance event, source line/digest, alternative,
or superseded claim cannot resolve. It reads the catalog and source packets;
it does not replace either.

The atlas/view projection remains a different derived surface. It was not
expanded because its 11 view lenses, clusters, and review packets belong to
the philosophy-domain graph workbench rather than to source-witness
bibliographic claim closure.

This subsequent projection supplies a portable current-corpus reader for
future `abyss-stack`, Neo4j, Oxigraph, MCP, UI, or site work. It supplies no new
human correctness evidence, no backend winner, and no claim promotion.

### Independent source-return inspection

A separate shell/JQ inspection did not call the builder or validator. Its first
digest pass retained JQ's final newline and therefore falsely reported 31/31
claim-line and 59/59 line-backed-node mismatches. The graph digest contract is
over canonical JSON without that serialization newline. The check was
corrected rather than the projection being changed.

With the newline removed, the independent pass found:

- 31/31 source claim lines with the expected canonical digest;
- 59/59 line-backed claim, anchor, and provenance nodes with the expected
  canonical digest;
- 26/26 identity nodes matching their canonical source record;
- 11/11 path-evidence nodes matching the raw referenced file;
- 202/202 edges beginning at their own claim node, with zero malformed edges;
- 31 claim nodes and 31 trace packets;
- zero direct subject-to-object edges;
- zero provenance nodes missing start time, end time, or method.

Three source-visible model inspections then traversed one aggregate membership
claim, one `Ecce Homo` editor responsibility claim, and the structured nominal
`Zweite Auflage` issue-state claim. They resolved identity-versus-literal
typing, evidence, maker, provenance time/method, and empty human-review routes
back to the exact source packet. These are advisory nonhuman trace inspections,
not truth review.

## Frozen graph soil

Tracked inputs:

- `graph-claims.jsonl`: 13 claim packets, SHA-256
  `50f21b424c0f93b8824472c3340dd9d8c7635af4520a90fb58404e3f46db2319`;
- `graph-queries.json`: 10 questions, SHA-256
  `0a6a541b45367fcdd275a32d99bda34124e56310087b75245e7e9fa51dd597ad`;
- `graph-provenance.jsonl`: the pre-output freeze event;
- `graph-query-plan.schema.json`: the four-layer query contract.

The packet deliberately contains:

1. a German/Russian translation-alignment candidate and a claim deferring the
   alignment until reviewed transcriptions and human alignment exist;
2. a recurrence-sign candidate and a claim deferring sign identity until
   occurrence, lemma, translation, and human review exist.

These are unreviewed laboratory inputs. They test whether a projection
preserves disagreement, literal status, identity, provenance, and review
state; they are not canon candidates merely because all three readers can
store them.

## Preserved runs

Runtime root:
`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-graph-projection-v1/`

| Run | Method digest | Build/materialization | Median warm query | Measured bytes | Host runner peak RSS | Outcome |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `foundation-pilot-v1-graph-a-20260722` | `f2058eaa…4009` | 0.000039 s | 0.002685 ms | 42,169 packet bytes | 32,030,720 | retained negative: field presence mislabeled as trace resolution |
| `foundation-pilot-v1-graph-a-resolver-20260722` | `27c9f3be…2300` | 0.023353 s | 0.002290 ms | 79,827 packet bytes | 32,071,680 | corrected A: live owner-reference resolution; 10/10 advisory matches |
| `foundation-pilot-v1-graph-b-neo4j-20260722` | runner `eddb2249…f1`; bridge `ad83312a…fca` | 1.527820 s | 4.549903 ms | 156,038 packet bytes | 32,301,056 | retained negative: literal status leaked into identity results; 8/10 advisory matches |
| `foundation-pilot-v1-graph-b-neo4j-typedrefs-20260722` | runner `eddb2249…f1`; bridge `46589d01…81c` | 0.065963 s | 2.826839 ms | 156,132 packet bytes | 32,284,672 | corrected B: typed query boundary; 10/10 advisory matches |
| `foundation-pilot-v1-graph-c-oxigraph-20260722` | runner `690381a6…03f7` | not materialized | not run | launch receipt only | not measured | retained negative: lexical runtime ownership was erased by symlink resolution |
| `foundation-pilot-v1-graph-c-oxigraph-runtimepath-20260722` | runner `1ca27b2b…f94`; bridge `bd26b372…4df` | 0.074591 s | 0.179564 ms | 306,207 isolated-store bytes; 470,377 packet bytes | 33,464,320 | corrected C: deterministic named-graph RDF projection; 10/10 advisory matches |

The corrected B run measured deletion at 0.216974 seconds and rebuild at
0.058067 seconds. It uses resident Neo4j 5.26.26 Community; exact lab-only
database bytes are not exposed by the shared service, and container memory
includes unrelated state.

The corrected C run measured deletion at 0.000359 seconds and rebuild at
0.078919 seconds. Its 537 quads occupy 14 named graphs: 13 claim graphs and one
manifest graph. It preserves 25 evidence statements, four directed
`alternativeTo` statements, 13 direct assertions, and two literal-object
claims. First and rebuilt canonical N-Quads are byte-identical at SHA-256
`fef3dfe159d2aa7e9fd4001c2adc117447f19f43231b3a0a47786e6f3ec8b776`.

These tiny sequential observations are not a performance ranking. A has no
resident service, B reuses a warm shared service, and C starts an isolated
library-backed Store. Their lifecycle and cost boundaries differ.

## C runtime and mapping receipt

Corrected C used:

- PyOxigraph 0.5.9, released 2026-06-18;
- Python 3.14.6 in
  `/srv/abyss-machine/runtimes/tree-of-sophia-foundation-lab/pyoxigraph-0.5.9`;
- wheel SHA-256
  `4558d430bbad6e6b4ba98e0e89a2e28402211069d38e6e9b00083ae2d9d2d175`;
- upstream source revision
  `d6f5b98941f4b0d1f09d8ce822929670a6a359a6`;
- runtime-manifest SHA-256
  `7c2c12461fbfe5cad9c7e51ad8fecfc958c15b69ea5cb34876b9103a179cf930`;
- reviewed license expression `MIT OR Apache-2.0`.

The mapping uses explicit claim resources rather than RDF-star as the
authority model. Each direct subject-predicate-object assertion lives in its
claim-specific named graph. Identity objects are named nodes with `refValue`
and `refKind`; deferral values remain RDF literals. Claim resources carry
maker, provenance event, evidence, layer, epistemic status, review status,
review count, and alternative links.

The path query intentionally uses a SPARQL claim-edge inventory followed by
bounded deterministic BFS. It is not presented as a general SPARQL property
path benchmark.

## Source-visible checks actually performed

The first A run has four advisory model inspections; the traceability
inspection rejects its field-presence claim. Corrected A has five advisory
model inspections against live tracked work, expression, edition, item,
claim, event, rights, and anchor records.

The first B run has two rejecting advisory model inspections for literal
status returned as identity. Corrected B has five source-visible advisory
inspections.

Corrected C also has five advisory inspections. They checked:

- the Work → Expression → Edition → Item path and its three claim IDs against
  claim-specific named-graph assertions;
- both members and both anchors of the translation-alignment family, while
  confirming that its deferred status remains a literal;
- both members and both anchors of the recurrence-sign family, with the same
  identity/literal separation;
- all four layer counts against all 13 frozen claims;
- all 13 claim resources, direct assertions, maker/event/evidence/review
  routes, owner-resolved edge traces, and both alternative families.

The C traceability query consumes `canonicalTraceable` values supplied only
after owner resolution. It therefore proves reproducible trace carriage and
closure, not an independent truth audit.

All source-visible inspection receipts identify `model:codex`, carry
`advisory-nonhuman`, and set `promotion_authorized: false`.

Human graph-review receipts across A/B/C: **0**.

## A/B/C projection comparison

| Concern | Corrected A: canonical JSON/JSONL | Corrected B: Neo4j property graph | Corrected C: RDF/Oxigraph |
| --- | --- | --- | --- |
| Authority | reads owner records directly | replaceable derived namespace | replaceable derived disk Store |
| Claim identity | explicit claim records | explicit claim nodes plus `claim_id` on every direct assertion | explicit claim resources plus one named graph per claim |
| Reference typing | owner IDs and literals read from claim fields | `ref_kind`; identity results and literal detail separated | named identity nodes with `refKind`; literal RDF terms retained |
| Alternatives | traversed from records | four typed alternative relationships | four `alternativeTo` statements across two families |
| Provenance/review | maker, event, evidence, and review fields | claim-centered properties and relationships | claim-centered PROV/event, maker, evidence, status, and named-graph routes |
| Query route | deterministic Python | bounded Cypher catalog | bounded SPARQL catalog; deterministic BFS for path |
| Lifecycle | rebuilds in process | unique lab namespace; delete/rebuild proven | exact run-local store; delete/rebuild and canonical dataset digest proven |
| Portable dataset | canonical JSON/JSONL owner packets | Cypher/import and query receipts; shared database not self-contained | sorted canonical N-Quads plus query catalog |
| Isolated store cost | no database | unavailable inside shared Community database | 306,207 bytes for this tiny Store |
| Resident service burden | none | existing long-lived Neo4j service | none; isolated host-managed Python runtime |
| Human correctness evidence | none | none | none |

A is the simplest semantic reference. B supplies navigable property-graph
behavior in an already resident operational service. C supplies a compact,
self-contained RDF dataset with stronger named-graph portability and exact
isolated store measurement. Those are architectural observations, not a
quality winner.

## Decisions

1. Retain all three first defects as negative evidence; never rewrite failed
   or completed packets into green history.
2. Keep corrected A as reference semantics and minimum viable graph route.
3. Admit corrected B and C only as `awaiting-manual-review` projections.
4. Require actual ToS owner resolution before any projection calls a route
   traceable; field presence is insufficient.
5. Keep stable identity refs and literal assertion values typed separately in
   storage and query results.
6. Keep explicit claim identity at the center of every direct assertion.
7. Keep bibliographic, textual, provenance, and interpretive layers
   separately queryable.
8. Preserve rejected, deferred, ambiguous, and competing claims; import must
   not collapse them into a winning edge.
9. Require isolated namespace/store lifecycle and deletion/rebuild proof for
   every database projection.
10. Keep RDF explicit-claim/named-graph mapping versioned; do not make
    RDF-star or a graph engine the sole claim owner.
11. Do not promote any of the 13 claims until real human review occurs.
12. Compare projections on faithful source return, human trace cost,
    maintenance, portability, and site fit—not visualization or one latency
    number.

No new ToS ontology decision follows from this 13-claim packet. The experiment
supports the existing claim-first owner boundary.

## Human comparison still required

A genuine projection decision still needs:

- blind source-visible judgment of all 10 frozen answers;
- 10 random and 10 hard edge audits under the declared manual protocol;
- independent checking of both competing claim families without treating the
  recognized reading or a model expectation as automatic truth;
- measured human trace-review minutes;
- maintenance and failure-recovery cost over repeated runs;
- larger-scale evidence before extrapolating from 13 claims;
- a future-site read-only consumer trial proving that no graph database becomes
  the source of truth;
- a second human check before any promotion.

Until then, the proven result is deliberately narrow: canonical claim packets
remain the owner and reference semantics; both Neo4j and Oxigraph can project
this frozen packet faithfully after their discovered boundary defects were
preserved and corrected; automatic green mechanics still cannot establish a
bibliographic, translation, sign, concept, or philosophical truth.
