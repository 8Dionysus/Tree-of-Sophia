# Declared-profile source correction, 2026-09-07

## Scope and manual review

Partial Foundation C01/F01/F08/V02 work, based on
`1c3b514febf466acf8d205710e834fbb3d03d982`. The existing Growth source command
and source-revision transaction now accept a separately delegated
`tos_local_profile_revision_owner_v1` configuration. `profile_type_id` names
the exact current declared metadata profile. The operation remains
`record.revise`; the historical-only configuration retains its old scope.

- Yes: the registry and declared local schemas own type, basename, identity
  prefix and understood record versions. The writer reuses `SourceRecordProfiles`,
  including its shared metadata floor; no per-kind writer branch is added.
- Yes: the same bounded source transaction retains exact old package bytes,
  increments only the record version, explicitly replaces every current form,
  and preserves unknown fields and unselected companion bytes.
- Yes: source, forms, claims, provenance, assessment, rights and publication
  remain separate. Only delegated descriptive fields may change. Type, ID,
  schema version, identity status, supersession, visibility and decisions are
  not editable through this operation. A form or creation grant is insufficient.
- Yes: current registry, registry schema, consumed source schemas/dependencies,
  and reader/command/form code participate in preparation dependencies. Drift
  prevents an uncommitted request from publishing. A committed retry retains
  its historical receipt and checks current scope rather than recreating it.
- Not applicable: new historical interpretation, agent competence or admission,
  canon, rights, UI design, deployment, external actor or runtime policy.

The authoritative procedure and usage are in the existing
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`, under
“Versioned source correction”. The semantic-interchange README documents the
reader/writer boundary; no schema or source record is rewritten for this change.

## Verification

The Letter contract tests first failed because the new configuration was not
supported, then passed through the shared transaction. They exercise exact
archive/inspection, unknown companions, form history, CLI replay, concurrency,
scope revocation, stale inputs, bounded packages, symlink/nested-file refusal,
and real process loss before/after atomic directory exchange. Additional
negatives reject wrong profile authority, registry/schema drift, unsupported
record versions, and prohibited identity/visibility/schema transitions.

The source-command integration test introduces a synthetic `fixture-message`
profile using only schema/registry data. Creation, form revision, source-record
revision and the ordinary catalog/graph/knowledge reader retain the same ID
and complete source data. Forms remain ready with no admission; retrying the
original creation returns its unchanged receipt after the later revisions.
This synthetic object is not historical evidence.

Reproduce with:

```bash
python -m unittest discover -s mechanics/growth-cycle/tests
```

The final complete suite passed 141 tests in 60.875 seconds. The focused
revision suite passed 25 tests in 13.756 seconds; source commands passed 35
tests in 17.233 seconds. Documentation currentness, 56 nested route cards,
16 task routes and source-home validation passed. Timings are
concurrent local samples, not p95 or hosted performance.

## Real-source read-only sample

`prepare-revise` was exercised on
`tos.letter.nietzsche-naumann-1886-705`, version 1, digest
`sha256:848c8221bfa02a9f2cea9729c4ea28765c828e18c444d399d48ea8b7856922b5`.
The patch deliberately repeated the existing notes without interpreting or
correcting them, and selected all three existing source-copy forms by their
exact wording pointers. Preparation took 0.305 seconds and returned ready
Russian, Russian and German forms with unchanged wording and no admission.
All six current package files remained byte-identical. The proposed version 2
was **not applied**. This proves current-source preparation, not a new accepted
historical fact or a canonical no-op revision.

## Limits and rollback

Only declared `corpus-metadata-v1` profiles use the new configuration. Native
non-profile record correction, Claim revision, multi-subject transactions,
identity transitions and substantive assessment remain separate work. The
existing flat-package byte/file/history bounds and Linux atomic-exchange
constraint remain. Snapshot publication and incremental indexing are not
performed by a source-write command; readers must refresh through their owners.

Rolling back this writer does not erase source history. The old historical
configuration remains usable; an old writer refuses the new configuration.
Existing readers still consume unchanged source schema formats and retained
archives. Local tests and read-only real-source preparation do not prove hosted
CI, merge, deployment, UI acceptance or completion of Foundation v1.
