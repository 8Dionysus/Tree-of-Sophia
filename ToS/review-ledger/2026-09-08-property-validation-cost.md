# Property applicability: full validation with bounded repeated work

Date: 2026-09-08 UTC. Owner: `access/src/tos_access/knowledge.py`.
Source baseline: `a3698220fbce71a053e81547e243aa9285513285`; its access code
is identical to `e844ef3da6c70c8944426f0cd8bf824197219595`, used in the clean
comparison checkout. This review concerns a deterministic reader optimization,
not semantic acceptance, full Foundation v1 scaling or a deployed UI fix.

## Cause, change and retained checks

A cold ordinary-core profile on the complete current public inputs found that
every instance repeated property applicability and inherited-type traversal.
The new invocation-local table selects the same ordered definitions once for
each encountered type. It does not store field values, instance verdicts or
payload copies, and never survives into another registry/snapshot invocation.
Required properties, value types, unknown/abstract types, source mapping,
endpoints, Claim consistency, cardinality and recorded review gaps still run.
The processor fingerprint includes the whole validator and its nested helper;
an opt-in normalization cache therefore observes this implementation change.
No public revision codec, source authority or access ABI changed.

Two permanent tests protect a continuing growth boundary: property-registry
expansion must not multiply repeated ancestry work by the number of instances,
and mutable registry/type changes must take effect at the next validation.
The fixtures also check independent invalid, missing, boolean-as-number and
array-element values, non-inherited parent properties, exact error ordering,
and unchanged input objects. Synthetic fixtures are not historical knowledge.

## Same-graph comparison

CPython 3.14.7; 41,365 nodes and 61,279 relations; no persistent normalization
cache. Only the newly launched measurement process was restricted to CPU 0
within its already admitted mask. Existing applications and host policy were
untouched. The harness extracted the baseline validator by AST and verified
that all other module definitions were identical before binding it to the
same dependencies. Normalization and input loading are outside these timings.

| Unprofiled pass | Wall seconds | Process CPU seconds |
| --- | ---: | ---: |
| baseline A1 | 5.923015 | 5.887766 |
| candidate B1 | 0.664873 | 0.661159 |
| candidate B2 | 0.664321 | 0.660291 |
| baseline A2 | 5.929428 | 5.896337 |

This is about 8.9 times faster for this complete validation step, from two
samples per implementation on one corpus. It is not an end-to-end latency or
p95 claim. Separate profiled passes counted 6,617,559 versus 140,995 calls to
`_type_is_a`, and 179,705,696 versus 12,712,335 total function calls. Their
69.061/6.738 seconds include profiler overhead and are not normal latency.
Both versions still invoked `validate_node` exactly 41,365 times.

Every complete report matched, including the ordered 17,564 review/evidence
gaps, not only its green status. Report SHA-256:
`1219d4d271fedea212cf59cbdd207d17e629a69c60b2df920a0bbd8917944556`.
The graph was unchanged before/after all six passes; full framed-record hash:
`178858cced20e5382959e1bcf854f8b6882c7371829997f152a607deb58c5b03`.
Process peak RSS was 1,296,080 KiB; the admitted unit reported 1.2 GiB and
zero swap. This cumulative peak cannot attribute a memory saving.

## Cold consumer observations and limits

Separate fresh-process observations called actual HTTP boot preparation, then
the ordinary core's default UI focus lens and final response serialization.
They did not run HTTP transport, browser rendering or concurrent requests.
All complete graphs, boot packets and lens responses matched. The serialized
lens response was 657,990 bytes, SHA-256
`aa4991338c15750b380e74c2f1dcad76992cfa44334d521b6f23b0017c03fd5e`.

