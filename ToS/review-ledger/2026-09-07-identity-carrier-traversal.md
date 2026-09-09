# Zero-distance identity-carrier traversal, 2026-09-07

## Scope and boundary review

Partial Foundation F01/Q01/U01 work in `access/`, based on
`e17a0c3a3b62e5841d76d347c61dea133a0b7012`. The overview lens and resumable
exploration expand source-filtered carriers with an identical declared
`tos.*` entity ID before a positive-distance relation step. `all` preserves
carrier BFS. Explicit path-condition joins still follow exact relation steps.

- Yes: exact records, source refs, revisions, Claim identities, epistemic
  fields, wording and relationships remain unchanged. Identity expansion
  produces an inclusion reason, not a fabricated edge or accepted assertion.
- Yes: no grouping by names, fallback IDs, similarity, `same_as`, shared
  record makers or provenance. Filtering applies before expansion.
- Yes: page, work and session budgets apply to carriers, not unique scene
  vertices. Identity expansion is checkpointed; each group expands once.
  A previously queued carrier discovered at a shorter depth is moved forward
  and reported as a bounded context update, without repeating primary discovery.
- Yes: clipped bounded lenses explicitly report `identity_expansion_limited`.
  Zero-depth queries do not expand identities. No source admission, rights,
  language, canon, publication or agent authority is inferred from reachability.
- Not applicable: new authored historical assertions, translations, assessment
  decisions, identity merge/split operations or changes to the UI owner.

Python and Worker implementations, the result/API contracts and access README
own this behavior. D1 uses the indexed `(entity_id, id)` seek added to the
existing idempotent exploration migration; capability discovery fails closed
without it. No remote migration or deployment was performed.

## Verification

- Python knowledge suite: 55 tests passed, 20.761 seconds; exploration:
  13 passed, 1.746 seconds. Tests first exposed missing identity reachability,
  a shorter-path queue error and repeated group-expansion work; all corrected.
- An independent 0/1 shortest-path oracle covers five synthetic cyclic
  topologies, three directions, three depths, page sizes 1/3, unique discovery,
  exact edge membership and per-page bounds. These fixtures are not ToS facts.
- Worker typecheck passed; 17 knowledge/exploration tests passed in 42.837
  seconds. Python, pure TypeScript and indexed D1 agree on lens results;
  D1 continuation preserves discovery/emission order across different pages.
  Indexed identity query plans, source filtering and unavailable-index behavior
  are checked. Real Miniflare HTTP tests retain restart, eight concurrent
  replays, expiration, crossed publication and rejection of v1–v3 checkpoints.
- One-identity fixtures at 16/32/64 carriers in Python and 64 in D1 retain
  total metered traversal work within `4N + 1`; this is not a whole-query CPU,
  serialization, storage or global scaling bound.
- The unchanged Observatory KnowledgeClient passed against local HTTP:
  focus 40/40, exact relation 2/1, next area 40/64, search 6/6, exploration
  pages 1–4, available evidence, direct path present and excluded path absent.
  Client digest `ebe748b62db61b049c424e9533eabc5e0ad23030f47ef020084dff9601ed179e`.
  The measured source revision preceded this review/documentation refresh.

## Real data and measurement limits

Constantin Georg Naumann → Nietzsche letter 705 now succeeds at overview
depth 2: 5 carriers / 4 scene vertices / 4 relations, 75,089 bytes and 0.215
seconds. The reverse direction succeeds with 12 / 11 / 15, 252,032 bytes and
0.968 seconds. One-node-page continuation completes these neighborhoods in
5 pages / 20 work units and 16 pages / 61 work units respectively.

Cold graph creation: 16.328 seconds. Whole-process peak RSS: 1,203,028 KiB;
user/system CPU: 17.255 / 0.603 seconds, including cold creation and queries.
The separate real-client HTTP sample measured cold/warm catalog
20.901 / 0.000285 seconds, first/repeated search 3.712 / 0.125, alternative
search 0.200, focus 0.218, node/relation inspection 1.084 / 0.000154,
exploration/continuation 0.324 / 0.001128. Focus was 972,621 bytes: compact
delivery is still significant remaining work. These concurrent local samples
are not isolated benchmarks, p95, remote timings or physical UI acceptance.

## Compatibility, rollback and remaining work

Execution v4 fingerprints/checkpoints reject old continuation state; schema
readability of historical result packets remains. Reader rollback to the base
requires fresh cursors and removes no ToS sources or decision history. The
additional index is idempotent and does not alter stored source records.

Compact Claim paths, UI scene-map adoption and real gesture/frame acceptance
remain with the backend and separate UI owners respectively. Source semantic
admission, hosted CI, merge to main, deployment and live D1 parity are not
established by this slice. The broader Foundation goal remains incomplete.
