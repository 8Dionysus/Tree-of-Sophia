# Normalization revision codec: compatible cost reduction

Date: 2026-09-07 UTC. Baseline: `a65e48b13e7c127448b784cc563e1741d7e63500`.
Owner: `access/src/tos_access/knowledge.py`. This is a local performance and
compatibility observation, not completion of Foundation v1 scaling or a runtime
deployment. It changes no authored knowledge, schema, source record or export.

## Cause and change

One instrumented ordinary-core build covered 39,461 nodes and 58,882 relations.
Its 101.654 seconds include profiler overhead, so are **not** normal latency.
Stable digest encoding accounted for 65.447 seconds and 28,133,549 recursive
calls, including repeated generic dispatch for known string dictionary keys.

The codec now handles string values first, encodes dictionary keys directly,
and binds the streaming hash writer once. The existing byte protocol is
unchanged: UTF-8 byte lengths, sorted/coerced keys, framed containers, binary64
numbers, normalized negative zero, distinct booleans/null, rejection of
unsupported/non-finite values, and bounded reuse of short string tokens.
Long text still streams without retaining another full wire representation.
No whole-object memoization or skipped validation was introduced.

The existing independent wire-format test now also covers long/non-ASCII object
keys on both sides of the short-token boundary and coerced non-string keys.
This protects revisions consumed by source-return packets, inspectors, lenses,
and continuation readers; it does not prove semantic assessment or D1 runtime.

## Paired local comparison

CPython 3.14.7, the same loaded public inputs, no persistent normalization cache.
Each pass rebuilt the complete graph, clearing the bounded token cache first.
Order was baseline, updated, updated, baseline. Timings exclude input loading,
output comparison and subsequent queries. Other host workloads were not stopped.

| Pass | Wall seconds | User CPU seconds | System CPU seconds |
| --- | ---: | ---: | ---: |
| baseline 1 | 19.246842 | 18.824821 | 0.369217 |
| updated 1 | 17.312426 | 16.901127 | 0.353819 |
| updated 2 | 17.690277 | 17.232885 | 0.393568 |
| baseline 2 | 19.183589 | 18.788902 | 0.332919 |

Mean wall time fell about 8.9%; this is two observations per implementation,
not a p95 budget or a hardware-independent speedup. The shared process peak RSS
was 1,181,856 KiB; this cumulative peak cannot attribute memory savings to either
codec. Prior 27-second cold-core observations included different execution
conditions and must not serve as this comparison's baseline.

All four complete graph fingerprints matched, including source/content
revisions, forms, source records, assertion context and semantic-validation
results. Every input file was hashed again after the final pass and unchanged.
Inspection of Laws of Hammurapi, depth-one focus, `Hammurapi` search, and the
complete catalog also returned identical packet digests across all four passes.
Inspection took 35.6–36.8 ms, focus 184.7–203.8 ms, uncached search 3.60–3.93 s,
and uncached catalog 5.77–6.30 s. The codec change does not solve those scans.

Input SHA-256 values:

- corpus index: `a1296c7b6670985657dc06ee4e0f4792ab8b93bdaad907aa6e647685041c3148`
- philosophy projection: `dece31afca3c60df28a780f1ea5f969625e6cc7272d3cbfa526da9c714ea61d7`
- bibliographic graph: `b6d9833522bd900811f901fd0bd2a7615041962988d1a6ef09ca3b13458cfb17`
- entity registry: `3cc9ba54790e603dfb99f7158107e8277ae3a0984a6440ce1931aa2cfebf9bfb`
- relation registry: `c5a17624a5e8f326a98155bb8340f0cde03a0daed4ed6e0fef2f7172de306aa2`

Measured updated `knowledge.py` SHA-256:
`4cd3edff7777384d7b144b16ab1ec318079e83b4ce1b5072b62594e403938dd5`.

## Reproduction and boundary

The permanent compatibility check and affected product lane are:

```bash
python -m unittest discover -s access/tests -p test_knowledge_contract.py -k stable_revision_wire
python scripts/validation_lanes.py --run standalone_access
```

Both passed: the focused wire check, all 136 access tests (140.767 s), and
standalone source-profile validation (`ok: true`). Source-home validation and
`git diff --check` also passed. Full repository release checks, CI, merge and
live UI/Worker/D1 behavior were not rerun for this codec-only change.

For paired cost reproduction, load the five public inputs named by
`ToSAccessCore` once. Extract `_stable_digest` from the exact baseline using
`git show a65e48b13:access/src/tos_access/knowledge.py` and Python AST. Bind that
function in the current module's globals, then alternate it with the updated
function in baseline/updated/updated/baseline order. Clear
`_short_digest_string.cache_clear()` before each `build_knowledge_graph(*inputs)`;
measure `perf_counter` and `getrusage` around the build only. Compare every
top-level field and every ordered node/relation, not counts alone; compare the
four named query packets as well. Rehash inputs afterward. This uses about
1.2 GiB of process memory on the measured corpus; select an admitted local
resource envelope before repeating or growing the dataset. No serving data or
cache needs to be changed to reproduce it.

Manual review found the source-return, identity, uncertainty, language and
owner boundaries unchanged. Review/assessment/canon/publication states remain
source-owned. Rollback is restoration of the previous codec implementation;
it does not roll back corpus writes or invalidate public revisions. Internal
processor-bound normalization caches correctly regard changed helper code as
a new processor and may recompute once.

Cold startup, first catalog/search, address-local mutation costs, growing-data
budgets and UI/Worker/D1 integration remain open in
[the foundation map](../doctrine/FOUNDATION_V1.md). No deployment, restart,
source migration or cleanup was performed in this change.
