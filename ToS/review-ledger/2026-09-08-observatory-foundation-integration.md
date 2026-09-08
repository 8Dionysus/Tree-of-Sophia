# Observatory / Foundation integration review — 2026-09-08

## Source and ownership

This local integration joins Foundation parent
`d8211b9b9ca57cb67b56cf342a4c8f7595e709ae` with the UI owner's thirteen
commits ending at `231183c09b172368b8b5bc7e16c83819da3e750b`.
The UI branch owns its scene, navigation, contextual controls, reading shelf,
language selection and human-form presentation. The integration does not
redesign those surfaces or change their source files. Access remains read-only;
source assessment, growth commands, admission and canon retain their owners.

The only merge conflict was the generated documentation currentness file.
It was resolved by the owner builder, not by selecting a stale parent copy.
The standalone browser distribution is rebuilt from the merged UI sources;
the distribution carried in the branch was behind those sources. The replaced
generated JavaScript chunks are recoverable from the two Git parents.

## Consumer contract review

Review covered `knowledge-client`, human-form selection and rendering,
Claim reading, inspector and pinned-reader snapshot handling against the
Foundation access contracts. The inspector resolves a Claim with its exact
member/context closure from one current response; content revisions prevent
mixing old wording with a new path. Forms retain their mandatory context and
source-snapshot admission. Pinned reading preserves that context and reports
snapshot mismatch instead of interpreting a stored packet as a current grant.
Missing forms remain missing; navigation descriptions are not source wording.

The independent static review found one access schema defect: a bounded
response with both principal Claim legs but an omitted value-member edge
correctly retained the Claim as `incomplete-value-member-context`, yet the
advertised schema enum rejected this reason. The schema now admits that exact
existing producer value. The extended scene contract test executes both full
and compact LensResults with and without the member edge, validates both
complete response schemas, and requires the incomplete Claim to remain
explicit. No semantic validation or context requirement was relaxed.

The first test run exposed incomplete assertion-context data in this existing
scene-only synthetic fixture. The new whole-response assertions now use the
existing assertion-context producer for that fixture; the rerun passed in
0.105 seconds. This is test-data repair, not a production bypass.

## Observed verification and boundaries

- On the merged UI sources: 189 frontend tests in 26 files, TypeScript checking,
  and the Vite production build passed. The generated main chunk is 911.21 kB
  (256.58 kB gzip); the existing 900 kB warning remains visible. No threshold
  or timeout was raised.
- The preceding Foundation parent passed all 159 standalone access tests and
  the source-profile validator. Its Python production code is unchanged by
  this merge. The schema correction has the additional focused test above.
- The UI owner's actual browser/HTTP canary used backend `e844ef3da` and UI
  `231183c09`, not this integrated source snapshot. Its receipt
  `tos-nav-language-canary-20260908/receipt.json` has SHA-256
  `c8da05ea31dfbe991d7dd9210a697244a6edfda3e2bed8eba939c0d3f389556f`.
  Three Nani descriptors passed RU → EN → RU for scene label, hover, primary
  inspector heading and independently localized pinned reading. On the
  measured language transition, node IDs, selection and transforms remained
  stable. Six compact/full HTTP pairs preserved their source Claim and
  provenance without inventing ready human forms.
- That canary's natural cold compile took 40.255911 seconds with HTTP 200.
  It does not explain the earlier timeout, establish a latency budget or
  constitute an integrated-snapshot performance comparison.

Generated corpus and documentation companions are rebuilt and checked from
this final review before checkpoint. Source-home and nested route checks
remain part of the integration validation. Exact results accompany checkpoint
review; the note itself is not their substitute.

No whole-repository release gate, remote CI, merge to main, deployment,
publication, actual semantic competence or Foundation completion is claimed.
The complete source-foundation lane in the working checkout is still affected
by an existing ignored private local-content file; this integration neither
reads nor moves nor deletes it. The integrated-snapshot browser/HTTP check and
the remaining profiles, historical Sign-version route, actual assessment and
scaling requirements remain explicit work, not inferred acceptance.

## Review outcome and continuation

Source/derived separation, stable identity, language authority, mandatory
context and read-only access boundaries survive this local integration.
The schema finding is repaired, with positive and negative transport evidence.
No new doctrine or decision record is required for joining already-owned UI
work and correcting its advertised transport contract.
Rollback of the reader uses the previous UI/access revision and its generated
bundle; it must not remove source records or knowledge history. The UI owner
retains visual responsibility. Foundation retains joint consumer verification
and the rest of the full goal.
