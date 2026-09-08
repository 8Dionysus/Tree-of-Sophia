# Backend-defined knowledge lenses

## Index Metadata

- Decision ID: TOS-D-0058
- Original date: 2026-09-04
- Surface classes: access/contract, access/backend, access/deployment, docs/architecture
- ToS layers: access, derived-exports, philosophy, canon, source-witnesses
- Tree classes: normalized knowledge graph, declarative lens, human display, agent interface
- Guard families: source-first authority, display provenance, bounded execution, contract parity
- Posture: accepted

## Pre-landing identity correction

On 2026-09-08 this foundation-branch record was assigned TOS-D-0058 while
integrating `main@0e1ff330612d200750911c64a8f07fcf28c599ef`. Its original
TOS-D-0043 slot collided with the independently landed Spark/legacy retirement
record. This corrects branch-local identifier metadata before first landing;
it does not supersede either decision's meaning, acceptance or original date.
Historical references to this lens decision resolve to
[`d187c3c8:docs/decisions/TOS-D-0043-backend-defined-knowledge-lenses.md`](https://github.com/8Dionysus/Tree-of-Sophia/blob/d187c3c8ff7a7cb261d0be017c5dcb735643c8aa/docs/decisions/TOS-D-0043-backend-defined-knowledge-lenses.md).
Current references use TOS-D-0058; unqualified TOS-D-0043 now identifies only
the landed retirement decision. Source entity and registry IDs are unchanged.

## Context

The public access product could open a finite catalog of graph views and expose
several specialized query packets. That made existing material visible, but it
left selection, grouping, presentation hints, and substantial naming logic in
the browser. Adding a genuinely new lens therefore still required UI code, and
an agent had no single contract for discovering available fields, composing a
view, or explaining every returned node and relation.

The source projections also carry heterogeneous shapes. Labels, comments,
epistemic posture, repository metadata, and provenance exist in different
layers. Flattening them into one unqualified graph would erase important ToS
boundaries; returning only the common denominator would discard most of the
material.

## Decision

Add an access-owned normalized knowledge read model and a declarative
`tos_lens_spec_v1` execution contract. Stable ToS sources and generated
projections remain authoritative. The access layer namespaces native IDs by
source graph and, where source-local IDs collide, by collection or relation
pack. It preserves the original `native_id` for discovery, preserves the public
source payload losslessly across a stable envelope and `attributes`, and adds
stable `display`, `epistemic`, `view_ids`, and `source_refs` fields.

Every normalized node has a title, kind label, and summary. Every relation has
a label, endpoint statement, and explanation. Source text is preserved when it
exists. When it does not, access emits a deterministic metadata description,
marks that synthesis in provenance, and reports the missing source-level
description as a coverage gap. Synthesis is never represented as authored
meaning.

Humans and agents use the same LensSpec grammar for source selection, node and
relation filters, node- or relation-first construction, bounded traversal,
endpoint closure, grouping, sorting, layout hints, inspector fields, and hard
result limits. Stored legacy views are LensSpecs executed by the generic engine;
their IDs are conveniences, not branches in backend logic. The catalog exposes
the supported vocabulary, observed attribute types and examples, filter
facets, operator value contracts, and limits so a future UI can be a
constructor over backend capabilities rather than their owner.
The same backend exposes the exact operation map and JSON Schemas as a contract
bundle. Repository paths remain source references, but deployed humans and
agents do not need filesystem access or example-based inference to construct a
valid LensSpec.

`seed.focus_node_id` makes a center explicit instead of asking a client to
infer it from the result list. Exact namespaced IDs win; a native ID is accepted
only when it has one match in the selected sources. The result repeats the
resolved, display-complete center in `focus`, and the convenience
`tos.knowledge.focus` operation builds a bounded radial lens with predicate,
direction, depth, and result-limit controls. Endpoint closure operates from a
frozen selection basis, so iteration order cannot recursively escape the
requested neighborhood.

The catalog also exposes common entity routes. These connect human terms such
as concept, author, work, and word to source-derived `kind_id` values and,
where needed, confirming predicates. They report unavailable kinds as
`not_projected` and expose separate role readiness so a generic Agent node does
not silently become an Author without an authorship relation. Lens result
counts likewise distinguish global relation-filter matches from relations
eligible around the selected nodes and from actual truncation. In particular,
the current tracked Zarathustra lexical
projection is hash-only and explicitly creates no word, lexeme, or public site
route, so access must not turn its form hashes into word nodes.

Local Python, native MCP, HTTP, CLI, and the Cloudflare Worker expose the same
read-only operations. Cloudflare stores normalized rows in D1 and compiles
validated filters to bounded SQL/traversal work; it does not load the complete
graph into each Worker invocation. Large metadata objects are imported and
reassembled as bounded UTF-8 chunks so every generated statement remains
inside the D1 statement contract. The structured HTTP query uses `POST` only
to carry JSON and creates no server state.

The source-navigation projection includes public responsibility claim packets,
including the seven explicit Nietzsche `authored_by` claims, rather than
inferring author links from directory layout. Source-record variant labels and
notes remain source-returnable node metadata, allowing Russian and English
entity search and display without access-authored translation.

D1 data revisioning follows row truth rather than development churn: source
inputs, actual normalized item content revisions, capability data, and an
explicit read-model schema version trigger import. Lens grammar, catalog,
documentation, and Worker-only changes do not. Structural SQL changes require
a schema-version bump. This keeps fixture and pure-engine tests in the inner
development loop and reserves the full D1 import for a completed data/schema
change.

This decision extends TOS-D-0038's standalone boundary and TOS-D-0042's
repository-driven edge. It does not replace TOS-D-0037's source-owned view
membership or move view meaning into access.

## Options Considered

- Continue adding one endpoint and one browser implementation per view.
  Rejected because the UI would remain the effective composition owner and
  agents could not construct unknown lenses.
- Publish one large undifferentiated graph document. Rejected because it would
  lose source-layer identity, make ordinary edge requests high-memory, and
  encourage a derived payload to be mistaken for canon.
- Allow arbitrary JavaScript, Cypher, or SQL from the browser. Rejected because
  executable queries are not a stable portable ABI and would enlarge security,
  resource, and persistence authority.
- Define a bounded declarative grammar over a lossless normalized read model.
  Chosen because new compositions become data, transport implementations can
  validate the same intent, and source authority remains explicit.

## Consequences

A replacement UI can discover kinds, predicates, fields, stored lenses, and
limits at runtime. It can create materially different graph constructions
without changing backend code, provided they fit the versioned grammar. Agents
receive the same catalog, search, inspect, compile, provenance, and explicit
authority boundary as people.

The normalized graph is larger than the former minimal packets, so local access
caches it by input revision and the edge uses indexed D1 rows. Result limits,
filter counts, traversal depth, request bytes, safe field paths, and endpoint
closure are contract invariants. A canonical source revision participates in
every result fingerprint together with per-item content revisions, so
unchanged membership cannot hide changed source material or a changed display
envelope. Truly new operations or semantics still require a contract version;
“arbitrary lens” does not mean arbitrary code.

Human-readable completeness does not imply scholarly completeness. Coverage
counters deliberately reveal where ToS has only metadata synthesis and where a
future source-owner pass can add reviewed summaries or explanations.

## Source Surfaces

- `access/AGENTS.md`
- `access/contracts/knowledge-api.v1.json`
- `access/contracts/knowledge-graph.v1.schema.json`
- `access/contracts/lens-spec.v1.schema.json`
- `access/contracts/lens-result.v1.schema.json`
- `access/src/tos_access/knowledge.py`
- `access/src/tos_access/core.py`
- `access/src/tos_access/http_server.py`
- `access/src/tos_access/mcp_server.py`
- `access/deploy/cloudflare-worker/src/knowledge.ts`
- `access/deploy/cloudflare-worker/src/knowledge-store.ts`
- `scripts/tos_corpus_index_common.py`
- `ToS/derived-exports/tos_corpus_index.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`

## Validation

Validate the three JSON Schemas against generated graph, stored LensSpecs, and
representative results. Run the standalone access tests and validator, the
Cloudflare TypeScript tests and typecheck, a local D1 build/import, and
Python-versus-Worker contract comparison for catalog, search, inspect, stored
lens, and arbitrary lens execution. Regenerate and validate the decision
indexes. CI, deployment, and public runtime acceptance remain separate landing
claims.
