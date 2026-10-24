# Authored definition tree

The manifest records the current local source selection as
`posture: selected-authored` and cites the
[composite selection review](../../../review-ledger/2026-10-19-temporal-definition-selection-review.md).
That note retains the original I/C review of 524 definitions unchanged and adds
T1's scoped independent meaning review of ChronologyVersion and TemporalCrosswalk.
The initial topology candidate passed independent acceptance (12/12); that
historical result does not review the two later definitions. Recording a review
locator is not a mechanical grant of rights, publication or runtime authority.

`canonical/`, `shared/` and `manifest.json` are the selected authored definition
inputs. The stable `../entity-types.v1.json` and `../relation-types.v1.json`
paths carry the generated compatibility pair for existing consumers. Historical
`baseline/` bytes are immutable evidence; assembled directories are disposable.
I/C combines this source-selection commit with the exact root-output replacement
and shared CI/build routing in its integration branch. The T0-only branch still
has the historical mixed-format root files and intentionally fails the new exact
compatibility guard until I/C completes that paired integration. No main,
release, production or corpus-runtime selection is implied by this local change.

The initial migration records 160 entity types, 168 properties and 196 relations
in original registry order. The current candidate manifest has 182 entity types,
168 properties and 199 relations: the two reviewed temporal types follow the
historical entries. The initial entries retain the complete migration mapping:
`collection` and stable `id` identify the old array member; `path` identifies its
selected home; `previous_paths` records subsequent home moves. Shared
properties and schema/profile metadata are not copied into individual types.
A move changes the manifest and file location, leaving the record ID, references
and semantic parent arrays unchanged. The tested move also retains the old path
in `previous_paths`; Git retains the original source revision and bytes.

## Navigation choices

`navigation.entities`, `navigation.properties` and `navigation.relations` are
presentation roots, never semantic types or predicates. The initial filesystem
layout follows the first authored semantic parent, while the manifest explicitly
holds the navigation parent. Future rearrangement edits the navigation metadata
and homes, never semantic parents. Parent order alone is only a mechanical
starting layout. The following seven primary homes received I/C review as navigation choices:

| Type | Reviewed primary home | Reason and retained second parent |
| --- | --- | --- |
| Place | Identity | The referent persists through changed names, boundaries and dated claims. SpatialObject remains a semantic parent. |
| TemporalAssertion | Assertion | This definition represents attributed dating rather than an independent time coordinate. TemporalObject remains a semantic parent. |
| ProvisionActivity | Assertion | Its owner represents a qualified source assertion about provision. TemporalObject remains a semantic parent. |
| ReceptionProcess | HistoricalProcess | Its subject is an unfolding process; ReceptionHistory remains the reception-specific classification. |
| HistoricalForgetting | HistoricalProcess | Forgetting unfolds historically; ReceptionHistory remains an equally retained classification. |
| RediscoveryEpisode | HistoricalEvent | The bounded episode is an event; ReceptionHistory remains its reception-specific classification. |
| IntellectualLegacy | HistoricalState | The inherited situation is a state, distinct from the processes producing it; ReceptionHistory remains a semantic parent. |

These are reviewed navigation decisions, not changes to the definitions’ meaning.
Initial migration preserves every historical edge. Later semantic evolution is
reviewed separately and checked against current reference/cycle constraints. Relation
navigation does not add `is_a` or `part_of`. A browser root can be added without
changing the semantic relation forest.

## Rust operations

The auto-discovered binary uses existing crate dependencies, with no Cargo/lib
edits. Run from the repository root with the executing host's resource admission:

```
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- split ENTITY_JSON RELATION_JSON NEW_TREE
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- check TREE
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- check-migration TREE
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- check-compatibility TREE REGISTRY_DIRECTORY
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- assemble TREE NEW_CANDIDATE_DIRECTORY
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- check-generated TREE CANDIDATE_DIRECTORY
```

