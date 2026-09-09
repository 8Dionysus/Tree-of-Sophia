# Packet-local scene identity, 2026-09-07

## Scope and owner

`access/` owns the additive `tos_knowledge_scene_v1` delivery map in lens
results and exploration pages. ToS-authored identity, exact normalized
carriers, assertions, review and source forms are unchanged. This is partial
Foundation F01/U01 work, not completion of identity-aware research or the UI.
Base: `8daa566ce89c6413a04d8d2afec3caf56b0d25eb`.

The contract and implementation are in `access/contracts/knowledge-graph.v1.schema.json`
(`$defs.scene`), the lens/exploration result schemas, `knowledge.py` and the
Worker `knowledge.ts`; Python and D1 exploration reuse those implementations.
`access/README.md` states consumer semantics and compatibility.

## Manual boundary review

- Yes: only identical declared persistent `tos.*` entity IDs group returned
  carriers. Same labels, unqualified fallback IDs and even a `same_as` edge
  do not provide identity for grouping. Claim and subject/object IDs remain
  distinct; no assertion is accepted or strengthened by this operation.
- Yes: every exact node belongs to one scene vertex. The representative is a
  deterministic default carrier, not an adjudication of competing records.
  Original wording, context, epistemic fields, source refs and revisions stay
  on the unchanged carrier records. Scene IDs are not inspection/query IDs.
- Yes: every returned relation is either an arc referencing the exact
  relation ID, or an explicitly collapsed typed projection self-link.
  Other self-relations and competing/parallel relations remain separate.
- Yes: mapping occurs after pagination and source filtering. No omitted,
  restricted or off-page carrier is fetched to fill a vertex. This is not a
  new corpus registry, semantic projection, source mutation or model call.
- Not applicable: new assessment, canon, publication, translation or rights
  decisions; no such authority changed. No new durable ontology decision is
  introduced: the implementation applies the existing subject/record split.

## Verification

- Python contract test first failed with missing `scene`, then passed.
  It covers shared IDs, namesakes, Claim identity, exact carrier preservation,
  self-links, typed projection collapse, input-order independence, source
  filters, delivery pages and unqualified fallback negatives.
- Knowledge contract suite: 53 tests passed, 22.355 seconds. Exploration:
  11 passed, 1.179 seconds, including real-core schema/native MCP continuation.
  An initially unresolved scene-schema reference in exploration was corrected
  by locating the shared definition in the already consumed graph schema.
- Worker typecheck passed. Knowledge/exploration suites: 15 passed, 30.005
  seconds. Python, pure Worker and indexed D1 agree on shared-carrier maps
  and delivery pages. Exploration retains cycle/direction/size coverage,
  restart, concurrent replay, expiration and crossed-publication checks.
- Actual unchanged Observatory KnowledgeClient against local HTTP passed:
  focus 29/29, exact relation 2/1, next area 40/64, search 6/6, exploration
  pages 1–4, evidence available, direct path present and excluded path absent.
  Client digest `ebe748b62db61b049c424e9533eabc5e0ad23030f47ef020084dff9601ed179e`.
  This proves additive compatibility, not rendering or UI adoption of the map.

## Real data and cost boundary

One local sample over 39,764 carriers / 59,198 relations:

| Focus, overview depth 2 | Carriers / vertices | Relations / arcs | Scene bytes | Query seconds |
| --- | --- | --- | --- | --- |
| Constantin Georg Naumann | 3 / 2 | 2 / 1 | 1,243 | 0.251 |
| Nietzsche letter 705 | 6 / 5 | 5 / 4 | 2,914 | 0.245 |
| Letter addressee Claim | 9 / 7 | 11 / 9 | 5,135 | 0.246 |

Cold graph creation was 17.409 seconds; process peak RSS 1,202,960 KiB,
user CPU 17.431 seconds and system CPU 0.645 seconds include graph creation
and the three queries. Tests/other work ran concurrently. These are samples,
not p95 or an isolated map benchmark. The unchanged real-client HTTP run
measured cold/warm catalog 23.699 / 0.000185 seconds, first/repeated search
3.993 / 0.137, focus 0.217, node/relation inspection 1.126 / 0.000175,
exploration/continuation 0.331 / 0.000809 seconds. No remote timing claim.

## Remaining owner work and rollback

Neighborhood traversal still counts technical carrier hops; focusing on the
person does not yet reveal the letter at the desired semantic depth. Compact
Claim paths and scene changes remain required. The current UI still renders
exact carrier nodes and has not consumed `scene`; scene selection, saved
places, exact inspector and gesture preservation require the separate UI
owner's consumer review. No UI source, peer worktree, browser or deployment
was changed in this slice. No new UI task was formed or resumed.

The new field is optional for historical v1 packets. Legacy consumers retain
carrier rendering. Lens execution v3 fingerprints and exploration v3
checkpoints reject replay across the changed delivery implementation. A
derived-reader rollback to the base commit does not remove any new ToS
source or decision history; it requires restarting incompatible cursors.
CI, merge to main, public deployment and physical interaction acceptance are
not established by these local checks.
