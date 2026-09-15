# Bounded knowledge-catalog delivery

This is a handoff to the Access catalog owner. It does not define a new
endpoint or weaken the existing `tos_knowledge_catalog_v1` response. Whole-book
delivery has a separate [source-reader contract](../corpus-reader/NATIVE-MULTISPAN-REQUIREMENTS.md).

## Present consumer boundary

`ExplorationSession.discover()` obtains the complete catalog, exploration
capabilities and search capabilities, validates their compatibility and binds
them to one immutable session snapshot. The live lens builder can reuse that
exact catalog object and retain one validated executable schema. Opening the
builder does not need another copy of the same vocabulary. Replacing the
session binding, an explicit schema retry or reloading the page requires the
appropriate fresh validation. Preview and source reads still perform their
existing current-version checks; a retained catalog is not currentness proof.

The first catalog response remains required. On the selected prepared
publication measured on 2026-09-15 it was 3,566,903 bytes. Its semantic
diagnostics contained 18,425 gap entries, approximately 1.80 MB; descriptions of
2,782 node fields contributed approximately 0.82 MB. These are observations of
one publication, not limits or claims about later builders. The normal browser
response ceiling remains 4 MiB. A smaller graph neighborhood does not shrink a
global catalog response.

## Required owner split

The owner should provide a bounded, explicitly advertised startup description
and bounded discovery of the larger vocabulary and diagnostic collections:

- Startup needs the compatible contract identity, selected publication binding,
  available sources, exploration profiles and limits, and the required readable
  context and authority boundary. Search/exploration availability stays explicit.
- Type, predicate and property descriptions need searchable pages or exact-ID
  resolution. Every selected saved condition must resolve against the same
  catalog binding, including definitions, applicability, value types, units,
  languages and allowed operators. Unknown entries remain unavailable.
- Diagnostic summaries must distinguish known counts from unknown values and
  expose their bounded detail route. Deferred gap rows must not disappear from
  the reported scope or turn an incomplete assessment into a complete one.
- Contract schemas must remain bound to the supported software grammar and the
  catalog used for compilation. A source revision alone must not silently join
  incompatible contract, registry or publication versions.

The concrete packet names, identity/epoch fields, integrity commitments,
continuation grammar and byte budgets belong to Access. Consumers must not
obtain a smaller v1 response by locally deleting fields, omitting mandatory
context, substituting knowledge search for vocabulary discovery, or increasing
the response ceiling whenever the corpus grows.

## Acceptance and ownership

Before enabling a compact consumer route, verify stable startup size as the
corpus grows; complete selected-property resolution; mixed-publication and
schema rejection; cancellation and page limits; unknown totals; and explicit
unavailable states. The existing complete v1 route must retain its compatibility
contract during migration. Retained UI objects remain bounded and are released
when their session binding changes.

The current source routes are
[`ToSAccessCore.knowledge_catalog`](../../../src/tos_access/core.py),
[`PublishedKnowledgeReadModel.catalog`](../../../src/tos_access/published_read_model.py)
and [`catalog_semantics`](../../../src/tos_access/catalog_semantics.py).
The prepared reader verifies its selected catalog digest and source revision;
future partial delivery must preserve that ownership rather than manufacture
partial-catalog authority in the browser.
