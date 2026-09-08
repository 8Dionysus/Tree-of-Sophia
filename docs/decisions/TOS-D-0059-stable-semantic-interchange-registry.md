# Stable semantic interchange registry

## Index Metadata

- Decision ID: TOS-D-0059
- Original date: 2026-09-04
- Surface classes: doctrine/ontology, corpus/contract, access/backend, access/deployment, docs/architecture
- ToS layers: doctrine, contracts, canon, source-witnesses, derived-exports, access
- Tree classes: entity registry, relation registry, claim graph, semantic interchange, constructor backend
- Guard families: source-first authority, stable identity, lossless projection, claim reification, semantic validation
- Posture: accepted

## Pre-landing identity correction

On 2026-09-08 this foundation-branch record was assigned TOS-D-0059 while
integrating `main@0e1ff330612d200750911c64a8f07fcf28c599ef`. Its original
TOS-D-0044 slot collided with the independently landed end-of-KAG-freeze
record. This corrects branch-local identifier metadata before first landing;
it does not supersede either decision's meaning, acceptance or original date.
Historical references to this registry decision resolve to
[`d187c3c8:docs/decisions/TOS-D-0044-stable-semantic-interchange-registry.md`](https://github.com/8Dionysus/Tree-of-Sophia/blob/d187c3c8ff7a7cb261d0be017c5dcb735643c8aa/docs/decisions/TOS-D-0044-stable-semantic-interchange-registry.md).
Current references use TOS-D-0059; unqualified TOS-D-0044 now identifies only
the landed unfreeze decision. Source entity and registry IDs are unchanged.

## Context

TOS-D-0058 made graph composition backend-defined, but native source kinds and
predicates still came from several independently shaped projections. A client
could discover strings such as `work`, `concept`, `author`, or
`has_normalized_place`, yet it could not rely on one stable hierarchy,
domain/range contract, evidence posture, or versioning rule. A readable
LensResult was therefore constructible, but semantic interoperability across
canon, source navigation, bibliographic claims, philosophy projections, and
future imports remained under-specified.

This gap is especially unsafe around responsibility, chronology, place, and
identity. Author is a role of an Agent, not a durable person subclass. A Work
is not an Edition or File. A navigation Region is not a Place. A date attached
to a Work is an evidence-bearing temporal assertion, not part of Work
identity. Shared labels or nearby paths do not establish `same-as`.

## Decision

Add ToS-owned, versioned semantic entity and relation registries under
`ToS/doctrine/semantic-interchange/`, governed by public JSON Schemas under
`ToS/contracts/`. Stable `tos.entity.*` and `tos.relation.*` IDs provide a
machine interchange vocabulary. Each entry declares a human-readable label
and definition, owner, lifecycle, hierarchy, and source crosswalk. Relation
entries additionally declare domain, range, directionality, transitivity,
cardinality posture, assertion mode, evidence requirement, and review
requirement.

The normalized access graph carries stable IDs beside, never instead of, the
source-native `kind_id` and `predicate_id`. Complete public input records are
retained in `source_record.payload` with its digest and field mapping;
`attributes` is the query projection, not the lossless original.
Every representation remains source-returnable.
Unknown vocabulary uses an explicit unmapped type. Registry validation rejects
cycles, duplicate mappings, missing parents or inverses, invalid cardinality,
and unknown endpoint types. Complete-graph validation rejects unregistered
types, inconsistent mapping status, unresolved endpoints, and domain/range
violations.

Model responsibility as typed relations from Agent identities. Preserve the
Work → Expression → Edition → Item → File ladder and Link as separate entity
families. Preserve authored semantic Source and Concept nodes as distinct from
bibliographic Work and source files. Model chronology as reified Claims whose
object is a TemporalAssertion with its source value, precision, interval, and
posture. Model geographic identities as Place while marking source-navigation
Region as a non-geographic navigation partition.

Materialize the source-witness bibliographic graph without flattening its
Claim, subject, object, evidence, maker, provenance, review, version, or
supersession nodes. Materialize relations embedded in authored canon node
contracts as source-owned edges. Create cross-layer `projects` only when two
representations already declare the same persistent ToS entity ID. Create
`grounded-in` only from an exact authored `source_refs` path to an indexed
canon node. Do not infer `about`, `represents`, or `same-as` from labels, path
proximity, or text similarity. `same-as` additionally requires evidence and
accepted identity review.

Expose the registry schemas and data through `tos.knowledge.contracts`, and
expose observed stable types, relation families, native vocabulary, facets,
definitions, limits, and mapping coverage through the knowledge catalog.
Python, CLI, HTTP, native MCP, the Cloudflare Worker, D1, and browser commands
consume the same versioned contract. Five traversal steps are permitted so a
consumer can follow an honest Work → Expression → Edition → Claim → Place
route without a shortcut that collapses evidence layers.

## Options Considered

- Treat every native source string as a globally stable type. Rejected because
  identical strings can carry different scope and future source vocabularies
  cannot declare hierarchy or endpoint constraints.
- Replace native source strings with one canonical ontology. Rejected because
  projection would erase authored distinctions and make the read model a
  hidden semantic authority.
- Infer cross-layer identity and topic edges from labels, paths, or
  embeddings. Rejected because similarity is a retrieval lead, not an
  evidence-bearing identity or semantic assertion.
- Add a versioned interchange registry while retaining native vocabulary and
  claim provenance. Chosen because constructor clients gain stable machine
  contracts without weakening source ownership.

## Consequences

A future UI or agent can ask for a stable Work, Agent, Concept,
TemporalAssertion, Place, or relation family and compose a lens without
hard-coding the present corpus strings. Every returned node and relation still
has a readable description, mapping status, exact native vocabulary, and
source route. Imports can be evaluated for mapping completeness before a D1
refresh; a new unknown type is visible rather than silently misclassified.

The read model grows because claims and cross-layer representations remain
first-class. Queries must therefore stay bounded, and D1 schema/data revision
must change when stable columns or source inputs change. A zero-unmapped,
domain/range-green build proves registry coverage and mechanics only. It does
not accept the truth of a bibliographic or philosophical assertion, approve
rights, or promote canon.

Released IDs cannot be repurposed. Compatible additions bump the registry data
version when released; incompatible changes introduce a successor ID and
explicit supersession. Source-native vocabulary remains available throughout
migration.

## Operational Constraints

- Change authored sources or registry mappings before rebuilding derived
  exports and D1 readers.
- Preserve all public input fields, `source_refs`, native kinds, native
  predicates, evidence, provenance, review, and supersession data.
- Fail closed on registry invalidity, unmapped production vocabulary,
  domain/range violation, or an invalid identity-equivalence claim.
- Do not perform remote D1 import or deployment as part of ordinary inner-loop
  registry development; build and compare the final read model once after the
  source and schema stabilize.
- Treat generated graph, SQL, catalog, and LensResult artifacts as disposable
  readers with no source, review, rights, or canon authority.

## Source Surfaces

- `ToS/doctrine/semantic-interchange/README.md`
- `ToS/doctrine/semantic-interchange/entity-types.v1.json`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/contracts/semantic-entity-type-registry.schema.json`
- `ToS/contracts/semantic-relation-type-registry.schema.json`
- `ToS/contracts/tos-corpus-index.schema.json`
- `ToS/contracts/source-witness-bibliographic-graph.schema.json`
- `scripts/tos_corpus_index_common.py`
- `scripts/source_witness_bibliographic_graph_common.py`
- `access/contracts/knowledge-api.v1.json`
- `access/contracts/knowledge-graph.v1.schema.json`
- `access/contracts/lens-spec.v1.schema.json`
- `access/contracts/lens-result.v1.schema.json`
- `access/src/tos_access/knowledge.py`
- `access/src/tos_access/core.py`
- `access/deploy/cloudflare-worker/src/knowledge.ts`
- `access/deploy/cloudflare-worker/src/knowledge-store.ts`
- `access/deploy/cloudflare-worker/scripts/build_runtime.py`

## Verification

Validate both registry instances against their schemas and run semantic
registry plus whole-graph invariants. Rebuild and parity-check the corpus index
and source-witness bibliographic graph. Execute a repository-backed focus
scenario centered on `tos.work.friedrich-nietzsche.also-sprach-zarathustra`
and require Agent, Source, TemporalAssertion, Place, and Concept plus authored,
grounding, claim-object, and normalized-place relations. Run standalone access
tests and validation, Cloudflare typecheck/tests, and one final temporary
read-model build. Remote D1 import, CI, merge, deployment, public runtime, and
human semantic review remain separate evidence.

## Related Decisions

- Extends TOS-D-0058's backend-defined LensSpec with stable semantic types and
  full claim-bearing source layers.
- Preserves TOS-D-0038's read-only standalone access boundary.
- Preserves TOS-D-0042's repository-driven edge and content-addressed D1
  refresh boundary.
- Preserves TOS-D-0037's source-owned graph-view membership.
