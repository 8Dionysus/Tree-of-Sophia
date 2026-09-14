# Source-owned Claim navigation review

Date: 2026-09-08. Scope: the navigation-template change following
`31d4643139cd5fb4e3e71eed63f2094c19913f38`, not Foundation v1 acceptance.
Reviewer: primary implementation agent under the Operator's Foundation goal;
bounded helpers checked exporter behavior and independent Worker/D1 transport.

## Owner and boundary review

- **Yes — source return and layer separation.** `HUMAN_FORMS.md`, relation
  registry template/version and schema own the syntax. The exporter binds the
  full raw Claim, exact predicate entry/mapping and both endpoint records.
  The descriptor is separate from the original Claim and any HumanForm.
- **Yes — uncertainty and mandatory context.** The title explicitly describes
  a Claim record and its declared initial statuses. It is a field catalogue,
  not a fluent assertion. The whole Claim retains its negation, qualifications,
  alternatives, evidence and unknown fields. Navigation alone cannot supply
  an available wording pointer or standalone compact reading.
- **Yes — identity and language.** No ID parsing, personal-name expansion,
  truncation, language-split identity or inferred source language is used.
  Russian/English registry syntax leaves original endpoint names unchanged.
  The three real Nani routes retain the source's `С. П. Нани`, not a conjectured
  full name. An unavailable language or input is refused rather than guessed.
- **Yes — lower-authority carrier controls.** Access validates closed descriptor
  fields, exact source/template bindings and duplicate projected Claim fields
  before normalization. Native artifact labels use their existing fixed pointer;
  a carrier cannot point into arbitrary source narrative to create a name.
  Duplicate registry mappings remain invalid even within one entry.
- **Yes — independent owner boundaries.** No historical source, assessment,
  competence grant, private payload, consent, rights, canon, publication or
  runtime installation changed. UI design and the user's live server remain
  with their owners. KAG, memo, proof, roles and host deployment are not effects.
- **Not applicable.** Canon/example synchronization, lived witness, counterpart,
  compost, calibration and branch-lineage changes are not part of this slice.

The current export has 353 Claim descriptors: 303 available navigation titles,
45 unavailable identity-only renderings because the object is a value, and
5 unrecognized predicate mappings. These counts measure navigation coverage,
not textual quality, semantic acceptance or completed legacy HumanForms.

## Verification

- Source graph module: 91 tests passed in 228.849 seconds; peak 252 MiB, swap 0.
  The six new tests cover exact names/records, finite syntax, status combinations,
  native adapters, typed endpoint closure, refusal ordering and byte limits.
- Full `standalone_access` lane: 157 tests passed in 144.619 seconds and the
  standalone source profile passed. Whole lane: 242.930 seconds; peak 1.7 GiB,
  swap 0. Its optional unconfigured AbyssOS adapter is not a required failure.
- Worker knowledge module: 13 tests passed in 31.991 seconds; peak 465.3 MiB,
  swap 0. Three actual legacy Claim records agree across Python, Worker and D1
  for RU/EN and full/compact reads; repeated D1 reads retain the snapshot.
  Worker typecheck also passed. No Worker implementation change was required.
- Source foundation, source graph, corpus index, source-home, decision records
  and generated decision indexes, documentation currentness/cross-corpus guards,
  AGENTS currentness and 56 nested route cards passed before this review note.
  Its source-index/documentation companions require the final owner rebuild.
- A negative test exposed divergence of copied carrier status/predicate/object/
  qualifiers from the full raw Claim; the added equality guard rejects it.
  Initial new-test failures also corrected the test's assumption about foldable
  full incident graphs and the exact `wording_state` field. They were not
  hidden by weakening the existing compact-path behavior.
- Cold/warm normalization equality and a source-name/version edit verify
  dependency invalidation, changed Claim revisions and current relation display
  without repeating extraction or manufacturing prose.

## Limits and next owner

The earlier exact `31d4643` backend / `14acc9c05` UI HTTP canary establishes
Motif and public Claim reading compatibility, not this new descriptor's rendered
browser behavior. This slice still needs its exact commit-bound checkpoint
review and UI-owner canary. No remote CI, merge, release or deployment is claimed.

Legacy statement/forms migration, literal/structured-value navigation and broader
Foundation content remain source-owner work. Sign additionally requires the
actual evidence-bearing promotion boundary, not just another entity type name.
