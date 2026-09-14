# Exact Claim version source-return review

Date: 2026-09-08. Base: `cc43be2874d65a63b24a8053e4089ea4ed05e7b6`.
Scope: read-only public Claim history, Sign basis traversal and portable exact
version delivery. This is not real Sign issuance or Foundation v1 acceptance.
Reviewer: primary implementation agent under the Operator's Foundation goal;
a bounded helper authored the pure reader/fixtures and separately challenged
the source/access boundary.

## Owner and boundary judgment

- **Yes — source versus projection.** The source-owned version-view schema
  and entity/relation registries define a separate RecordVersion. The retained
  Claim body, canonical record digest, original stream-byte digest, package
  history and current catalog binding remain distinct. Stable address and
  byte integrity do not establish textual or philosophical truth.
- **Yes — exact identity and lineage.** A Sign's immutable candidate reference
  targets that exact record version, never the current Claim with the same ID.
  Multiple Signs can share one version address. The structural relation has a
  concrete Sign/RecordVersion domain/range and independently compares the
  exact references; it is neither a fact edge nor an identity-equivalence act.
- **Yes — visibility and history.** The pure reader verifies current public
  catalog/source bindings and the complete retained shared correction chain,
  including interleaved sibling revisions. It exposes only the selected Claim,
  not neighboring prose. Protected paths, current restrictions, corrupted or
  missing archives, unsupported source families, changed snapshots and declared
  budgets yield observable refusals. No latest-version fallback is available.
- **Yes — human and machine context.** Full inspection carries the exact body
  and provenance. Compact delivery keeps the reference, availability and whole
  assertion context. Source-authored wording is quoted in its own language;
  navigation titles do not manufacture authored names or assessed HumanForms.
  Unknown fields, nulls, false values and empty collections remain distinguishable.
- **Yes — authority ceiling.** Old assessment/status fields are inspectable as
  historical source context, not current grants. The read view performs no
  assessment or source write. No owner grant, private input, source Claim,
  historical review, canon, publication policy or UI artwork was changed.
- **Not applicable.** Lived witness, counterpart, compost, canon mirrors and
  actual philosophical Sign acceptance are outside this read-only slice.

Two genuine access defects were found and corrected during review. First,
generic carrier convenience fields could leak stale prose in full source data
and impersonate authority in compact epistemic fields. The closed source
envelope now rejects them; unknown original fields remain inside the verified
record. Second, typed endpoints alone permitted a mismatching candidate
version. Semantic validation now compares the exact Sign basis and target ref.
Independent probes verified both fixes and the corresponding valid cases.

## Verification

- Pure Claim-version reader: 14 tests passed, 0.982 s, 22.2 MiB, no swap.
  Tests include exact current and interleaved historical records, raw fixity,
  duplicates, corrupt/missing archives, request tampering, unrecorded sibling
  edits, later archive corruption, path/visibility/budget refusals, batching,
  concurrent drift, unchanged source bytes and no configured command invocation.
- Hardened access/whole-source cases: five focused tests passed, 4.264 s,
  109 MiB, no swap. The separate mismatching-basis test passed, 0.131 s,
  84.7 MiB. Source-view and closed navigation-envelope schema checks then
  passed in both end-to-end tests, 3.220 s, 44.9 MiB, no swap.
- End-to-end fixtures use the actual retained package/receipt format and
  declared source profile: v1 is archived, current Claim advances to v2,
  and two unissued synthetic Signs still traverse to the one v1 view.
  Full/compact source→catalog→navigation→access results retain exact context;
  missing/corrupt/private/version-shape cases retain a gap instead of v2.
  Synthetic fixtures are not a claim that those Signs were legitimately issued.
- Worker knowledge module: 13 tests passed, 29.037 s, 508.3 MiB, no swap.
  The existing RU/EN transport case now compares exact-version available/gap
  carriers through Python, pure TypeScript and local D1 in full and compact
  delivery. It covers transport, not source-archive verification or remote D1.
  Worker TypeScript checking also passed.
- Final complete access suite: 163 tests passed, 237.529 s; the subsequent
  standalone source profile passed. The complete lane took 432.869 s,
  peaked at 1.5 GiB and used no swap.
- Final complete source-witness bibliographic graph suite: 97 tests passed,
  462.795 s, 286.4 MiB peak and 340 KiB swap. Source and test hashes were
  unchanged from launch through terminal completion of both final suites.
- Catalog, bibliographic graph and corpus were rebuilt through their owners;
  all three direct validators passed. Script/test topology: 16 tests passed.
- Two earlier full runs were intentionally stopped after review found the
  defects, not reported as whole-lane success: the old access module completed
  162 tests, but its standalone packaging validation was interrupted; the
  source-graph module was interrupted before completion. The final runs above
  completed on the corrected source.

## Limits and next owner

The first reader handles declared public `source-claims.jsonl`, not arbitrary
native annotations, private packages, archived HumanForms or every record
family. Valid larger sources can be over budget; there is no truncation.
No real Sign has been issued by these tests. ToS still owns source-visible
assessment quality, competent issuance and other required profile adapters.

The separately completed exact-cc43 browser canary passed the three Nani Claim
routes and Stoa required HumanForm context, but failed compound Claim pinning:
the inspector's related closure was absent from the pinned reader. The UI owner
subsequently repaired it in separate commit
`aad1f4a401e86840e13dbf651f91ca2f9d561481`: 195 frontend tests and 34 actual
browser checks passed against the immutable cc43 source snapshot. That evidence
is not yet a jointly integrated exact-version reader canary. The old canary's
first cold compile took
14.966622096 s and returned HTTP 200; this single observation is not a controlled
benchmark. This version-view change does not claim UI acceptance.

The prior full source-foundation lane remains distinct from direct validation:
its transfer-source-passages check refused an existing ignored private file.
That file was not read, moved or deleted here. No remote CI, main merge,
release, deployment, semantic acceptance or full-foundation completion is claimed.
