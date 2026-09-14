# Owner-local source profiles and retained transactions

Date: 2026-09-08. Reviewer: source-owner agent within the continuing Foundation
v1 operator mandate. This review concerns source mechanics and confidentiality,
not historical or linguistic acceptance, public release or canon.

## Owner surfaces and inspection

The explicit `tos_local_owner_profile_command_v1` route in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md` owns source creation,
descriptive revision and forms in the already accepted confidential context.
`scripts/source_owner_record_profiles.py` composes the existing public grammar
reader; its shared shape check remains in `scripts/source_record_profiles.py`.
The new command adapter is
`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_profile_commands.py`.

No private schema, catalog, identity registry, history grammar or source ignore
was introduced. The existing `semantic-metadata-v1` profiles, source/create
receipt, record-revision archive and human-form grammar remain authoritative.
The selected private root holds every package companion, including retained
configuration, requests, serialization provenance, form history and archives.
Source/contract bytes remain with the public checkout owner. Native metadata
and private representation are read only under their distinct exact scope;
current local-derivation rights are checked before content reads.

Root inspection covered the shared profile extraction, private facade and
all its tests, command and provenance changes, public catalog guard, public
form visibility guard, private package/lock/atomic exchange, archive reconstruction,
identity discovery and exact retry paths. An independent bounded helper
implemented/reviewed the reader and then inspected the command boundary.

The helper reproduced three real defects before this review was finalized:

1. The combined config's record-operation names could be submitted inside
   `apply`, where the old common form helper treated a non-create name as a
   revision. The private boundary now admits only `form.create`/`form.revise`,
   separately from checking delegated scope.
2. Internal form-history consistency alone did not prove that replay results
   matched the exact command. Revision-produced form refs must now exist in
   retained history and are reconstructed from the exact archived predecessor
   and retained request. Form replay compares result refs to its supplied
   request, including source, package, actor and authority bindings.
3. Replay could return stale current forms after the last dependency read.
   Both creation and update replay now repeat current configuration/context,
   rights/native closure and exact package checks before returning.

Root added regressions for these reproductions and for private native packet
creation followed by Occurrence creation, forms and descriptive revision.
Independent helper output is review input, not acceptance by itself.

## Verification

The inputs in every new executable test are synthetic. None reads retained
private source text, establishes real rights, creates a durable private corpus
or constitutes an assessment/competence record.

- Private reader: 18 tests passed, including metadata-only validation after
  earlier explicit exact verification, separate full-snapshot rejection of
  changed bytes, complete native binding, unknown fields, protected modes,
  schema shadow/alias refusal and no public catalog API.
- Private command final complete run: 26 tests passed in 207.529 s, including
  the independent review regressions and the native-to-profile integration.
- New private native writer → Occurrence → forms → revision integration:
  passed separately in 12.843 s; the packet, proposed status and source bytes
  remain unchanged and no admission is created.
- Original public source commands: 39 passed in 300.675 s.
- Original bibliographic graph: 81 passed in 231.384 s; Occurrence growth:
  8 passed in 10.633 s.
- Human-form core: 22 passed in 0.191 s; native writer: 22 passed in 17.421 s;
  Occurrence assessment guards: 5 passed in 46.577 s.
- Script and test topology: 16 passed in 1.773 s. Source-foundation, source-home,
  public catalog and bibliographic graph parity/validation passed. Documentation
  tests: 35 passed in 1.921 s; nested route-card checks: 56 passed. Corpus and
  documentation companions were rebuilt and checked after the final review.

Reproduce the changed contract with:

```bash
python -m unittest tests.test_source_owner_record_profiles -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_owner_profile_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_commands.py -v
python -m unittest tests.test_script_topology tests.test_test_topology -v
```

## Manual boundary checklist and limitations

Source traceability, unchanged native identity, exact previous-byte retention,
shared machine/human model, explicit authority, private/public separation and
uncertainty preservation: yes within the inspected scope. Source/proposal,
mechanical readiness, semantic assessment, scoped use and public admission
remain distinct. No reviewer record was relabeled or human judgment fabricated.
Canon, lived witness, translation quality, public example and broader AoA
runtime/proof/memory ownership changes: not applicable to this slice.

Metadata copying is not content assessment. Only Occurrence has an understood
native-binding profile here; other existing semantic profiles require no
fabricated binding and metadata-only access. Private Claims and source-bound
assessment selection remain unimplemented next consumers. No actual private
store, real closed-source Occurrence, UI consumer or public projection was
created by this implementation or its tests.

The writer still performs a bounded current identity inventory across the
public and selected private metadata homes. It does not discover other private
stores, prove global uniqueness there, provide constant-cost indexed writes,
make a cross-subject transaction or isolate hostile same-account editors.
The account/issuer must preserve stable inputs during each command and own
durable store preservation. Archives are source history, not cleanup candidates.
No full Foundation completion, CI, merge, deployment or published runtime is
claimed. The next owner is this same source-growth route followed by the
explicit private assessment/Claim consumer integration, not public export.
