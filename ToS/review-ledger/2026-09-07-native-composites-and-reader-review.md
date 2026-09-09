# Existing scholarly composites: reader integration review

Reviewed on 2026-09-07 by `agent:codex-tos-foundation`, following
`4fea6c0ea383cfb7fb8f72f67b6aec22735eb297`.
This is exact-source mapping and reader evidence, not reconstruction assessment,
membership admission, independent corroboration or Foundation v1 completion.

## Scope and boundary

All ten existing `tos_scholarly_composite_witness_v1` records now map into the
source catalog and both ordinary knowledge carriers, without rewriting their
source bytes or assigning replacement IDs. The native adapter binds the exact
schema, `composite_id`, source identity status, label, editorial description,
record digest and authority. `tos.entity.composite` is an intellectual object,
not a physical artifact or the reconstructed ancient original.

Provider/member/coverage observations remain complete source metadata. They
do not generate new witness-membership Claims or turn a reported editorial
period into historical time. The existing source-planting links stay navigation.
Qualified human forms and source assessment are distinct: legacy descriptions
remain readable, but missing language declarations do not become ready Russian
hover forms. Adjacent Corpus-form files are refused for this native shape.
The current assessment source-binding adapter does not yet recognize native
composite identity; this change does not claim otherwise.

## Verification

The initial positive test failed because the catalog lacked the composite
family; a negative test demonstrated that nonpublic native composite input
was simply not considered by that reader. The implementation now rejects
nonpublic/unknown source shape, schema/identity/digest/catalog drift, duplicate
IDs, duplicate JSON keys, nonfinite numbers, unsafe or symlinked paths and
records above 1 MiB. Four focused tests passed in 4.945 seconds, including the
existing physical-artifact adapter checks.

Both carriers retain the full original record. Tests check exact labels and
descriptions, authority limits, absent inferred time, separate physical identity,
and no synthesized Claim edges from native membership observations. The scene
has one vertex for a composite even when its inspection packet includes both
source carriers; reducing those source carriers themselves to one would lose
inspectable provenance.

The real `ToSAccessCore` probe inspected all ten composites. Cold construction
took 26.146 seconds; peak RSS was 1,283,016 KiB. Depth-1 focus, bounded to
80 nodes/150 relations, returned 3 nodes/2 relations for nine composites and
2/1 for the Harpers' Songs composite. Nine observations were 0.295–0.309
seconds; Harpers' Songs took 1.395 seconds. Every focused subject mapped to one
scene vertex. Probe snapshot:
`30d9e474afcb8bb63322ab056dbbf387a320dfdbfb23537476d979095b139769`.
These are single local observations, not p95, UI interaction or performance
acceptance. Later documentation rebuilding changes the snapshot.

Reproduce through `ToSAccessCore.discover('.')` with `access/src` on the Python
path, then `core.knowledge_focus('tos.composite.akkadian.old-babylonian-gilgamesh-fragments',
depth=1, node_limit=80, relation_limit=150)`. Inspect exact metadata with
`core.knowledge_node` using that same ID. The generated
`ToS/source-witnesses/catalog/composites.jsonl` provides the remaining IDs and
their exact source returns.

Catalog build, graph build/validation and source-foundation validation passed.
The access knowledge-contract module passed 59 tests in 42.838 seconds.
The full bibliographic graph module passed 75 tests in 268.509 seconds and the
processing module passed 16 tests in 2.545 seconds. The expanded collision test
passed separately in 0.743 seconds: a declared Corpus-shaped profile cannot
take over the existing native composite namespace.
Manual boundary review: yes to exact source return, no identity replacement,
preserved observations/rights/uncertainty, no promoted content and no new owner
authority. Canon, public payload acquisition, lived-witness consent,
calibration and deployment are not applicable to this diff.

## Remaining work and rollback

This connects existing material rather than creating a separate demonstration
corpus. It does not complete textual reconstruction growth, native human forms,
native assessment bindings or the Parmenides/Simplicius route. Those remain with
the ToS source/assessment owners and need explicit compatible extensions; no
fictional physical member may be used to satisfy the v1 contract. Reader
rollback does not alter any source record or decision history. Full UI,
Worker/D1, CI, merge, publication and deployment are not proved by this review.