Initial unrestricted-process cold lens timings were 18.375 seconds baseline
and 31.167 seconds candidate; unchanged boot and other work also varied.
The subsequent CPU-0 ABBA sequence was 21.687, 26.630, 22.322, 39.696 seconds.
This variation prevents a defensible end-to-end speedup claim. Three warm
requests in each pinned pass ranged from 0.175 to 1.209 seconds. These samples
are application-cold, not storage-cache-cold, and do not explain that warm
variation. The prior UI canary's greater-than-60-second cold failure was not
reproduced here; its cause and durable latency budget remain open. This
optimization does not justify raising the UI timeout or calling cold loading
solved. Global eager normalization, remaining hashing, query work, concurrency
and growing-data budgets retain their own checks.

The UI owner's separate actual HTTP/browser repeat on unchanged backend
`e844ef3da` plus UI `231183c09b172368b8b5bc7e16c83819da3e750b` recorded one
natural cold compile request, 40.255911 seconds, HTTP 200, and no concurrent
compile request. Its capabilities GET completed in 0.000278 seconds. Exact
request events and body are retained in `tos-nav-language-canary-20260908`;
receipt SHA-256:
`c8da05ea31dfbe991d7dd9210a697244a6edfda3e2bed8eba939c0d3f389556f`.
That different source snapshot and separate browser run are not an A/B
baseline for the validator measurements. They narrow the observed cold path
without proving the earlier timeout's cause. The same canary verifies only
the three supplied Nani RU/EN navigation descriptors and independent pinned
reading, not all languages, materials or interaction budgets.

## Reproduction, validation and review boundary

The task-local artifact directory `tos-cold-start-profile-20260908` retains
the initial profile, all six cold-read receipts, `measure.py`, `measure-ui.py`,
`measure-validator.py`, and `validator-comparison.json`. Every receipt binds
the exact five input files, before/after fixity and implementation bytes.
The measured candidate `knowledge.py` SHA-256 is
`9e01d591d0567acc0909b6218e69557506bd8b670d4e7f4c869563eb0eab20bb`;
the validator comparison receipt SHA-256 is
`dad482769d0542dd01ef39785327e08fef42d679d6075c5e1d4eb2890ccd0b57`.
Use exact baseline input bytes from that receipt, not a later corpus with the
same filename. A fresh reproduction needs an admitted roughly 1.8 GiB
incremental memory envelope; it must not alter serving processes or caches.

Permanent reproduction routes:

```bash
python -m unittest discover -s access/tests -p test_knowledge_contract.py -k property_applicability
python scripts/validation_lanes.py --run standalone_access
```

The two focused tests and all six same-graph comparisons passed. An initial
full standalone run executed 159 tests and found one missed test migration
from the preceding Sign change: the existing native-annotation route still
expected `tos.entity.sign`. The owner registry and adapter already preserve
that data as `tos.entity.annotation-sign`, not as an issued source Sign. The
test now requires the declared adapter, preserved proposed status and original
IDs/payloads, and absence of invented promotion basis or source Sign. Its
Claim/evidence/source traversal assertions remain intact. No production code
was changed in response to that failure. The complete rerun passed all 159
tests in 181.704 seconds and source-profile validation; the lane took 328.336
seconds, with 1.7 GiB peak memory and no swap. Source-home, all 56 nested cards
and 16 test/script-topology tests passed. Generated companions are rebuilt from
this final note and verified through their owner routes before checkpoint;
their exact outcomes accompany the commit review.

Manual review of the source delta, mutable registry boundary, incremental
processor fingerprint and exact output comparison found no weakening of
source traceability, identity, language, uncertainty or authority. Review
gaps remain gaps; no assessment, Sign, canon or publication decision was made.
No new architectural decision is needed: the existing validator contract is
unchanged. Rollback restores its previous applicability loop; it does not
remove source records or roll back knowledge history. An opt-in internal
processor cache may recompute once after either implementation change.

No full repository release gate, CI, merge, deployment or published-runtime
claim is made. The separate source-foundation gate remains affected in the
working checkout by an existing ignored private local-content file; it was
not read, moved or deleted for this optimization. A clean-checkout gate and
the remaining Foundation requirements must be verified independently.
