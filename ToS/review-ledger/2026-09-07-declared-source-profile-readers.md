# Declared source-metadata reader profiles

Date: 2026-09-07. Scope: source type registry, exact metadata schemas,
catalog/graph/navigation readers and their executable extension boundary.

## Reviewed boundary

The three historical source families previously repeated their schema choice
in three Python consumers. The existing entity registry now owns that choice
as `source_record_profile`; the shared bounded reader executes it. It does not
add a second ontology registry or turn physical artifacts into Corpus records.
The catalog and knowledge-contract routes expose the same declaration.

One type can retain several explicitly understood source schema versions.
Each version selects an exact local schema plus declared dependencies. All
source versions preserve the same kind/identity prefix; a record correction
does not become another referent. Supplied previous-registry checks reject
profile changes without a version advance, removal/repurposing of old schema
routes, and silent reassignment of kind or ID prefix.

No authored source-witness record, claim, creation receipt, historical version
or adjacent form is changed by this migration. The historical v1 source
schema and Corpus metadata properties remain the source authority. Native
artifact v1/v2 identity, review/visibility fields and separate adapter remain
unchanged. No new dates, document/carrier relations, custody assertions or
semantic admission are inferred.

The metadata reader is intentionally not a complete philosophical profile:
subject-specific claims, roles, content assessment, general creation/revision
and source-visible artifact forms still need their existing owner routes.
`historical.create`, `record.revise` and form commands keep their independently
delegated scopes. Reading a profile grants no writer, publication, canon,
rights or assessment permission. An unknown profile/version fails closed
without deletion or normalization of its source.

## Checks during implementation

- First synthetic extension test failed with `KeyError: fixture-document`:
  the old catalog could not discover a new declared metadata kind.
- After catalog/reader wiring, the graph's old closed layer enum correctly
  rejected the extension. Schema and exact registry binding changed together.
- Two focused profile tests now pass: a synthetic document reaches both
  graph and navigation carriers with the same ID/type, exact complete source
  payload, unknown fields, Russian name/hover source-copy forms and catalog
  discovery. A compatible second schema route reads both source versions
  without changing the referent. These fixtures are not historical letters.
- Negatives cover unsupported readers/versions, attempted command fields,
  duplicate schema routes, adapter collisions, abstract instances, missing
  crosswalks, nonpublic visibility, wrong identity, substituted catalog
  fields/links/digest/schema, path escape, symlinks, duplicate JSON keys,
  nonfinite numbers and the 1 MiB metadata ceiling.
- Existing source-command/revision tests: 36 passed in 17.021 seconds.
- The real catalog `--check` passes before any regeneration: moving the
  historical routes into declarations has not changed its emitted entries.

Test harness setup mistakes (a wrong navigation function name, missing test
import path, and a missing positional catalog argument) were corrected; they
were not production failures or reasons to weaken the intended assertions.
The broad run exposed a boundary regression: the new internal profile error
escaped the public catalog's historical `CatalogBuildError`. That exception
contract is restored. False identity is now rejected in the catalog before
the graph can consume it; the existing negative test follows that earlier
refusal and still requires an identity-specific failure.

The source creation route also now binds consumed profile registry/schema
digests and the new helper implementation. A test first reproduced publication
after its profile contract changed since preparation; it now gets
`JournalConflict` before creating the target directory. No old receipt or
provenance packet is rewritten to describe this new implementation.

## Final local verification

- 54 graph reader tests passed (118.497 s); the added default-focus check for
  an unrecognized-in-Python synthetic type also passed separately.
- 51 access knowledge-contract tests passed (26.624 s).
- 9 corpus-index tests passed (28.704 s).
- 37 source-command/revision tests passed (26.652 s), including the consumed
  profile-contract change between preparation and application.
- 26 topology tests passed (1.925 s).
- Source-home, exact real catalog, claim-graph and corpus-index validators
  passed. The graph and corpus index were rebuilt in dependency order.
- Independent old/new JSON comparison against `bac67e93a` found exact parity
  for all 715 graph nodes, 1327 edges, 196 Claim traces, counts and layers.
  Only graph dependency digests and its fingerprint changed. Source-navigation
  matched completely; corpus resource inventory/digests account for the
  contract documentation and this review note. No authored source-witness
  or catalog file differs from that parent.

Test timings are observations of these local runs, some concurrent; they are
not a controlled performance comparison, latency budget or scaling proof.
The full private-payload-dependent source-foundation lane, whole release gate,
CI, merge, external artifact admission, D1 and rendered UI were not claimed.

## Manual review and rollback

Source return, identity separation, original language, field preservation and
authored/derived separation: yes. New human signatures or agent admission:
not applicable; this change performs neither. Physical/source-text merger,
runtime ownership transfer and UI redesign: absent. Unknown data is retained
in source, not promoted to a mapped meaning by its presence alone.

TOS-D-0044 already owns the stable-registry and lossless-reader decision. The
current doctrine and this review describe the narrower executable extension;
no new ADR is needed merely to narrate the adapter refactor.

2026-09-08 integration note: the preceding historical TOS-D-0044 reference
names the registry decision at
`d187c3c8ff7a7cb261d0be017c5dcb735643c8aa:docs/decisions/TOS-D-0044-stable-semantic-interchange-registry.md`,
now [TOS-D-0059](../../docs/decisions/TOS-D-0059-stable-semantic-interchange-registry.md)
after the explicit pre-landing ID-collision correction. It does not name the
independently landed KAG-unfreeze decision.

Rollback is to the earlier coordinated reader/registry/catalog-schema set,
not removal of source files or history. New profile records require a reader
that understands their declared grammar; an old reader must not silently
strip them to appear compatible. CI, merge, deployment, D1 parity and actual
UI interaction are not established by these local tests.
