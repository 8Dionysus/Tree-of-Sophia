# Snapshot-indexed node and relation inspection

Date: 2026-09-07 UTC. Baseline: `79f70590ce0399e18a117cc4339aaef286f6d3ca`.
Owner: `access/`. This is local read-path evidence, not source acceptance,
Foundation v1 completion, historical snapshot support or deployment.

## Behavior and contract

`ToSAccessCore.knowledge_node` and `.knowledge_relation` now share one lazily
prepared `KnowledgeGraphIndex` per core's current immutable graph. It stores
references to records and ordered incident positions, not copied payloads.
Replacing the graph replaces this index, even if both graphs carry the same
source-revision string. A supplied index from a different graph is rejected.
Concurrent first reads serialize index creation; later readers reuse it.

Inspection keeps exact-ID precedence, shared entity carriers, ambiguous native
IDs, all original match records, stable edge ordering, source refs, counts,
assertion/form context and relation endpoint closure. Self-loops occur once in
a node's incidence list. Combining shared-carrier neighborhoods removes only
duplicate input positions, not independently supplied records with equal IDs.
The read adapter does not adjudicate identity or repair invalid source data.

For one resolved node, total degree is read from the index and only the
requested prefix (up to the existing 1,000-relation limit) is materialized.
Shared identities/ambiguous matches still require work proportional to the
matching carriers and their combined incidence. Full packet serialization and
large source fields retain their own costs. An index is not a graph-size budget.

Tests compare indexed and unindexed packets and make iteration over global
node/relation lists fail after preparation. A separate guard forbids scanning
the incident list for a single-node bounded prefix. They also cover identity
precedence, shared/native aliases, loops, repeated records, invalid requests,
wrong snapshots, returned-list isolation, concurrent preparation and replacement
of an index under an unchanged source-revision string.

## Real corpus and paired observations

CPython 3.14.7; the same five public input SHA-256 values as the
[codec comparison](2026-09-07-normalization-revision-codec.md). They were
rehash-checked after this run and unchanged. No persistent cache, source write,
remote query, network serialization or running-service replacement was used.

The existing migration consumer again found all 460 ready source-copy forms
for all 171 bibliographic subjects, with no semantic admission. Its full
171-subject inspection pass, including index construction and reading the
source-form sets for comparison, took 0.339596 seconds. Focus and compact
delivery retained their previous behavior; they were not accelerated here.

An independent same-snapshot comparison prepared the index once (0.319516 s),
then alternated plain/indexed/indexed/plain inspection:

| Input packet set | Plain seconds | Indexed seconds | Comparison |
| --- | --- | --- | --- |
| all 171 bibliographic subjects, relation limit 0 | 8.223256; 7.450831 | 0.004842; 0.000836 | every packet equal |
| 107 relation samples, including every source graph | 3.239416; 3.214033 | 0.005164; 0.000870 | every packet equal |

These paired timings exclude index construction and JSON/network transport.
The faster second indexed pass is visible, not averaged into an assumed SLA.

The ordinary core then inspected **every** current normalized exact ID. All
39,461 nodes returned their exact source-bearing carrier; all 58,882 relations
returned their exact carrier and correct endpoints. The same index was reused
throughout, including ordinary core snapshot-file checks on each call.

| Actual core pass | Wall seconds | User / system CPU seconds | Observed p95 per call | Observed maximum |
| --- | ---: | --- | --- | --- |
| 39,461 nodes, relation limit 0 | 3.672042 | 2.547798 / 1.092018 | 0.101197 ms | 4.011334 ms |
| 58,882 relations | 5.844233 | 4.240160 / 1.566508 | 0.107433 ms | 1.161061 ms |

Those percentiles describe this one prepared, local, in-process pass only.
Cold graph preparation still took 18.386765 seconds before the index build.
The process peak was 1,192,752 KiB; it includes the graph and comparison work,
not isolated index memory. Growing-data, concurrent-load, transport, first-use
and memory-budget acceptance remain open.

Measured source SHA-256:

- `knowledge.py`: `b4170cd696631d858ec57494354573958a231b3a717ffabfc0a4340c2dafde5b`
- `core.py`: `88976fefc7306e11f6ee955244fab1b6416d61ac79d3ac690c7a0c817aaefa29`

The normalization processor digest remains
`611ab3bee1cad1234ecf1820dff5da59d8583d1909bd879e7f3cca4e772b2688`
relative to the baseline: this query-only helper change requires no corpus
normalization or export rebuild. Source and public packet revisions are intact.

## Reproduce, review and rollback

```bash
python -m unittest discover -s access/tests -p test_knowledge_contract.py -k inspection_index
python -m unittest discover -s access/tests -p test_access_contract.py -k core_inspection
python -m unittest discover -s access/tests
```

Both focused checks passed, followed by all 138 access tests (148.330 s),
source-home validation and `git diff --check`. Standalone source-profile
validation passed in the immediately preceding codec checkpoint; it was not
repeated for this inspection-only diff. No full release or remote CI gate is
claimed for this checkpoint.

For real-input comparison, load one graph with `ToSAccessCore.discover(root)`
and `.knowledge_graph()`. Construct `KnowledgeGraphIndex(graph)` once. Compare
`inspect_knowledge_node(graph, id, 0)` with the same call using
`graph_index=index`, and do the corresponding relation comparison. Compare
whole packets, not counts. Then iterate all graph node/relation IDs through
`core.knowledge_node(id, 0)` / `core.knowledge_relation(id)`, checking exact
matches and relation endpoints; time these separately from preparation and
serialization. Rehash inputs afterward. Select a bounded admitted resource
envelope before running this on a larger graph.

Manual source/consumer review found no schema, identity, source-return,
authority, uncertainty or language change. Restoring the former scan-based
core calls rolls back this performance change without changing corpus records,
public revisions or history. The first read after a process restart still
prepares its graph/index; no live process was restarted here.

This does not optimize LensSpec/focus, search, catalog, historical querying or
Cloudflare/D1. It does not prove CI, merge, UI interaction or deployment. Those
requirements remain in [the foundation map](../doctrine/FOUNDATION_V1.md).
