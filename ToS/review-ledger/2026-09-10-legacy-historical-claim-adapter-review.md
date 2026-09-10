# Captured historical Claim adapter review — 2026-09-10

## Scope and source authority

This review covers the separately authorized descriptive adapter, not changes
to the three real Jenseits commission Claims or any historical assessment.
The work starts from approved integration base
`036d47b018849e0e31b2ce26cbd60b696c5558a1`, with G5 candidate
`af45df629d06466d73dcea68494c7889a6bb1324` imported as
`5a682b88f76fc032b85cbd1403828c29d4cb929f`. Stable patch identity of that import
is `ee0a34ce95d2a8b2a7fdaaca7e1766066acbba24`.

The [Growth route](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#captured-legacy-historical-claims)
owns the two new grant schemas. [Human Forms](../doctrine/HUMAN_FORMS.md)
retains whole-Claim context and independent assessment. The
[historical Claim schema](../contracts/historical-claim.schema.json) and
original Claim IDs are unchanged; native SourceClaimProfiles are not coerced
into accepting the legacy carrier. No source-witness record, Claim stream,
creation receipt or actual form set is edited in this change.

## Reviewed boundaries

- Yes: explicit historical family grants bind the exact source path, Claim,
  historical record and original receipt digest. Revision requires explicit
  qualifier and form-field selections; old grants gain no adapter.
- Yes: only statement/language/script/display qualifiers may change. Existing
  structural qualifiers, endpoints, object, schema, evidence, maker, assertion
  layer and admission stay fixed. Every current form is rebound, with the
  complete statement and mandatory whole-Claim context.
- Yes: shared Claim locking, package CAS, predecessor archives and atomic
  exchange remain the transaction owner. Other rows retain their exact bytes.
  Event record history and Claim history can interleave without rewriting
  original request, receipt, environment or provenance bytes.
- Yes: captured historical.create v2 origin is required by either writer.
  Partial capture, foreign receipt and broken archive chains fail closed.
  Uncaptured legacy packages do not obtain a writer through this adapter.
- Yes: exact Claim readers budget their package, contracts and Claim archives.
  They explicitly do not certify separate Event record archives. The writer
  additionally verifies those archives before authorizing a correction.
- Yes: graph and assessed-form consumers recognize the exact legacy adjacent
  form path. No Event wording, predicate label or source classification becomes
  a fabricated Claim statement or an assessment.
- Yes: this adapter checks allocated IDs against public source-located current
  and retained forms. This is not a claim that all existing native writers
  already enforce the same global collision boundary.
- Not applicable: translation, canon, rights/publication, witness acquisition,
  philosophical planting, runtime deployment or source truth acceptance.

The parent independently inspected the new adapter and shared seams. This
note records mechanical and boundary review, not independent execution truth,
historical competence assessment, or a grant to mutate real Claims.

## Validation evidence

Each local XML result records its exact working tree, base HEAD, tracked diff
SHA-256, untracked implementation/test SHA-256 values and selected test names.
XML artifacts remain local test evidence, not source corpus inputs.

| Run | Result | Resource observation | XML SHA-256 |
| --- | --- | --- | --- |
| Legacy adapter and command discovery | 12 tests, 121 subtests passed; 19.89 s | 144.6 MiB peak, zero swap | `1563d20bd92702fae64ae338f658423ad5c79aa766ebfd38671828e2a014094c` |
| Exact-version reader fixtures | 17 tests, 31 subtests passed; 0.76 s | 30.3 MiB peak, zero swap | `f4dbbdd9e69779de27bf9dfc5b8ce211e6b2eeedadbb492d89a2a153df595c97` |
| Native/G5/Event/assessed compatibility and inventories | 21 passed, one pre-existing inventory failure; 22 tests, 813 subtests; 119.33 s | 145.8 MiB peak, zero swap | `1bd74a6dbfbef49a0a0d92938bb8f44e2a09bac6b2d7b6bbc0fa6b2c0d9e622d` |
| Final exact grant controls and repaired inventory target | 2 tests, 15 subtests passed; 1.42 s | 75.1 MiB peak, zero swap | `a34f41311974a7bb5d18afd297bb82eeab2d7e82cae7b3ab3ab69ca94116f96a` |

The first three runs bind tracked patch SHA-256
`93147989bef03dfe7d1baf1b58e96adc7005fe46fc0aebe2bda91db682122eeb`.
The final focused run binds
`6b98ced80a0b622db2d63a4f5bf70be84ad3bc078129231bcd79ca43f907a6c2`,
after the requested return-type correction, explicit required form-field grant,
non-object field refusal and exact missing inventory entry. The earlier 21
passing compatibility checks were not rerun for that inventory-only repair.

The missing entry was `access/tests/test_readable_context.py`, already present
but unlisted at the approved base. Its actual compiler/schema/cache/binding tests
were read before adding the owner-routed inventory companion. No access
production code changed for this repair.

G5 made retained correction forms mandatory. The exact-version reader's
synthetic fixture still emitted empty form selections; it now uses the actual
shared preparation/application helpers. Production history guards were not
weakened. Earlier interrupted/failed exploratory runs remain separate: an
imported unittest class expanded initial discovery, and two negative fixtures
needed an explicit synthetic date record. They are not counted as green runs.

## Limits and next owner

No full corpus/catalog/graph generation, complete test lane, CI, merge,
deployment, UI/runtime acceptance or actual historical assessment is claimed.
The official `source_home` lane passed as a separate structural closeout
check (30.9 MiB peak, zero swap); it establishes neither meaning nor admission.
The parent owns exact integration and any later reviewed historical wording,
new grants and actual source revisions. A separate bounded companion will add
finite historical recognition to existing native/Artifact form-ID checks;
that follow-up is not silently included in this adapter's acceptance.
