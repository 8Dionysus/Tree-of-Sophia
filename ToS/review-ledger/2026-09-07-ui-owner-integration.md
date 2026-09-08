# UI owner integration — 2026-09-07

## Exact source and boundary

Combined the reviewed UI-owner tip
`e171d2ce562954372d632fa7e11afa8f1e432d2d` with backend parent
`c4c1553cc77beb53e2a9a93bb1c33d7670cc7161` in the Foundation v1 branch.
The four incoming commits are `c27b1546e`, `6440248cf`, `efda70f45` and
`e171d2ce5`: the live lens constructor, organized vocabulary, reading/place
continuity and soft inclusion glow. The UI-owner worktree was clean.

All `access/web` source and rebuilt assets match that owner tip exactly.
There is no new aesthetic or gesture decision here. The only merge conflict
was generated `documentation-family.current.json`, rebuilt through its owner
builder from the combined sources rather than choosing an old projection.
The peer worktree and its running preview were not changed.

Source/derived and owner separation **yes**: UI conditions compile the same
read-only LensSpec; saved places re-read current knowledge and do not store a
server cursor or acquire command authority. Typed overview exclusions remain
backend-owned. Inclusion glow explains query selection, not truth, acceptance
or significance. New rights, consent, canon and publication judgments **not
applicable**. The reviewed peer implementation is not proof of joint runtime
behavior; the checks below address that narrower integration claim.

## Combined checks

- `npm run typecheck`, `npm test`, `npm run build` in `access/web`: passed;
  84 tests across 12 files, test duration 0.754 s, build 2.90 s.
- Cross-corpus documentation currentness and guards: passed after rebuilding
  the merged generated companion.
- `python access/packaging/verify_ui_backend.py --root
  /srv/AbyssOS/Tree-of-Sophia --web-root
  /srv/AbyssOS/Tree-of-Sophia/access/web/dist --client-module
  /srv/AbyssOS/Tree-of-Sophia/access/web/src/observatory/knowledge-client.mjs`:
  passed with the real client and local HTTP server. It covered search, focus,
  exact node/relation inspection, relation reopening, four exploration pages,
  contested evidence, a direct path and explicit path exclusion.
- Client SHA-256:
  `ebe748b62db61b049c424e9533eabc5e0ad23030f47ef020084dff9601ed179e`.
  Observed data revision:
  `e05dba60221d11287d4fd88e72ab0b99ef64fe8102dbb44adec189e62d4b47cf`.
  Initial client focus: 29 carriers / 29 relations; relation reopening: 2 / 1;
  next focus: 40 / 64. This is source-snapshot-bound evidence, not a promise
  that subsequent documentation/index updates keep the same revision.

One local, non-isolated timing run measured cold catalog 22.051 s, warm catalog
0.000292 s, first search 4.017 s, repeated search 0.143 s, focus 0.190 s and
exploration continuation 0.000878 s. The catalog payload was 2,346,933 bytes.
These are observations, not p95 budgets or a complete scaling evaluation;
CPU/RSS, browser drawing and physical gesture feel were not measured here.

## Remaining work and recovery

The current UI still consumes opaque carrier IDs. One subject represented in
two source layers can appear twice, and the carrier bridge consumes a graph
hop. The next access/UI seam must distinguish a source identity, its carriers
and a scene vertex without dropping exact inspection or inventing equivalence.
Compact Claim paths and complete source text reading remain separate required
work. This merge does not close Foundation v1 or substitute frontend tests for
browser acceptance, CI, merge-to-main or remote deployment.

The two parent commits preserve both source histories. A reviewed integration
revert can restore the earlier consumer without deleting knowledge, assessment
history or the UI owner's branch. No remote publication was performed.
