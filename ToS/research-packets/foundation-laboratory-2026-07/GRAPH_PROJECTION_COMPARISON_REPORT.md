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

## Subsequent catalog-backed bibliographic projection — refreshed 2026-08-01

The later source-witness foundation now has a separate tracked JSON graph at
`ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`.
This is not a fourth backend and does not rerun or rewrite the frozen 13-claim
A/B/C experiment. It applies corrected A's claim-first reference semantics to
the current generated source-witness catalog:

- 108 public-metadata-only bibliographic claims;
- 357 nodes and 720 edges;
- 108 claim trace packets;
- 27 claim-scoped literal objects;
- 19 resolved provenance events carrying exact time and method;
- zero direct subject-to-object edges;
- 108 explicit `unreviewed` states.

The 55 topology claims materialize the complete declared corpus ladder as
separate source packets: 20 Work `has_expression` Expression, 20 Expression
`embodied_by` Edition, and 15 Edition `exemplified_by` Item relations. These
are bibliographic topology claims, not textual-equivalence assertions.
The responsibility layer separately closes seven Work `authored_by` Friedrich
Nietzsche claims—one per current Work—without deriving authorship from folder
names or collapsing it into a generic creator edge.
It also carries eight Expression `translated_by` claims. The newest route is
the exact 1913 Antonovsky Expression: its evidence node resolves a proposed
page-7 title-page anchor to the Item and file digest, while the separate 1996
claim remains attached to its own Expression. Sharing an Agent object creates
no textual-equivalence edge between those Expressions.
The chronology layer separately carries seven Work
`first_publication_chronology` literal profiles. Their staged/private/public
and posthumous distinctions remain claim objects; the graph does not select a
single Work year or convert ordering into truth.

The 2026-08-01 bounded provision surface contains eight Edition-owned literal
objects, three normalized Places, and five distinct historical Organizations.
The original two-case A/B/C result remains frozen; a separate
extension adds only the exact 1883 first-part *Zarathustra* statement, Chemnitz,
and the historical Schmeitzner Corporate Body. A later pass adds part II and
part III from their own exact DTA records, not from the first claim or repeated
labels. A further exact part-IV pass reports only `Naumann; Leipzig; 1891`
while retaining the 1885 private print, 1890 printing, planned 1891 delivery,
and March-1892 actual delivery as separate chronology evidence. The exact 1913
Antonovsky Item then contributes one publication claim across its cover and
title page plus a separate manufacture claim for the following-page printer
line. All sixteen normalized-participant edges begin at claim nodes. Exact normalized queries
return three claims for Leipzig, three distinct claims for Chemnitz or
Schmeitzner, two claims for Saint Petersburg, and one exact part-IV subject
result. The modern Berlin Insel
successor, Person Ernst Schmeitzner GND, Naumann printer, and founder Person
remain outside the normalized claim routes; Vladimir Posse and the
brothers-Linnik printer-as-publisher shortcut are likewise absent. The graph creates no direct
Edition-to-Place or Edition-to-Organization assertion.

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

A separate one-off inspection read raw records, item manifests, source claim
files, the generated catalog, and the generated graph without importing the
builder or validator. It found:

- exact raw-record/claim closure for 20 `has_expression`, 20 `embodied_by`,
  and 15 `exemplified_by` relations;
- exact one-to-one closure between seven current Work records and seven
  `first_publication_chronology` claims, including one staged and six
  single-event profiles;
- byte-level agreement for the chronology claim-file output digest and all 18
  input digests recorded by its provenance event;
- 108/108 catalog rows returning to the expected source line and canonical
  claim digest;
- 108/108 graph traces returning to the same source line and digest;
- 357 nodes, including 108 claims, 73 identities, 129 evidence nodes, 27
  literals, one maker, and 19 provenance events;
- 720/720 edges beginning at their own reified claim node;
- zero direct subject-to-object edges.