`check` validates the manifest, one home per ID/path, navigation reachability and
cycles, semantic parent references/cycles, endpoint, inverse/supersession/property
references, historical ID retention, baseline digests and unlisted homes. A new
`split` always creates an unselected candidate, not an implicitly accepted tree.
Candidate posture retains the historical semantic-edge migration guard. Selected
posture requires the explicit repository-relative `selection_review_ref` in the
review-ledger source route; unknown postures, missing reviews and absolute or
traversing locators fail closed. The checker validates the locator shape, not
the review’s authorship or authorization; I/C and source review own those facts.
JSON values retain shared fields and original array order. Emission uses UTF-8,
two-space pretty JSON and a trailing LF. Evidence compares full parsed values
and exact bytes separately, with SHA-256 for both. Equality of parsed JSON alone
does not establish byte identity. Output equality is an independent check.

The CLI bounds each member to 2 MiB, total admitted JSON bytes to 16 MiB and
manifest entries to 8192. These protective limits are explicit errors, not
inferred ontology limits. Filesystem references reject traversal and symlinks.
The complete pair is written into an exclusively created sibling staging
directory, checked there, synced, and committed by Linux atomic no-replace
rename. Existing candidate directories are never overwritten. Interruption
before complete validation cannot select or publish a partial registry; stale
staging directories after a process kill are unselected scratch. The binary has
no operation that overwrites the compatibility pair or changes reader pointers.
Selection is an explicit reviewed manifest/source edit, never an assembly side effect.

## Acceptance scope

`tests/topology-acceptance-vectors.json` preserves the supplied oracle unchanged.
`definition_tree_native.rs` materializes all twelve cases plus negative controls.
The focused feature-minimal run checks topology, losslessness, deterministic
emission and candidate commit behavior. The default-feature consumer test also
uses both existing registry JSON schemas, the complete native semantic registry
validator, and the existing corpus-record schema for two new Place instances
with different IDs/labels. A valid instance requires no type file or generic
handler branch. Neither test creates a real source record or grants source
admission, rights, canon, publication or semantic acceptance.

The first complete round trip reports parsed equality for both registries and
byte inequality for both. Original sizes are 264272 / 335553 bytes; uniform
pretty output is 334639 / 432114 bytes. Original bytes combine inline compact
entries with pretty fields. The initial runtime receipt and exact input/output digests remain
in the execution handoff evidence and Git history, separately from test inputs. This is a formatting change, not byte-preserving output.
The immutable historical input remains independently available under `baseline/`.

## Initial migration, current-source CI and later evolution

`check-migration TREE` checks every historical ID, exact historical semantic
parent sets and **full parsed-value equality** of both registries, including all
shared fields and ordered records. Byte/format equality remains a separate
reported result. I/C runs this strict initial migration gate before replacing
both flat root files with their generated compatibility outputs. It is also a
forensic check against the pinned historical cut; it is deliberately not a
permanent gate after reviewed source evolution.

`check-compatibility TREE REGISTRY_DIRECTORY` validates the current definition
tree and compares both exact filenames `entity-types.v1.json` and
`relation-types.v1.json` in the supplied directory against current deterministic
assembly. The target is a plain directory: it requires neither manifest nor
evidence.json, ignores unrelated contents and offers no single-file subset.
Missing either file, or even whitespace-only drift in either file, fails.
`check-generated` retains its older equivalent behavior for callers already
using that command. Neither command compares current semantics to obsolete
baseline fields; both use the current selected source.

The accepted CI guard proposal, from repository root, is:

```
cargo run --offline --locked -p tos-ops-mechanics-plan --no-default-features --bin tos-definition-tree -- check-compatibility ToS/doctrine/semantic-interchange/definitions ToS/doctrine/semantic-interchange
```

Use the existing pinned-baseline `semantic_registry_transition` lane and
applicable source/schema consumers alongside that generated-drift guard. I/C
owns CI/lane/schema/Cargo and flat compatibility output changes. New type,
property and predicate definitions are ordinary manifest-listed source data:
one new unique home, explicit navigation placement, valid current references
and acyclic semantic parents. The builder has no fixed inventory counts or
per-type dispatch branches. The historical baseline does not prohibit these
extensions in selected authored posture. A new definition or semantic ancestry
does not automatically grant a source writer, reader/profile capability, rights
or admission; those routes keep their existing contracts and review.

