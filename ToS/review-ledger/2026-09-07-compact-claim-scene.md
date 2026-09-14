# Compact Claim scene view, 2026-09-07

## Scope and manual review

Partial Foundation F01/H01/Q01/U01 work, based on
`f8821f97db2329fad3f92b76f07fd28f2f939403`. `access/` adds `scene.compact`
alongside the exact scene. It offers presentation paths for complete reified
Claims with mapped predicates and consistent typed subject/object legs.
Source ToS records, semantic admission and the separate UI owner are unchanged.

- Yes: one path names one exact Claim carrier, both original relation legs,
  subject/Claim/object IDs and the Claim's content revision. It is not a new
  normalized relation, predicate assertion, identity or accepted fact.
- Yes: competing Claims remain separate. Claim identity cannot collapse into
  an endpoint. Unmapped, ambiguous, incomplete, focused or mixed-carrier Claims
  remain explicit, with a reason. Unknown incident relations are not discarded.
- Yes: source wording and its mandatory context remain attached. The path
  points to an intact selected source form, otherwise available source
  summary/title. Missing wording is explicit, not filled with IDs or generic
  text. Reading requires Claim semantics/epistemic context and the context of
  every original path/detail relation; `standalone=false` is mandatory.
- Yes: claim-supported-by edges may fold into exact path details. Grounds
  retain their records and references. A focused/shared ground stays visible;
  an incomplete Claim used as grounds must not fold as an evidence-only vertex.
- Yes: full packet records and scene remain available. Compact vertex IDs plus
  folded IDs partition the original scene. Visible arcs, path legs/details and
  explicitly collapsed projection self-links account for every relation once.
- Not applicable: new rights, consent, publication, canon, translation or
  assessment decisions. This is deterministic presentation selection, not a
  model judgment or new historical inference.

The contract is in `access/contracts/knowledge-graph.v1.schema.json`,
`knowledge-api.v1.json` and the access README. Python `knowledge.py` and Worker
`knowledge.ts` implement the shared rule; both exploration backends now reuse
the existing source-bound display/form selection before constructing the map.

## Verification and limits

- New Python test failed on missing `compact`, then passed. Focused controls
  cover competing polarity, unknown qualifiers, exact expansion, missing page
  legs, extra incident edges, predicate gaps, duplicate legs, false identity,
  focused Claim/grounds and a schema rejection of standalone reading.
- A 16-topology property check varies Claims, shared grounds, cycles, missing
  and duplicate legs, self-relations, extra incident relations and focus. It
  checks endpoint closure, exact relation accounting, carrier preservation,
  explicit retained Claims and input-order independence. Synthetic topology
  is not evidence for any historical relationship.
- Python knowledge/exploration suites passed (69 tests, 21.272 seconds), then
  the additional accounting property passed separately. The first combined
  invocation lacked the fixture import path; the successful invocation uses
  `PYTHONPATH=access/tests`.
- Worker typecheck and 18 knowledge/exploration tests passed (44.330 seconds).
  Python, pure TypeScript and D1 agree on compact lenses and paginated results;
  D1 continuation checks qualified paths and missing-leg negatives. Adding
  shared display selection exposed an absent-attributes error in sparse test
  packets; the shared carrier function now handles that case consistently.
  Existing restart, concurrent replay, snapshot crossing, expiry and v1–v4
  checkpoint rejection checks remain green.
- The unchanged Observatory KnowledgeClient passed through real local HTTP:
  focus 40/40, exact relation 2/1, next area 40/64, search 6/6, exploration
  pages 1–4, evidence available, direct path present and excluded path absent.
  Client digest `ebe748b62db61b049c424e9533eabc5e0ad23030f47ef020084dff9601ed179e`.
  This verifies compatibility, not browser adoption, gestures or rendering.

## Real reading and cost sample

At overview depth 2, focusing on Constantin Georg Naumann returns four exact
scene vertices and a compact view with two vertices / one Claim path.
Focusing on Nietzsche letter 705 returns eleven exact scene vertices and five
compact vertices / four paths. Their source-copy statement forms are present,
each with one intact source context, `standalone_reading=false`, and unchanged
`unreviewed` status. The addressee, sender, relation to Jenseits and attributed
commission account retain their different qualifications. This does not
upgrade the underlying retained source-reading evidence.

Person/letter packets were 75,325 / 254,272 bytes, with scene maps of
4,425 / 15,297 bytes; query samples were 0.263 / 0.285 seconds. Cold graph
creation was 17.171 seconds; whole-process peak RSS 1,203,144 KiB and user/system
CPU 17.053 / 0.612 seconds include creation and both queries. Work ran alongside
other checks: these are local samples, not isolated benchmarks or p95.

The separate real-client run measured cold/warm catalog 21.463 / 0.000304
seconds; first/repeated/alternative search 3.750 / 0.126 / 0.200; focus 0.212;
node/relation inspection 1.115 / 0.000148; exploration/continuation
0.339 / 0.002710. Focus was 985,939 bytes and exploration pages 93,798 / 161,910
bytes. Added source-bound forms increase exploration payload; smaller summary
delivery remains a separate performance concern. These source revisions
preceded the review/documentation refresh. No hosted timing claim.

## Remaining work and rollback

The compact view is optional and packet-local. Query depth still counts the
ordinary relation legs through a Claim; this slice does not implement semantic
one-step traversal. A small continuation page with one leg cannot invent the
other from an earlier snapshot or imply a complete compact path. Shared scene
composition, UI adoption, compact captions where absent and physical gesture
acceptance remain required. No UI source/worktree or browser was changed.

Execution v5 fingerprints/checkpoints reject replay across the changed output.
Historical v1 result packets remain schema-readable without `compact`.
Rollback to the base reader requires fresh cursors and removes no ToS sources
or decision history. Hosted CI, merge to main, deployment and live D1 parity
are not established by these local checks. The full Foundation goal remains
incomplete.