The original two provision rows were additionally inspected from metadata
snapshot to raw source claim, catalog line, graph literal, normalized
identities, and negative query. Their frozen experiment receipt preserves that
direct check, the rejected flat-field representation, schema mutations, and
local cost snapshot. The later first-part *Zarathustra* row was independently
inspected through positive Chemnitz/Organization and negative part-II,
part-III, and Person-GND queries at its checkpoint. The subsequent part-II and
part-III rows were each inspected by exact subject and source return; the
Chemnitz query now returns three distinct claims, while the Person-GND control
still returns no match. The Antonovsky pair was inspected by exact Edition and
Saint Petersburg queries: publication reaches only the `Жизнь для всех`
Organization, manufacture reaches only the brothers-Linnik Organization, and
both retain their own source-return paths. None of these episodes creates human
review evidence.

A temporary negative probe then removed one Work-owned
`expression_claim_refs` entry and ran the actual foundation validator. It
failed with the exact missing closure instead of accepting the still-present
record link or generated graph as a substitute. These are mechanical and
nonhuman trace checks, not truth review.

A second isolated negative probe removed the *Antichrist* Work's sole
`responsibility_claim_refs` entry. The actual foundation validator rejected
the orphaned `authored_by` claim, the stale catalog, and the changed
digest-bound Work input together. The production tree was not altered by that
probe.

A third clean-copy baseline passed before two chronology mutations were tried
separately. Changing the staged *Zarathustra* interval end from 1885 to 1884
was rejected because it no longer matched the last stage, no longer matched
the provenance digest, and made the generated catalog stale. Restoring that
claim and removing the Work's sole `chronology_claim_refs` entry was rejected
as a missing exact chronology closure, a changed topology-provenance input,
and a stale catalog. The temporary copies were then deleted; the production
tree was not altered.

### Current-corpus read-only query route

The subsequent projection now has a repository-local stdout query reader. It
does not add a graph backend or change the frozen A/B/C result. Before
answering, the reader validates the tracked payload, verifies its projection
fingerprint, rebuilds the complete graph from the exact catalog and source
packets, and requires canonical parity.

Manual probes outside the unit-test harness established:

- the exact `Ecce Homo` `edited_by` claim returned the Edition and Raoul
  Richter identity nodes, source JSONL line 2 and its canonical digest,
  evidence, `model:codex` maker, timestamped annotation event, and unchanged
  `unreviewed` status;
- the exact *Der Fall Wagner* nominal-issue claim returned a literal object
  rather than an identity, preserving both textual identity and textual
  difference as `unresolved`;
- an AND query for the *Mysl* collection subject plus `contains_work`
  returned exactly the seven sorted membership claims;
- a Work plus `has_expression` query returned all eight separately sourced
  *Zarathustra* Expressions;
- a cross-tree `embodied_by` query returned the Russian *Zarathustra*
  Expression and the aggregate *Mysl* Edition as one exact claim, without a
  textual-equivalence assertion;
- an Edition plus `exemplified_by` query returned the two distinct 1893
  Naumann Items, each with the Edition record, Item record, and item manifest
  as three evidence nodes;
- the same Expression paired with an unrelated *Ecce Homo* Edition returned
  explicit `no_match`;
- a bounded `authored_by` query returned exactly seven sorted Work claims, each
  to the Friedrich Nietzsche Agent;
- a bounded `first_publication_chronology` query returned exactly seven sorted
  Work claims from the dedicated chronology source file; the *Zarathustra*
  source return preserved four stages with `public/public/public/private`
  availability;
- the same chronology predicate paired with a *Der Fall Wagner* Edition
  returned explicit `no_match`, and limit 6 rejected all seven matches instead
  of truncating them;
- an *Antichrist* Work plus Raoul Richter plus `authored_by` query returned
  explicit `no_match`;
- a selector-free invocation exited 2 instead of dumping the graph;
- a 20-match `embodied_by` request with limit 19 exited 2 instead of returning
  a silently truncated result; a seven-match `authored_by` request with limit
  6 likewise exited 2, and the 99-match `unreviewed` route remains
  over-limit by default.

The output is deterministic navigation and includes the exact source claim
object. It is not human trace-time evidence, bibliographic acceptance, a
runtime API, site contract, or publication surface.

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