Selected source may also change a historical parent set through the normal
owner review and semantic registry transition route. `check` and
`check-compatibility` then validate the current graph; `check-migration` reports
the deliberate difference instead of rewriting history. Historical IDs remain
present (deprecation/supersession retains their identity), and every raw baseline
byte and digest stays unchanged. Navigation-only edits still never add or remove
semantic edges. No blanket “ignore baseline” or “accept any graph” switch exists.

No runtime reader should observe a partially replaced compatibility pair; I/C
uses an immutable source revision or the reader’s exact-source revision fencing.
Existing byte-bound prepares may require refreshing when registry bytes change.
Rollback both root compatibility files and the authored source selection together
to the accepted historical Git revision, retaining this review evidence. Never
reconstruct historical raw bytes from normalized output.

## Retained regression coverage

The suite has 18 default-feature tests after bounded cleanup; research oracle
cases are requirements, not a demand for one permanent test function per case.
`tests/topology-acceptance-vectors.json` stays unchanged as the original research
oracle. Historical raw baseline files remain exact migration inputs. Runtime
receipts belong to execution evidence, not a second set of test expectations.

| Requirement / independent risk | Retained test |
| --- | --- |
| T0-001: all160/168/196 records, shared fields, deterministic pair and separate byte gate; candidate and selected postures | `t0_001_full_inventory_split_assemble` (posture table) |
| T0-002/003/004/005/009: duplicate home, missing parent, navigation cycle, semantic edge loss, unreachable home | respective `t0_002`, `t0_003`, `t0_004`, `t0_005`, `t0_009` tests |
| T0-006/007/010: move/history, relation navigation root, label-only invariance | respective `t0_006`, `t0_007`, `t0_010` tests |
| T0-008: generated divergence; both exact outputs, missing file, whitespace drift, plain directory and legacy command alias | `generated_compatibility_pair_keeps_t0_008_alias_and_plain_directory_contract` |
| T0-011: actual existing registry/schema consumers and new valid/invalid instance | `existing_schema_and_registry_consumers_accept_full_candidate_and_new_instance` (mechanics feature) |
| T0-012: interrupted staging leaves old complete output | `t0_012_interrupted_write_does_not_publish` |
| Dangling endpoint / tampered raw baseline | `extra_dangling_endpoint_and_baseline_tamper` |
| Shadow home / path escape | `extra_shadow_copy_and_path_escape` |
| Review locator/posture fail-closed | `selection_requires_explicit_source_review_locator_and_known_posture` |
| Selected parent evolution vs strict historical migration | `selected_parent_evolution_is_not_a_permanent_migration_freeze` |
| New type/predicate data extension without implicit capability | `selected_new_type_and_predicate_extend_data_without_generic_handler_changes` |
| Historical identity retention / full-value migration refusal | `selected_preserves_historical_ids_and_strict_initial_full_value_gate` |

The feature-minimal run deliberately excludes the real schema/consumer instance
check; it cannot claim complete T0-011 acceptance. The default-feature consumer
run retains that evidence. The old mapping-only instance smoke test was removed
because it duplicated setup while proving less than the retained consumer test.

## Candidate native TS role profiles

The C4 integration proposal appends three candidate definition homes under the
existing SemanticObject navigation/semantic parent: native TS definition, value
profile and operation result. All162 preceding type rows,168 property rows,196
relation rows, current temporal definitions, historical baseline bytes and prior
selection review locator remain unchanged. Entity envelope version is41. These
are declared native metadata kinds, not source-created instances, assessment or
authority. Their narrow C4 source gate remains pending; prior topology/temporal
reviews are not represented as accepting these additions. Merge/selection of
this proposal requires its actual common source review and native gates.

## PWE candidate extension

Fifteen PWE candidate entity definitions and three qualified Claim relations have
individual canonical homes and ordered manifest entries. The compatibility pair
is derived from those inputs. The historical selection review does not accept
these added definitions; their source review and native compatibility checks are
separate gates. No source instance, admission, complete evidence or historical
identity judgment is introduced by this candidate extension.
