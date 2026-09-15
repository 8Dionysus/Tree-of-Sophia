# Tree of Sophia Access

`access/` is the standalone, source-read-only product surface for Tree of
Sophia. It gives native MCP, HTTP, CLI, the web application, and WebMCP one
query core and one fingerprinted projection set. Page commands may change the
browser presentation, but never ToS source, review, rights, or canon state.

Authored meaning stays under `ToS/`. This package reads allowlisted derived
exports and always returns their `source_ref` routes; it never promotes canon,
changes review state, or writes to the corpus.

## Development install

Create a local environment with `python -m venv .venv` and install with
`.venv/bin/python -m pip install -e 'access[mcp,dev]'`. Install and build browser
assets with `npm ci --prefix access/web` and `npm run build --prefix access/web`.
Program development and tests need no production corpus or AbyssOS installation.

To read production data, explicitly select an existing compatible snapshot:
`export TOS_DATA_ROOT=/path/to/data`. Then run `.venv/bin/tos doctor`,
`.venv/bin/tos serve`, or `.venv/bin/tos mcp`. The reader does not search parent
directories for data. Software-owned contracts and browser assets do not come
from the selected dataset. Compiling or admitting new data is a separate data
operation; software edits do not trigger it.

`tos serve` is loopback-only by default. `tos mcp` uses stdio unless an explicit
loopback-only HTTP transport is selected with `TOS_MCP_TRANSPORT`.
The software-only artifact and verification route live in
[RELEASING](../docs/RELEASING.md); `data_included: false` distinguishes it from
older combined bundles. Browser build outputs are no longer Git companions.

The backend exposes two source-navigation operations over the selected native
navigation product (or the corpus index in the legacy carrier mode).
`tos.source.descend` / `GET /api/source/navigation/{node_id}` walks from
an era, region, tradition, planting, or source object toward bibliographic
objects and Links. `tos.dossier.inspect` /
`GET /api/source/dossiers/{object_id}` returns a compact Work, Expression,
Edition, Item, File, or Link dossier:
the connected Work/Expression/Edition/Item/File chain, observed Links, scoped
rights records, gaps, source refs, and a fail-closed `agent_summary`. A
downloadable URL is reported as technical access only; legal openness requires
an accepted human rights review.

An explicitly pinned published SQLite reader uses that publication's native
`source_navigation_*` tables and seek indexes for both operations. It shares
the legacy traversal/dossier semantics, holds the selected snapshot through
the whole operation, verifies full payloads against selection columns, and
applies row, byte and SQLite-work budgets. Missing native navigation is an
explicit unavailable-product error, not permission to read another corpus
index. In particular, normalized knowledge rows alone do not provide the
complete native header and scoped rights required by a source dossier; a
prepared normalization product without those companions cannot serve one.

## Backend-defined knowledge construction

The backend also exposes one normalized knowledge graph across the philosophy
projection, authored canon node relations, canon relation packs, source
navigation, reified source claims, semantic interchange routes, and indexed
repository structure. Every node carries both its stable `entity_id` /
`type_id` and its source-native identity and kind, plus a localized title, kind label, summary,
epistemic posture, provenance, source references, query attributes, and a
lossless public `source_record` with digest and field mapping.
Absent an explicit source posture, only the `canon` carrier defaults to
`authority_layer: canon`; other carriers, including `source-claims`, default
to `derived-export`. This transport default never supplies a canon status or
changes the recorded review posture.
Every relation likewise carries a stable `relation_type_id` beside the exact
source predicate, localized labels, a readable endpoint statement, an
explanation, provenance, and the same return route to source. When source prose
does not exist, the backend emits a deterministic metadata synthesis and marks
it as such; it never presents generated wording as authored ToS meaning.
Missing prose is reported with a short localized notice, not filled with
repository paths, machine statuses, or internal review terminology. Those
values remain available in structured attributes, provenance and source refs.
Synthesized relationship statements use available localized endpoint and
predicate labels; source-supplied prose and translations are preserved verbatim.
Language/script keys are extensible (including `grc-Grek`, `zh-Hant` and
private-use tags); the catalog exposes observed display fields and availability.
Fallback preserves available source wording without pretending to translate it.
The shared browser reader consumes the source-owned `tos_readable_context_v1`
sidecar on full material reads. It checks exact material digests, field bindings
and declared coverage before keeping a bounded reading snapshot. Owner labels
and explanations describe governing context; technical fields stay in a
disclosure, and unclassified fields retain their keys and literal values.
Canonical numeric material preserves integer/float lexemes independently of
JavaScript number rounding. A budget-limited or unavailable presentation stays
an explicit gap with the original form context still accessible; it never
becomes an empty successful explanation. This is transport verification, not
independent verification of the vocabulary's judgment, semantic assessment,
translation, rights or admission. Older responses without the sidecar keep
their existing context reader rather than acquiring invented classification.
For canonical corpus nodes, the retained authored `properties` record owns
wording, not the outer index label: an ID-derived navigation label remains
`identifier-fallback` with `source_title_available: false`, including when
reading an older index. Its real `distilled_thesis` remains available separately.
Unresolved relation endpoints likewise acquire no source name or description
from their ID. This correction changes normalized display/provenance and requires
an explicit prepared-reader migration; it is not an execution-only compatible
profile update and does not activate an existing service.
See the [language transport contract](contracts/README.md) for compatibility
roles, fallback order and the still-distinct full Forms work.

For an explicit migration diagnostic, run
`PYTHONPATH=access/src python -m tos_access.coverage --root . --language en`.
This scans the existing normalized projection and reports per-source carrier
counts, mapped/unmapped types, missing display wording, derivation, and each
HumanForm role's delivery and candidate states. `--rows` streams one NDJSON
observation per node/relation before the terminal summary. Without that final
summary, an interrupted row stream is incomplete. Output contains references
and mechanical states, not source wording; its visibility still follows the
input snapshot and must not be treated as a public-safe derivative by default.

The old `display_coverage` totals count transport fields, including notices;
they do not measure substantive quality. This separate diagnostic excludes
notices from available descriptions and distinguishes generated navigation
from source-marked wording. The latter reports the projection's provenance,
not independently verified source authorship. It does not score quality, assess rights or admit
content. Rows retain both carrier and subject identity, so duplicate carriers
do not inflate a claimed number of distinct subjects. An absent role requires
its source owner's applicability review, not an invented description. Exact
source return and next-action categories accompany each row. Objects absent
from the projection, live source-byte parity, generated currentness and full
corpus migration remain separate checks. The scan is explicitly offline, not
a new HTTP/MCP/Worker operation or additional work on the hover/query path.

Clients discover the construction vocabulary through `tos knowledge catalog`
or `GET /api/knowledge/catalog`. The catalog reports current kinds,
predicates, safe fields, observed `attributes.*` fields and value types,
facets, operators, bounds, display coverage, stored lenses, and common entity
routes for concepts, authors, works, words, traditions, places, and source
objects. An entity route reports `not_projected` instead of pretending that a
missing kind exists; `role_readiness` says whether a contextual role such as
authorship is relation-confirmed or only a kind candidate, rather than guessing
from a label. Lens counts keep global query matches, topology-eligible
relations, returned relations, and selector truncation distinct. A human UI and
an agent can therefore build the same `tos_lens_spec_v1` document rather than
depending on browser-owned view logic. A LensSpec can select sources, start
from nodes or relations, filter, traverse, close relation endpoints, group,
sort, and provide presentation hints under explicit resource limits.

Native and offline catalogs share one exact contribution/renderer implementation.
The [offline catalog index](EXACT_CATALOG_INDEX.md) supports caller-transaction
bootstrap and addressed deltas, retaining reversible counts, source-ordered
representatives/examples and affected entity-route closure. It does not publish
prepared metadata or activate a reader. The explicit `prepared_catalog` join
updates catalog, full carriers, search and lenses in the same caller transaction;
source/semantic admission, commit and reader activation remain owner operations.
The [joined semantic maintenance API](PREPARED_MAINTENANCE.md) additionally
computes the exact semantic report and verifies that the final publication
contains the identical checked changes, under a combined SQL mutation budget.

Registered properties can be queried without knowing their internal paths.
For example, `{"property_id":"tos.property.time-role","op":"eq","value":"historical-time"}`
is a node filter, also usable at a path step. Discover the actual descriptor
and operators in `semantic_registries.properties`; an unknown property is an
error, not an empty success. Values remain source-declared and type-scoped;
missing values are not proven unequal. See the [property-filter contract](contracts/README.md)
for exact string/array semantics and snapshot compatibility.

The versioned ToS registries define the stable entity hierarchy and relation
contracts consumed by this graph. They keep Agent roles as relations, separate
Work/Expression/Edition/Item/File/Link, distinguish Place from a navigation
Region, and keep dates as TemporalAssertions. Source assertions retain Claim,
evidence, provenance, review, and supersession structure. Cross-layer
`projects` and `grounded-in` routes are admitted only from shared declared ToS
IDs or exact authored source references; `same-as` is never synthesized.

An exact historical description or Claim has its own `record-version` carrier,
not the current subject's ID. For supported public metadata families,
`has_record_version` links the subject to references from its verified retained
`record_history`; Sign's `promotion_basis_version` instead binds its exact birth
Claim. Inspect or focus those ordinary graph IDs through the same read-only
operations. Available versions quote their own source wording and language;
compact results retain the complete historical qualifications. Unavailable
versions expose a gap, never current wording or a new assessment/admission.
The [source reader contract](../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#read-only-exact-metadata-versions)
states supported families and record-only (not whole-package) verification.

Exact source reading has a separate opt-in
[transport contract](contracts/source-read.v1.schema.json). Discover it with
`GET /api/source/contracts` or `tos_source_read_contract`; check the selected
owner with `GET /api/source/capabilities` or `tos_source_read_capabilities`.
The default CLI/server selects no source owner and reports `available: false`.
An embedding explicitly supplies `SourceReadService` to `ToSAccessCore.discover`.
Its `SourceOwnerBinding.from_prepared_source` requires the actual source
assembler's complete vector verification and addressed catalog/readers;
constructing a vector object or copying a source revision is insufficient.

For an explicitly bound owner, `POST /api/source/handles` /
`tos_source_handle_discover` accepts a typed catalog selector or exact
owner-issued target. `POST /api/source/read` / `tos_source_read` accepts the
returned handle and `representation: "record"`. The same core returns exact
public metadata or Claim content without accepting filesystem paths, byte
ranges or a latest-version fallback. A well-formed selector absent from the
catalog returns `missing`, with no fabricated target, digest or handle.
Reads recheck the owner epoch and source bytes; handles grant no use rights.
Full node/relation inspection returns `source_read_targets`, keyed by the exact
returned graph ID, only where a complete retained owner record supplies the
target. This projection does not assert source-reader availability. The
constructor's **Open source record** action follows that target through the
same handle/read operations used by agents; it never guesses a filesystem path
or substitutes the latest version. Selection changes and dialog closure cancel
the read, with a 15-second overall deadline and a 2 MiB response ceiling.
Original notes and explicit gaps remain distinct from technical fields, carrier
text, assessment and use rights. An unconfigured owner reports unavailability.

Authored relation CSVs use the separate `authored_csv_record` layer and issuer
`Tree-of-Sophia/authored-corpus`, not a fabricated source-witness record/version.
Its selector contains only `layer`, `pack_id` and `edge_id`. Exact targets also
bind logical `source_row`, `source_file_sha256` and canonical parsed-cell
`content_revision`; handles belong to the same selected source epoch.
Activation requires an explicitly bootstrapped `authored-corpus` addressed root
in that source vector. An older vector without it reports `unsupported`.
The record preserves every original string/null cell; provenance returns raw
CSV text, byte offset/count, raw-row hash and the distinct canon/intake owner
posture. This is source disclosure, not semantic assessment or a rights grant.
Python/native inspection projects the exact target; the human source reader
and MCP use the same handle/read operations. A source-vector addition still
needs matching prepared publication and separate live UI verification.

For local source-owner selection, pass all four exact inputs:

```bash
tos --root /absolute/source-owner --prepared-read-model /absolute/snapshot.sqlite \
  --prepared-binding /absolute/binding.json --source-inputs /absolute/inputs.raw \
  serve --host 127.0.0.1 --port 8080
```

The source vector must match the prepared reader revision. The source mechanics
come from the access implementation checkout, never from caller-selected source
data. A portable bundle without those mechanics cannot activate this route.
`SelectedSourceReadService` creates fresh bounded owner readers per operation,
with at most two concurrent source operations and no waiting queue. Reader
caches and work counters do not accumulate across a server lifetime. Reselection
after source changes is explicit; no request quietly chooses a newer snapshot.
The same selection works with `mcp` and the `source capabilities`, `source
contracts`, `source discover REQUEST.json`, and `source read REQUEST.json` CLI
commands. Discovery/read accept `-` for bounded JSON input from stdin.

Native artifact/composite metadata uses its schema-declared `artifact_id` or
`composite_id`, not a fabricated `record_id`. Exact reads bind the existing
metadata owner's native-witness descriptor, version, digest and public metadata
scope; this does not expose scans, transcriptions or grant content-use rights.
An explicitly selected local source owner also advertises
`representation: "native_public_unit"` on the same read operation. It reopens
the handle's exact metadata record, takes only its native TextUnit binding,
and invokes the source owner's bounded public-span reader. This is not an
upgrade of the metadata handle's authority: both native closure and recorded
unconditional public rights are checked independently before text is returned.
`tos_source_native_unit_read_result_v1` keeps `record: null`; `native_unit`
contains ordered spans only on success, with separate `text_access`. Private
or conditional gates return `access-restricted` and no text. The constructor
offers this explicit action inside the source record, preserving the scene,
selection, exact Unicode text and separated spans. Client validation binds the
native IDs/versions and packet/layer digests back to the exact metadata record,
and checks each span's code-point length and UTF-8 digest. The 15-second shared
deadline includes both metadata and text requests. Local-reader conditional
permission remains distinct and is not inferred from available metadata.

For existing conditional local-reader permission, explicitly select
`--source-local-text-selection /absolute/protected/selection.json` alongside
`--source-inputs` and its prepared reader. The file follows
`ToS/contracts/native-local-text-read.schema.json`, is owned by the current
account with mode 0600, and pins the source root, exact binding and rights,
current mandate, a reviewed explanation of the conditions, and exact license
and attribution notices. Validity is at most one day. This records execution
under existing permission, not new rights or a positive assessment. Never
select a grant supplied by an untrusted source or a consumer request.

The separately requested `native_local_unit` returns ordered exact spans with
`local_conditions` and the complete selected notices. The constructor's
"Read under local conditions" action retains those notices next to the text;
agents must retain them with the return too. Public-declared native visibility
is still mandatory: no private transport or original payload is enabled.
Configuration, mandate, rights, closure and notice changes fail closed;
revocation/expiry requires explicit owner reselection, never a public fallback.
HTTP remains loopback-only. No remote Worker or external publication is enabled.

The default observatory's Sources panel uses this same exact reader for a
selected node or relation, not only bibliographic dossiers. Open the source
record to see its unchanged fields and version-bound provenance. When it has
a native-text binding, the panel discovers the selected owner's advertised
public/local representations. Each explicit text request still rechecks its
own access conditions; neither discovery nor metadata visibility is a grant.
A text refusal leaves the metadata record readable. Local text retains the
complete license and attribution notices alongside its separate spans.
The panel never reconstructs a source path or substitutes another revision.

Private source-owner reading is a separate lower-level contract:
`ToS/contracts/native-private-text-read.schema.json` and
`scripts/native_text_return.py` expose `PrivateTextReadSelection` plus
`read_private_unit(selection, binding)`. The protected selection pins a distinct
`OwnerLocalSourceContext`, the current account and mandate, a maximum one-day
validity, and each exact binding and rights-record digest. It permits only the
already recorded unconditional local-research route, returns unchanged selected
spans with complete rights records, and rechecks revocation and dependency
fixity before returning. It does not read the original Item payload, assess
translation or content, or change private visibility and redistribution rules.

This private return is **not yet connected to HTTP, MCP or the constructor**.
Those adapters must supply their own explicit owner-local transport boundary
and respect any recorded server-transfer prohibition. Neither a public metadata
handle nor the existing `native_local_unit` request selects a private store or
inherits this permission. Never turn its packet into a public source response.

Historical Claim and review-ledger reading remain separate integration work.
A build or mocked client test alone does not establish the live
constructor-to-owner cycle.

The read-only operations are available through all backend adapters:

- `GET /api/knowledge/contracts` returns the operation map, exact JSON
  Schemas, and semantic registries consumed by constructor clients, so neither
  a UI nor an agent must infer LensSpec shape or type meaning from examples;
- `GET /api/knowledge/search`, `/nodes/{id}`, and `/relations/{id}` search and
  inspect the normalized graph. The default search remains the exact-count,
  offset-based `tos_knowledge_search_v1` route. An explicit
  `mode=indexed` request uses the source-revision-bound trigram carrier and
  cursor continuation (`tos_knowledge_search_indexed_v2`); short queries and
  over-budget candidate sets fail closed rather than silently falling back to a
  full scan. The projection page keeps its separate `tos.page.search` v1
  semantics. `tos.page.knowledge-search` checks the selected backend's search
  capabilities and uses an available indexed or prepared compressed engine,
  never an implicit legacy fallback. It returns `search_mode` alongside the
  unchanged native cursor; keep both when continuing from another page session.
  The observatory uses the same advertised engine selection for human typing
  and agent search, not the legacy offset route. Its ordinary search keeps at
  most 16 previous-page cursor bindings (up to 1 MiB), without caching result
  rows. Unknown total counts stay unknown. The explicit knowledge-search tool
  supports continuation independently of that local navigation history.
  An explicitly compiled query store uses its
  existing FTS5 trigram index directly, with snapshot/query-bound keyset
  cursors and candidate/verification budgets; this route never reconstructs
  the graph or creates another search index during a request. A scan-only
  compiled store reports indexed search unavailable while retaining the
  explicitly separate legacy search route.
- `GET /api/knowledge/focus/{node_id}` resolves an exact normalized ID, a
  stable entity ID, or one unambiguous native ID and returns a bounded radial neighborhood with an
  explicit `focus` object. Ambiguous native IDs fail closed so the caller can
  disambiguate through search;
- `GET /api/knowledge/lenses/{lens_id}` executes a stored LensSpec;
- `POST /api/knowledge/temporal/compare` compares the normalized date envelopes
  of two exact source Claims selected from one snapshot. Discover the request
  schema through `/api/knowledge/contracts`; unknown grounds remain unknown,
  and the result keeps both full Claim contexts without accepting either;
  the separately declared `catalogue-assigned-document-date` role compares
  only with the same role after exact Document/profile/source/value binding.
  It does not compare a letter's catalogue date as an event date. Unknown
  calendar or year numbering returns `undetermined`, never an inferred date;
  this new reader carries a source-owner canonical JSON companion in
  `semantics.claim.source_canonical_json`, bounded to 262144 UTF-8 bytes.
  This companion is available to full inspection and temporal comparison;
  compact carriers omit it while retaining the other Claim semantics and refs.
  Missing, over-budget or mismatched bytes return `undetermined`. Python and
  the Worker bind the whole Claim, value and literal identity; the Worker
  preserves number tokens from the actual D1 row instead of reconstructing
  source hashes with `JSON.stringify`. Existing historical carriers do not
  acquire the documentary role or require this companion;
- `POST /api/knowledge/lenses/compile` executes an arbitrary validated
  LensSpec. This `POST` carries structured query data only and creates no
  server state;
- native MCP exposes the corresponding `tos_knowledge_*` tools; CLI exposes
  `tos knowledge catalog|contracts|search|node|relation|focus|temporal-compare` and
  `tos lens open|compile`.

The direct agent loop is `search -> focus -> inspect or refine`. For example,
`tos knowledge search 'Ницше' --kind agent` returns the namespaced Nietzsche
Agent, and `tos knowledge focus source-navigation:tos.agent.friedrich-nietzsche
--sources source-navigation --depth 1` constructs the author-to-works
neighborhood. The same operation can be restricted by direction, predicates,
node limit, and relation limit. A raw LensSpec can also set
`seed.focus_node_id`; its result always repeats the resolved center separately
from the ordinary node array.

The default `overview` neighborhood does not traverse record-production
relations (`tos.relation.made-by`, `tos.relation.generated-by`). A common maker
or serialization event is not a semantic relationship between the recorded
subjects. Exact node/relation inspection and `profile=all` retain those links;
Claim subjects, objects and evidence remain available in overview. The catalog
publishes exact exclusions. Unknown relations are not classified by their label
or native predicate spelling. Exploration uses the same rule; its v2 execution
version invalidates checkpoints from the earlier traversal semantics.

Local search prepares JSON substring documents once for the current immutable
knowledge snapshot. Repeated queries do not serialize the entire graph again;
filters, exact/prefix ranking, metadata matches and pagination retain the same
contract. A core instance retains only its current search index and replaces it
when the graph snapshot changes. Preparation adds first-search cost and memory;
substring scanning remains linear. The pure search function still works without
an index. Callers must not mutate a core-owned snapshot in place.

For example, save a LensSpec in `lens.json`, then run
`tos lens compile lens.json`. The same document can be sent as JSON to
`POST /api/knowledge/lenses/compile` or passed to the native MCP compile tool.
The result includes the normalized items, groups, facets, source revision,
per-item content revisions, and a deterministic fingerprint. The public
schemas and operation map live under `access/contracts/`.

Lens results and resumable exploration pages also deliver `scene`
(`tos_knowledge_scene_v1`, defined in the knowledge-graph schema). This is a
packet-local map, not a replacement graph: `vertices[].node_ids` partition
the returned exact carrier IDs, grouping only an identical declared `tos.*`
entity ID. Names, similarity, `same_as` and native-ID fallbacks never merge
vertices. A Claim retains its own identity, distinct from its subject/object.
`vertices[].id` stays stable when another carrier of that subject appears in
a later lens/page; it is a presentation key, not an inspection or query ID.

`representative_node_id` chooses only the default card/display carrier, in
source-navigation, canon, source-claims, philosophy, candidate-intake,
repository, semantic-interchange order, then Unicode code-point ID order.
This is not a verdict on competing records or forms. Consumers inspect each
original `node_id` and `content_revision`; they must not relabel or merge its
content. `focus_vertex_id` locates the focused carrier in the scene.
`arcs` point back to each exact `relation_id` with mapped scene endpoints.
Only a typed `tos.relation.projects` link within the same vertex is omitted
from arcs and listed in `collapsed_relation_ids`; it remains inspectable in
`relations`. Other self-relations and parallel assertions remain distinct.

The map is computed **after** source filtering and delivery pagination. It
does not discover other carriers, widen a query, change admission or bypass a
page budget. Its work/storage are bounded by the returned packet, with sorting
at most O((nodes + relations) log(nodes + relations)). Older packets may lack
`scene`; consumers must then preserve exact carrier vertices, not invent a
grouping heuristic. Execution v5 introduced compact-path fingerprints; current
v6 additionally binds typed origins. Older cached responses require a restart.
UI adoption is separate.

### Compact Claim paths

`scene.compact` offers an alternative presentation of the **same returned
packet**. Its `vertex_ids` select scene vertices; `relation_ids` select existing
scene arcs. `claim_paths` add presentation lines from subject to object, each
bound to one exact Claim carrier. They are not new normalized relations or
new assertions. Consumers choose this view explicitly (normally for overview)
and retain the full scene for technical inspection or expansion.

A path requires a mapped predicate, explicit subject/object IDs and exactly one
consistent typed `has-subject` / `has-object` leg present in the packet. The
ordered `node_ids` and `relation_ids` decode that path. Claim-supported-by
relations are retained as `detail_relation_ids`; their target records remain
in the packet. Other incident edges prevent folding. An incomplete, ambiguous,
unmapped, mixed-carrier or focused Claim stays a vertex with a reason in
`retained_claims`. Focusing on its grounds also keeps the neighborhood explicit.
Only isolated detail vertices fold, never an incomplete Claim used as grounds.
An explicitly selected raw relation takes priority over coincident Claim-node
focus: its Claim stays unfolded with `focus-relation`, keeping that exact edge
visible and inspectable in both native and shared scene implementations.
Self-relations between a subject and itself remain possible; a Claim cannot
share its subject/object presentation identity. Competing Claims keep separate
path IDs even when their subject, predicate and object coincide.

`reading.node_id` and `content_revision` identify the exact Claim whose wording
and context the line must expose. `wording_pointer` selects a complete ready
source form (caption, statement, then hover), otherwise an available source
summary/title; it never substitutes an ID or generic missing-description text.
A null pointer is an explicit wording gap. Source-form context must travel with
its `display_text`. Reading also requires the Claim's `context_pointers` and
the semantics/epistemic context of `relation_context_ids`. Polarity, conditions,
attribution, time, disagreement, assessment and unknown qualifiers must not be
dropped. `standalone=false` forbids treating the predicate label or shortened
wording as an unconditional fact. Exact inspection retains all source fields.

No model call, assessment, source admission or new historical inference happens
while constructing these lines. Human forms are reused through the existing
source-bound selection contract; exploration pages now use its automatic
language selection too. All original nodes, relations, content revisions and
source refs remain unchanged. `folded_vertex_ids` names presentation omissions,
not deletions. A partial page may retain a Claim until a later, sufficiently
complete view; consumers must not infer missing legs from other snapshots.

Explicit local assessed-form projections may carry the source-owned
`assessment_snapshot` annotation. The shared graph constructor requires exact
source and selected-packet parity between bibliographic and navigation carriers;
it refuses a mixed pair instead of choosing one carrier's wording. Python and
Worker form selection validate the annotation's transport shape, including the
absence of publication/runtime authority. Neither performs a fresh assessment
or authenticates a model invocation. The source owner controls currentness and
publication; see [local assessed snapshots](../ToS/doctrine/HUMAN_FORMS.md#local-assessed-research-snapshots).

The in-memory compatibility reader reuses snapshot-bound adjacency and lens
plans for focus, supplied lenses and stored lenses. Plans retain references, not
copies of full packets; at most four source scopes and two relation plans per
scope are cached. Publication replaces the current index while an in-flight
reader may finish with its borrowed old snapshot. Human-form delivery and
pagination are still computed per request. These are plan-count bounds, not a
hard byte ceiling; first-use planning and global selectors can still scan the
in-memory snapshot. This optimization does not establish cold prepared-query
performance or remove the compatibility reader's initial graph construction.

## Explicit prepared local reader

To build a fresh snapshot from an explicitly selected source tree, use the
[offline prepared bootstrap](OFFLINE_PREPARED_BOOTSTRAP.md). It assembles the
whole source graph; it neither switches consumers nor performs an incremental
source update. Its explicit `--attach-maintenance` option adds the exact catalog
and auxiliary semantic indexes before completion, under separate write budgets
and unchanged reader binding; attachment remains disabled by default.

The [local prepared publisher](LOCAL_PREPARED_PUBLICATION.md) also provides an
explicit offline, one-file full-row/catalog/lens/compressed-search profile and
addressed storage deltas. It accepts normalized owner inputs, has its own local
schema, and installs no public route or edge deployment.

Python callers can opt into the published SQLite read model already emitted by
the edge producer, using both `ToSAccessCore.discover(...,
published_read_model_path=..., published_read_model_expected=...)` arguments.
The expected value is an independently owner-selected snapshot binding framed
by `published_snapshot_binding(top, publication_epoch)`: read-model schema,
source/data revisions, normalization binding, small-header checksum, and the
actual `knowledge_exploration_clock` epoch. Merely trusting the database's own
header does not establish current source or policy authority.

This first prepared slice supports catalog and full node/relation inspect,
including identity aliases, exact incident counts and endpoint closure. It opens
read-only query transactions, verifies the small header and selected-row emitted
JSON checksums, and never builds a graph, normalizes sources, or creates an
offline normalization cache. Catalog bytes are loaded and checked only for a
catalog request. Full packets retain human forms, source pointers, provenance,
unknown fields and typed false/zero values. No compact response substitutes for
the full inspected record. Checksums detect accidental byte drift; they do not
authenticate a writer holding the same producer/filesystem authority.

Prepared temporal comparison uses the same transport-neutral computation as
the source-backed reader, over at most six exact Claim/value/document-subject
lookups in one selected read snapshot. It has no identifier-alias fallback.
Selected full rows retain `source_canonical_json` and Python JSON number types;
source/content revision conflicts, missing Claims, damaged carriers and read
budgets remain explicit refusals. Missing or unsupported date evidence retains
the shared `undetermined` or `unsupported` states. Comparison does not adjudicate dates or
establish new source, review, rights or canon authority.

Publication must include the prepared metadata and row digests through the
normal producer and apply the existing exploration-clock/seek-index migration.
Missing, corrupt, stale or concurrently replaced publications refuse explicitly;
there is no request-time migration, repair, revision reselection or full-graph
fallback. Staleness is relative to the supplied expected binding: the reader
does not rehash live source files or independently discover policy revocation.
The serving owner must preserve or advance its clock across restore. Recreating
identical header bytes with a reset clock is not distinguishable after restart.
Row, byte and SQLite-work budgets refuse oversized exact inspections
instead of returning approximate counts. Readers keep no connection or graph
across requests/restarts. SQLite may use its ordinary WAL coordination sidecars;
`mode=ro` and `query_only` prohibit database writes, not SQLite's filesystem
coordination protocol.

The distinct `tos_local_prepared_read_model_v1` profile may reuse these
catalog/inspect, lens and exploration carriers without the edge-v9 compatibility
search tables. It requires the same lens metadata and ordered indices, but an
edge-v9 binding never implicitly selects it. The explicit offline publisher and
joined compressed search are described below; source assembly and a live
consumer selection remain separate responsibilities. The Cloudflare
producer's v9 schema and capabilities are unchanged.

Compressed search is available only through explicit `mode=compressed` on this
local profile; [its contract](COMPRESSED_SEARCH_V3.md#local-adapters) covers
capability discovery, bounded work, continuation and full-carrier joins.
Lens/focus and exploration use the prepared services described below.
The default, without these two arguments,
retains the existing compatibility route. This is not completion of the broader
cold-reader or addressed-source publication work, nor a production activation.

The executable accepts the same explicit selection, including HTTP and MCP:

```bash
tos --root /path/to/runtime-data --prepared-read-model /path/to/selected.sqlite \
  --prepared-binding /path/to/owner-selected-binding.json knowledge catalog
tos --root /path/to/runtime-data --prepared-read-model /path/to/selected.sqlite \
  --prepared-binding /path/to/owner-selected-binding.json \
  --exploration-checkpoints /path/to/local-continuations.sqlite mcp
```

Use `serve` instead of `mcp` to start local HTTP. Binding JSON is limited to
64 KiB and loaded from the separately chosen file; it is never recovered from
the selected database after mismatch. The optional checkpoint path is a distinct
local continuation store, not authored knowledge. Without it, exploration
checkpoints remain process-local. `doctor` and `verify` still inspect the
source-backed profile and reject prepared flags rather than claiming its health.

`PublishedLensService(reader, limits=PublishedLensLimits(...))` executes native-v7
lens and focus semantics over an owner-published v9 snapshot. `execute(spec)` and
`focus(node_id, **options)` share the native property binder, focus specification
and final packet builder: query normalization, human forms, fingerprints,
inclusion reasons, grouping, ordering and stateless pagination remain identical.
Each page re-executes the complete bounded selection; it never substitutes a
subgraph for global `available`, `matched` or `eligible` counts.
The explicitly selected prepared core routes `knowledge_focus`,
`compile_knowledge_lens` and `stored_knowledge_lens` to this service, including
the existing HTTP and MCP adapters. Stored-lens catalog lookup and execution
both enforce the same publication binding and epoch; publication between them
refuses instead of mixing versions. The default source-backed core is unchanged.

The producer owns small exact source/kind/type and source/predicate/relation-type
histograms plus Python-lowercase order keys and local incidence indices. This
keeps default focus and dimensional/default-sort lenses off whole-graph scans.
General filters, native Unicode/scalar/list operations, mixed sorts and path
witnesses evaluate through bounded Python callbacks and keyset candidate streams.
Explicit seed IDs first use the exact/entity/native identity-index union, retaining
all source-scoped aliases rather than applying focus's representative selection.
The default ceilings are 2,048 candidates, 32,768 callbacks, 16 MiB decoded row
bytes, 4 MiB sort-key bytes, 100,000 path steps, and a 64-entry/2 MiB row cache;
reader row/byte/SQLite-work ceilings also apply. These are logical budgets, not
an RSS guarantee. Exact internal-edge counts use equality probes over the bounded
selected basis, avoiding unrelated high-degree edges; a large local basis can
itself hit the VM ceiling. Exhaustion refuses the entire request: no partial match count,
false negative path witness or approximate successful packet escapes. No SQLite
UDF invokes an unmetered native callback. Selected full rows check their emitted
digests and order-carrier mirrors. Header-bound histograms are producer evidence,
not a request-time recount or authentication of a same-authority writer.

Lens execution checks its native execution version and Python Unicode database
version. Old v8 publications retain catalog/inspect/explore support but cannot
serve lens/focus until the owner publishes v9. `reader.status()` only checks the
selected publication metadata and required indices, never all source rows; its
packet explicitly says `verifies_all_rows: false`. No service builds or migrates
a missing publication while handling a request.

`PublishedExplorationService(reader, ...)` provides native-v6
exploration over the same pinned reader. Each page uses bounded identity and
two-sided adjacency keyset windows; it does not load the catalog, build a graph,
count the whole neighborhood or use offset scans. Its full stream preserves
native query, ordering, work units, page limits, v2 origin closure and terminal
status. The opaque snapshot hash is instead SHA256 of compact JSON containing
`tos_published_exploration_snapshot_v1`, the native execution version and the
complete selected publication binding. Random cursors and this runtime-specific
hash are not portable identifiers.

By default checkpoints remain bounded process memory and do not survive restart.
An explicit constructor-only `checkpoint_path` selects a separate private 0600
SQLite query-state file and reports `restart_survival: true`. The parent directory
must already exist. The source read model and symlink paths are refused. A
cross-process write transaction atomically commits replay and successor after
the read-only source snapshot check; failures leave the previous state intact.
Every replay rechecks the selected source binding and epoch. UTC wall-clock
expiry persists across restart; backwards clock movement refuses without changes.
Schema/execution mismatch, busy state or corruption fails closed without reset,
migration or source revision reselection. The private execution configuration
also binds scene implementation `selected-relation-first-v1`; older checkpoint
files without that marker are incompatible, including stored replay responses.
The owner must explicitly select a new checkpoint path and start a fresh query.
Existing files are not deleted or migrated, and public execution/schema versions
and publication snapshot framing remain unchanged by this presentation fix.

Persistent capacity separately limits records, logical payload bytes and SQLite
database pages. DELETE-journal mode avoids unbounded WAL growth; a transaction's
rollback journal can temporarily add approximately one database cap plus SQLite
headers. Expired and evicted records are pruned on successful transactions and
free pages reused; this is disposable query state, not source history. Replay and
successor must fit together. The capability response exposes the actual mode,
limits, database cap, overhead and cleanup boundary. The explicitly selected
prepared core routes `knowledge_explore` and its capability response to this
same service. `ToSAccessCore.discover(..., published_exploration_checkpoint_path=...)`
selects persistent checkpoints; without that additional constructor argument,
the prepared core keeps process-local checkpoints. No environment variable,
request field or default server configuration activates persistence. This wiring
does not establish real-corpus performance or a deployed service.

The HTTP factory accepts that same configured core. The native MCP factory can
also bind it with `build_server(core=selected_core)`; this is mutually exclusive
with discovery-path arguments and keeps the core's exploration/checkpoint policy.
The default MCP factory retains its existing discovery behavior. HTTP capability
responses expose the selected mode; stale snapshots return 409, expired cursors
410, exceeded read budgets 413, and unavailable read/checkpoint stores 503.

In prepared mode `/health` checks only the selected publication header, binding
and required indices. It returns `scope: selected-prepared-publication` and
`verifies_all_rows: false` inside `read_model_status`; it does not scan corpus or
philosophy sources, prove every item digest or check checkpoint-store readiness.
An unavailable or changed selected publication returns health status 503.
The default source-backed health checks remain unchanged. The prepared MCP
factory is also tested over a real stdio handshake and tool discovery in separate
server processes, replaying the same persistent continuation after restart.

## Constructor boundaries

Construction coverage is deliberately bounded: selectors, type ancestry,
numeric temporal intersections, grouping, focus profiles and compact carriers
are implemented. `detail=compact` affects delivery only; inspect by ID returns
the full public record. `profile=all` includes text units and anchors;
the default focus profile is a bibliographic/conceptual overview.

`path_query` adds up to four conjunctive conditions over selector roots, each
with one to four directed steps. Each step has independent typed `node_query`
and `relation_query` filters. `exists` requires a matching walk; `not_exists`
means absence **within the selected sources**, not absence in the world. Walks
may revisit nodes. The focus remains explicitly included even if it does not
match the selector. These conditions do not restrict subsequent neighborhood
expansion; use `relation_query`/`traversal` for that. D1 executes correlated
joins; local engines enforce a path-inspection safety ceiling and fail rather
than interpreting exhaustion as absence. Arbitrary named-variable joins,
universal quantification, and unbounded path expressions remain unsupported.

`explain: true` returns recorded `inclusion` causes for returned nodes and
relations: focus, selector, traversal, or endpoint closure. Selector causes
include path witnesses or scoped absence. Referenced witness relations may be
outside the visible result and can be inspected by exact ID. This describes
query execution, **not** evidence acceptance or the truth of a relation.

For example, this read query selects Works with an exact Nietzsche authorship
path and records the matching relation IDs. Add a one-hop traversal if the
author and its lines should also appear in the visible graph; path witnesses
alone do not force their intermediate nodes into the scene.

```json
{
  "schema_version": "tos_lens_spec_v1",
  "lens_id": "works-by-nietzsche",
  "sources": ["source-navigation"],
  "detail": "compact",
  "explain": true,
  "node_query": {"filters": [
    {"field": "type_id", "op": "eq", "value": "tos.entity.work"}
  ]},
  "path_query": [{"path_id": "author", "steps": [{
    "direction": "outgoing",
    "relation_query": {"filters": [
      {"field": "relation_type_id", "op": "eq", "value": "tos.relation.authored-by"}
    ]},
    "node_query": {"filters": [
      {"field": "id", "op": "eq", "value": "source-navigation:tos.agent.friedrich-nietzsche"}
    ]}
  }]}],
  "pagination": {"nodes": 2, "relations": 2, "cursor": null}
}
```

`pagination: {"nodes": 40, "relations": 80, "cursor": null}` enables delivery
pages of the **bounded** LensResult. Reuse the same LensSpec with
`page.next_cursor`; `page.has_more` refers only to that result, not to the entire
corpus. A changed query, source revision, selected content, or execution version
rejects the cursor with HTTP 409. There is no retained historical snapshot.
Pages re-execute the bounded lens; they do not yet carry a graph traversal
frontier beyond `limits`. Page sizes may change without invalidating the cursor.
Tokens are public validated seek positions, not authorization credentials.

Primary nodes and relations appear once over continuation; focus and endpoint
context nodes may repeat and must be upserted by `id`. `page.primary_node_ids`
and `context_node_ids` distinguish them. Up to two endpoints per returned
relation plus the focus can supplement the primary-node page size. `counts`,
`facets`, `source_refs`, `agent_summary`, and `fingerprint` describe the complete
bounded result; `page.returned_nodes`/`returned_relations` describe this page.
Group memberships and inclusion entries are limited to the page. Search keeps
its independent offset pagination. Search and both inspection packets now
include `source_revision`; inspection still reads the current snapshot, not a
caller-selected historical one.

The local core prepares identity and incident-relation indexes once per
immutable normalized snapshot for node/relation inspection. Exact IDs retain
priority over shared entity IDs and ambiguous native IDs. A single resolved
node reads only the requested relation prefix; its total degree is already
indexed. Shared identities combine their incident neighborhoods, so that work
still grows with the matching carriers and their degree. A snapshot change
replaces the core's index, including when source revision strings coincide.
This avoids repeated whole-graph scans, not the initial normalization/index
build. It does not add historical snapshot retention, change LensSpec/search
execution, or materialize a Cloudflare/D1 index.

For an owner that already has one immutable graph and one exact replacement
carrier, `ToSAccessCore.knowledge_graph_addressed(...)` publishes a bounded
in-memory successor for the replace-only case. The caller must provide the
complete target `source_revision`; the core checks the graph's exact
normalization binding (processor, registries, and configuration), rebuilds the
changed node plus incident relations, and then reuses the result for ordinary
`knowledge_search`, `knowledge_node`, `knowledge_relation`, and
`knowledge_focus` calls. Additions, removals, dossier/Claim assembly, source
writes, semantic assessment, and publication remain on their owner routes.
The report distinguishes one submitted replacement from retained incident
payload reads and the full normalized graph scan used for global validation.
If any source input file changes, the in-memory successor is discarded and the
next graph read returns to complete builder assembly.

### Resumable neighborhood exploration

Unlike delivery pagination, `POST /api/knowledge/explore` continues an actual
BFS frontier beyond LensSpec result limits. Discover availability with
`GET /api/knowledge/explore/capabilities` and schemas with
`GET /api/knowledge/explore/contracts`. This is an optional extension;
the one-shot CLI does not implement it. Cloudflare requires the additive D1
migration and compatible read-model metadata; availability is checked live.
Native MCP exposes
`tos_knowledge_explore(request)` and `tos_knowledge_exploration_contracts()`.

Start with:

```json
{
  "focus_node_id": "source-navigation:tos.work.friedrich-nietzsche.also-sprach-zarathustra",
  "sources": ["source-navigation", "source-claims"],
  "direction": "either",
  "profile": "overview",
  "max_depth": 3,
  "page_nodes": 40,
  "page_relations": 80
}
```

Continue by sending **only** `{"cursor": "<page.next_cursor>"}` to the same
server. Query and page sizes stay fixed; changing filters starts a new walk.

For an exact selected node or relation, use the additive v2 request instead:

```json
{
  "schema_version": "tos_exploration_request_v2",
  "source_revision": "<current graph source_revision>",
  "origin": {
    "kind": "relation",
    "id": "<exact normalized relation id from the selected packet>",
    "content_revision": "<selected relation content_revision>"
  },
  "profile": "overview",
  "max_depth": 2,
  "page_nodes": 40,
  "page_relations": 80
}
```

The revision placeholders stand for lowercase 64-hex digests, not arbitrary
strings. `kind=node` selects an exact normalized node ID; v2 never guesses a
native/entity alias. The original `focus_node_id` request and v1 result remain
supported. Discovery keeps `request`/`result` for v1 and adds
`request_v2`/`result_v2`; capabilities list both versions and origin kinds.
The shared endpoint infers cursor-only continuation from its bound state.

A relation origin is not a new node, Claim, or assertion of equivalence. Its
exact `from` and `to` carriers are both depth-zero roots, irrespective of the
direction for subsequent hops. Self-loops retain both endpoint roles with one
carrier. The resolved `origin.endpoints` binds their IDs, entity IDs and content
revisions to the snapshot. At depth zero only this closure is returned: there
is no adjacency or identity expansion. The selected relation survives even
when its predicate is excluded from subsequent traversal. A selected Claim leg
remains an inspectable line and prevents folding that Claim into a different
compact line; relation focus does not invent a scene vertex.

In v2 every page repeats the origin roots as context and, for relation origins,
the selected relation in `page.context_relation_ids`. New relations are listed
in `page.primary_relation_ids`. Page budgets measure incremental discoveries,
with at most two origin nodes and one origin relation additionally retained;
ordinary endpoint context is still required. Total delivery is bounded by
`page_nodes + 2 * page_relations + 2` nodes and `page_relations + 1` relations.
`counts.discovered_nodes` includes roots; `counts.emitted_relations` excludes
the origin relation, which counts once against the internal session limit.
Repeated context never consumes a page's new-discovery budget.

V2 rejects mixed legacy/origin requests (**400**), missing or ambiguous exact
origins (**404**), source/content revision drift (**409**), source filters that
exclude the origin or its endpoints (**400**), and corrupt/missing endpoint
closure (**503**). It does not repair a prepared snapshot during delivery.
Execution v6 invalidates older checkpoints without changing authored records.

Relation IDs are ordered within each expanded carrier. In `overview`, before
expanding a carrier below `max_depth`, other source-filtered carriers of the
same declared `tos.*` entity are discovered at zero distance. The inclusion
reason is `identity-carrier`, not an invented relation or accepted `same_as`.
Names and fallback IDs never create this operation. It does not follow
similarity, reviewer agreement or shared record production.

Pending identity expansion is checkpointed and consumes the work budget.
Each identity group is expanded once. Newly found carriers count against the
same node/page/session limits and precede positive-distance steps. If an
already queued carrier is found at a shorter depth, it is moved forward; a
bounded context-node update reports the improved reason without repeating
its primary discovery. `page_nodes` bounds primary discoveries plus these
updates. An inclusion origin may name a carrier delivered on an earlier
page; it is an exact ID in the bound snapshot. At `max_depth=0` no identity
expansion occurs. `all` retains the exact-carrier BFS.

Bounded overview lenses use the same zero-distance identity rule before each
relation depth. Their ordinary raw-node limit remains in force; if identity
expansion is cut off, `counts.identity_expansion_limited` and a warning say so.
Use resumable exploration or narrower sources rather than treating the
bounded result as exhaustive. Path-condition joins still follow exact
declared relation steps; zero-distance identity does not silently alter a
caller-authored path predicate or turn a Claim path into a direct assertion.

Worker execution v4 and later additionally require `knowledge_nodes_identity_seek`
from the idempotent exploration migration. It reads identities in bounded
32-row indexed pages, using the same 24-query per-page ceiling as adjacency.
Old cached states require a fresh query; applying the migration does not
modify source records or grant deployment authority.

Apart from an explicitly selected v2 origin relation, only edges expanded from nodes below `max_depth` belong to this neighborhood,
not all possible edges between visible boundary nodes. `sources` applies to
both endpoints and relations. `overview` uses the focus profile's exclusions.
This extension does not yet accept LensSpec path conditions or arbitrary filters.

`status` distinguishes `paused`, `complete` (within the requested scope/depth),
and `limit_reached` (10,000 nodes or 20,000 relations; **not** complete).
Only `paused` has a continuation. A page can contain no new nodes/relations
while advancing through excluded edges: at most 512 edge inspections or queue
advances occur per page. Counters are cumulative discoveries, not global totals.
Use `page.primary_node_ids` for new nodes; upsert repeated focus/edge endpoints
by `id`. The node budget excludes these context nodes, which can add at most
two endpoints per returned relation plus the focus. No layout or camera reset
is implied by a backend page. Inclusion explains traversal, not semantic truth.

Checkpoints are opaque, immutable and replayable while retained. A retry returns
the same page and next cursor. They expire 15 minutes after the initial request;
expiry or bounded-cache eviction returns HTTP **410**. Local server restart
also loses checkpoints; a Worker isolate restart does not lose D1 state. A graph
change (including normalized content under an unchanged source revision) returns
**409**, even for a cached page. Malformed or mixed continuation requests return
**400**. Restart from focus on 409/410; no historical data is silently substituted.
MCP communicates the corresponding errors as tool failures, not HTTP statuses.

For partitioned corpus and bibliographic inputs, the local core reads an
explicitly compiled SQLite snapshot through indexed identity and adjacency
queries. Requests never assemble the complete graph or compile a missing store.
Short substring and unindexed property selectors can scan disk rows; request
state and returned packets still follow the declared limits. Checkpoint copying
and serialization scale with visited state. The serialized checkpoint cache is
bounded to 128 entries / 32 MiB. This is not multi-process durable checkpoint
storage or production load qualification.
No tree records, source payloads or review decisions are written. The local
service does not write files. The Worker writes only a disposable D1 cache:
128 checkpoints / 32 MiB total, at most 1 MiB per stored state or response.
Oversized admission returns **413** before creating a successor; narrow depth,
filters or page size. Missing migration/metadata returns **503**. At most 24
adjacency queries and 512 work units are allowed per D1 page; a smaller SQL
budget can cause earlier pauses than Python. Full graph lists are not loaded.
Both runtimes preserve BFS discovery order, but page partitions, execution
versions and snapshot digests are runtime-specific. Do not transfer cursors.
D1 publications increment a monotonic clock to reject even A -> B -> A changes
during a multi-query read. Concurrent continuations atomically select one replay
response and successor. See the [edge route](deploy/cloudflare-worker/README.md).

Compile the read model explicitly after rebuilding its source projections:

```bash
PYTHONPATH=access/src python -m tos_access.knowledge_compile --root .
```

The default output is `ToS/derived-exports/runtime/knowledge.sqlite3`, an ignored
build artifact. Its exact input manifests, registries and compiler version bind
the completed snapshot. Missing, stale or incompatible stores report
`query store build required`. The compiler requires SQLite FTS5 trigram support;
Foundation's `tos_offline_knowledge_v2` compiler includes typed-time and readable
context semantics. An older v1 compiled store must be explicitly rebuilt; a
software update alone does not relabel its contents as compatible.
`--search-accelerator scan` explicitly chooses the bounded-memory scan fallback.
Standalone packaging compiles its own artifact, includes the exact projection
closures, and records separate source/compiler and output identities. Compilation
runs in a fresh interpreter and checks the inputs and compiler against the
staged package before publication. ZIP and wheel writers stream runtime files;
installation validation separates wheel building from installation to release
the disposable build copy before creating the installed snapshot. See
[partitioned projection storage](../ToS/derived-exports/PARTITIONED_PROJECTIONS.md)
and the [runtime data allowlist](contracts/runtime-data.v1.json).

The legacy offline normalization path schedules a resumable dependency DAG:

source/type -> node -> endpoint title -> relation. Partitioned compilation uses
the same normalization and semantic construction rules with disk-backed
collections; it does not currently reuse that legacy DAG cache. In the legacy
path, unchanged intermediate output
stops downstream recomputation; completed steps survive failed builds. Final node
materialization and per-node, per-relation and per-Claim checks now reuse results
bound to actual input and dependency digests. Changed or missing evidence, review
versions, endpoints and registries invalidate their dependent checks. Global ID,
cardinality and registry checks still run. This does not yet schedule the entire
corpus lifecycle: source-file reading, graph indexing/assembly and SQL comparison
remain full passes on changed builds; OCR/alignment/review remain
separate source-owned stages. Broad legacy relation families and absent lexical
annotations remain explicit source/review gaps, not automatically accepted
semantic facts. Backend construction support is not completed corpus annotation.

Offline processing reports added/changed/removed input IDs and per-kind work.
`tos_access.processing.processing_input_changes` reads ID-ordered pages with
before/after digests from the cache DB; incomplete scans never imply removal.
An exact completed run also supports [bounded reverse dependency navigation](PROCESSING_DEPENDENCIES.md)
through `processing_dependency_closure`. Its read-only result describes the
retained DAG, not complete source impact or permission for partial publication.
The disposable cache has output byte/count limits, integrity checks, exclusive
builder ownership and bounded run history. See the
[retention boundary](deploy/cloudflare-worker/README.md#incremental-checks-and-cache-retention).
Private validation fingerprints use canonical JSON and are bound to the
normalization processor version. They do not replace the cross-language
framing of public source/content revisions or alter semantic validation rules.

The edge builder additionally checkpoints completed SQL and static-response
stages. An unchanged build verifies source/producer and output byte digests, then
reuses those stages without constructing the graph. Changing only web assets
does not regenerate SQL. `manifest.build_stages` distinguishes `computed` from
`reused`; `processing.status=not-run` means no normalization was requested in
this build, not that a new corpus validation ran. See the
[build-stage boundary](deploy/cloudflare-worker/README.md#resumable-build-stages).

### Opt-in backend measurement and UI compatibility

`python access/packaging/profile_backend.py --cache /absolute/scratch/cache.sqlite`
compares a fresh persistent cache with full rebuilds across cold/warm, metadata,
relation deletion, review-trace change, interruption and resume scenarios. Input
variants exist only in memory; it never changes source or serving data. It binds
input/processor digests and reports timings, work counts, equality and cache
size. Scenario times include full-result digest comparison and cache finalization;
the interruption scenario injects a cooperative exception and reopens the DB,
not a power-loss test. Use `--scenarios cold warm` for a shorter pass. The scratch cache must be
new and outside the source checkout; its adjacent `.profile.jsonl` is diagnostic
output, not source/review history. Admit CPU/RAM/storage through the host route
before a full-corpus pass and account for SQLite metadata beyond payload limits.

`python access/packaging/verify_ui_backend.py --web-root /absolute/ui/dist
--client-module /absolute/ui/src/observatory/knowledge-client.mjs` measures local
catalog/search/focus, then runs the supplied actual UI client against a temporary
loopback HTTP server. It checks focus/search/inspection/relation selection and
the HTML/CSP response, closing its server afterward. The supplied UI source must
also contain the sibling navigation/evidence modules and query operations;
Node's TypeScript stripping executes those same adapters. The check follows up
to four exploration pages, binds a source-owned contested relation to its
evidence and path, and verifies exclusion of that relation. It also measures
inspection and bounded exploration/continuation, and exercises the actual
`readMaterial` full-packet adapter for one node and both known/restored relation
identities at the same snapshot. Optional `--material-id` selects an exact public
knowledge node with human forms; `--language` selects the content language
(default `ru`). Its report retains each role's selection, exact form reference,
context-slot names and SHA-256 of the complete received packet, not its wording.
Those packet hashes use the consumer's UTF-8 JSON serialization for this
observation; they are not ToS canonical record digests or semantic acceptance.
Use `--report /absolute/scratch/query.jsonl`
to retain the timings and exact client/HTML hashes outside the checkout. This checks the named
producer-consumer seam, not browser rendering, production or deployment. Neither
command belongs in the fast test lane or modifies the UI checkout.

Full knowledge carriers may include `readable_context`, a bounded presentation
of the existing HumanForm and assertion contexts. `GET /api/knowledge/catalog`
publishes its exact source-owned vocabulary as `context_presentation` with
`id`, `version`, `source_ref`, canonical `digest`, and `payload`; catalog and
carrier must belong to the same `source_revision`. The public contracts bundle
includes `readable_context`, a thin reference to the graph schema definition.
Entries preserve raw governing values, unknowns and exact record/form/pointer
bindings. Deduplicated `exact_materials` retains canonical JSON text and hashes
at actual raw origins; verify and use its lossless numeric lexemes for display,
since ordinary JavaScript JSON numbers cannot preserve every source value.
`complete` describes returned-context coverage, not semantic review
or translation. Overflow yields `requires-exact-context` with exact roots;
invalid bindings yield `unavailable`, with no partial ready context.
Native canonical records opt in with `tos_canonical_node_v1`: their unchanged
`node_id` and safe positive `record_version` bind both source and HumanForm
context. The canonical type/ID grammar and no-`record_id` boundary match the
existing form owner. Legacy or unknown `node_id`-only records do not acquire an
inferred identity or version; unknown source fields remain visible rather than
being treated as alternative identities. Existing snapshots require explicit
normalization migration before they contain a changed context sidecar.
The complete multi-form canonical context can exceed the same presentation
budget: `requires-exact-context` then points to the unchanged source record and
mandatory HumanForm contexts, while a bounded individual form can be complete.
Compact lenses omit this optional sidecar because its raw roots are absent;
selected HumanForms retain their own mandatory context. Request full detail for
readable context and verify its vocabulary and bindings before using it.
Vocabulary/processor changes invalidate the derived stage. Carriers without
context bypass that stage and do not acquire a duplicate cache record.

Lens carriers deliver selected HumanForms using the explicit, lossless
`tos_human_form_selection_v2` envelope. Common context and literal admission
limits are transmitted once, then reconstructed before reading; source
materializations and exact inspection remain v1. The complete wire selection
keeps its 16 KiB conservative budget. Consumers support both versions,
retain raw envelopes for saved places, and validate complete decoded packets
before using wording. See the [delivery and migration contract](contracts/human-form-delivery.md)
for bounds, exact reconstruction, compact Claim pointers and rollback.

Cold normalization preserves the existing public revision byte protocol. A
bounded in-process cache reuses at most 4,096 short string tokens (up to 256
characters); longer values are streamed without retention in that cache.
Finalization still copies nodes for isolation, but reuses their existing
content revision when neither claim metadata nor view membership changes.
Catalog sampling stops after five examples per field while continuing to count
every item, value type and source. These optimizations do not skip semantic
validation or turn the build-time processing cache into a query dependency.


## Software archive

After building the browser, package the exact reviewed Git commit:
`python access/packaging/build_software_bundle.py --source-ref HEAD_SHA --output dist/tos-software.zip`.
Validate it with
`python access/packaging/validate_software_bundle.py --bundle dist/tos-software.zip`.
The adjacent external `.zip.manifest.json` binds the archive digest, while the
embedded manifest binds every member's path, size and hash. Local dirty builds
must use `--allow-dirty` and retain `source_dirty: true`.

The archive contains installable Python code, API contracts, static schemas and
built browser assets. It contains no corpus data, tests, Git metadata, sibling
repository, source payload, compiled query store or AbyssOS runtime dependency.
Verification installs a wheel in an isolated environment outside the checkout.
Select a compatible dataset separately with `TOS_DATA_ROOT` before reading it.

`build_standalone_bundle.py` and the older `validate_standalone.py --bundle`
remain explicit combined software/data tools during migration. Their compiled
input integrity checks remain active when those tools are selected. They are
not invoked by Repo Validation or ordinary software packaging. The same-run
Product Shell query-store handoff is no longer part of software CI.

The full local Tree may additionally expose a source-bound Zarathustra word
analysis capability. It resolves a German, Russian, or English query to one
exact German occurrence and prepares morphology, syntax, historical sense,
cited etymology, contextual semantics, Russian comparison, and English
rendering for the calling agent. Native MCP, local HTTP, and WebMCP use the
same read-only core operation. The standalone archive deliberately omits the
local provider and exact text, so the call returns an explicit
`available: false` packet rather than fabricating weaker evidence.

After extraction, install from any location with
`python -m pip install '/path/to/extracted/access[mcp]'`. Select data explicitly,
then run `tos verify --profile standalone`, `tos serve`, or `tos mcp`.

## Contracts

- `contracts/runtime-manifest.v1.json` defines dual runtime posture.
- `contracts/runtime-data.v1.json` is the data publication allowlist; the software archive has a separate code-only member contract.
- `contracts/query-operations.v1.json` owns transport-neutral read operations.
- `contracts/knowledge-api.v1.json` maps the knowledge catalog, search,
  inspection, stored-lens, and compile operations across HTTP, MCP, and CLI.
- `contracts/knowledge-graph.v1.schema.json` defines the normalized,
  display-complete read model.
- `contracts/lens-spec.v1.schema.json` defines the bounded declarative
  constructor grammar.
- `contracts/lens-result.v1.schema.json` defines its source-bound result.
- `../ToS/contracts/semantic-entity-type-registry.schema.json` and
  `semantic-relation-type-registry.schema.json`, with their ToS-owned registry
  data under `../ToS/doctrine/semantic-interchange/`, define the stable machine
  vocabulary without moving semantic authority into access.
- `contracts/epistemic-packet.v1.schema.json` defines the projection-bounded
  epistemic result and its fail-closed authority boundary.
- `contracts/evidence-lens-packet.v1.schema.json` defines the joined page/API
  result and its compact WebMCP `agent_summary`.
- `contracts/page-commands.v1.json` owns revisioned browser context and shared
  human/WebMCP actuation.
- `contracts/research-workspace.v1.schema.json` defines the portable local
  session packet for hypotheses, staged proposals, exclusions, route
  comparisons, notes, and its action journal.
- `contracts/web-actions.v1.json` is retained only as the v1 migration marker.
- `profiles/standalone.v1.json` is the required no-AbyssOS profile.
- `profiles/abyssos.v1.json` declares optional ecosystem adapters; the profile
  is currently paused by the ToS integration posture until an explicit owner
  command reopens it.

`tos verify --profile abyssos` additionally requires `TOS_ABYSSOS_ROOT` to
point at an AbyssOS root containing `abyss-stack`; this setting is never
required by the standalone profile.

## Cloudflare production deployment

The permanent production route under
[`deploy/cloudflare-worker/`](deploy/cloudflare-worker/README.md) builds from
this repository after changes land in `main`. Workers Static Assets carry the
web application and precomputed bounded packets; D1 carries a generated
read-only query model. The public site therefore does not depend on an
operator laptop or another always-on origin host.

The v9 D1 row schema with content revision v5 retains oversized knowledge
values in ordered `edge_meta` payload chunks. Empty inline JSON is an explicit
overflow sentinel, never a reduced substitute for the source. The native
reader reconstructs only within its existing 1 MiB row/request budgets and
checks the emitted-row digest before use; exceeding a delivery budget is an
explicit refusal, not a partial packet. Offline production admits at most
8 MiB per overflow value and still bounds every SQL row and statement.
Overlapping search fragments preserve substring matches across chunk seams,
including Unicode lower-case expansion. Compact seeds keep their existing
size and semantic limits independently of the retained full source value.
These extra metadata rows participate in the existing atomic full/delta
publication. Older readers are not a compatible overflow-delivery route.

The older route under
[`deploy/cloudflare-tunnel/`](deploy/cloudflare-tunnel/README.md) remains a
temporary recovery and local-preview profile. It publishes the same
loopback-only `tos serve` runtime through outbound Cloudflare Tunnel, but it is
not the production availability architecture.

The current `ToS` projection v1/v2 contracts remain source-owned. Their legacy
`runtime_owner` field describes the existing downstream deployment contract;
it does not override this product's standalone runtime manifest.

## WebMCP posture

The intended page agent is Codex in the ChatGPT desktop app's built-in browser.
The site does not embed a model, call the OpenAI API, require an API key, or
require users to install a separate MCP connection for this browser-native
path. Codex discovers the page's registered site tools through WebMCP and acts
on the same live page as the human. The native `tos mcp` server remains an
optional off-page access path, not a prerequisite for WebMCP.

The site feature-detects `document.modelContext` and remains fully usable when
it is absent. Stable tools expose view, search,
selection, focus, cancellation, page context, and the capability-gated
Zarathustra word-analysis task. Word analysis returns a compact agent envelope
and puts the complete task or explicit unavailable posture into the same page
inspector. Selection-dependent tools are registered for the current work,
passage, concept, source, node, edge, cluster, or other knowledge object and
bind the captured context revision, so a delayed reference to “this edge” (or
note for “this work”) fails closed after the human changes selection. Tool
execution forwards the browser-provided `AbortSignal` through page commands to
HTTP queries.

The Evidence Lens command works on philosophy projection selections and on
canonical relations in the corpus `route-graph`. Its first two curated scenes
contrast a retained Zarathustra canon relation with open modern claim/evidence
closure against the contested pre-canon Archaic Tribute reading. The page
receives the full route packet; WebMCP receives a compact summary below 1,500
characters with posture, bounded conclusion, route counts, gaps, and next
actions. Projected `contested_by`, `uncertain_relation`, and
`polemicizes_with` relations remain review leads rather than adjudicated
counterevidence.

The local research workspace lets the human and Codex work on the same
temporary investigation: inspect the semantic identity and source posture of
any selected object, compare projected readings, exclude a relation, save the
direct and alternative routes, add context-bound notes, or draw a working
hypothesis as a visibly distinct edge. Search and neighborhood tools return a
bounded agent envelope with stable IDs while the complete result remains on
the page.
Every hypothesis is structurally fixed as `session_hypothesis: true`,
`source: false`, `reviewed: false`, and `canon: false`. Undo/redo, browser-local
persistence, and a validated JSON export/import packet are supported. Codex
can also stage a typed relation, interpretation, metadata correction,
source route, or concept-enrichment proposal. The proposal is bound to a
parent hypothesis, the captured page/workspace revisions, explicit source and
evidence references, actor origin, timestamp, projection fingerprint, and a
deterministic trace digest. It is always local, `pending_review` with
`review_requirement: human_or_authorized_agent`, and `canon: false` (older
exported packets with `pending_human_review` remain readable). None of these
actions writes to candidate intake, review ledgers, authored ToS source, or
canon; export is the only handoff from this surface.

The product shell makes this shared surface visible instead of assuming the
Codex browser integration worked. Its header panel reports WebMCP availability,
registered and selection-bound tool counts, the current context revision, and
any registration failure. Without WebMCP it explains that the page should be
opened in Codex's built-in browser; the atlas remains usable and `tos mcp` is
shown only as optional off-page access. The same panel contains three
bilingual, copyable prompts for the core demonstration loop: inspect evidence,
reroute around a disputed edge, and move from comparison through a local
hypothesis to a traceable proposal that remains pending scoped review.

Every successful `Repo Validation` run builds and validates
`tree-of-sophia-standalone.zip`, then uploads the archive and its external
digest manifest as a downloadable workflow artifact. This is a source-bound
release candidate, not an official ToS release or an AbyssOS artifact-admission
verdict; official publication still follows `docs/RELEASING.md`.

The implementation follows the WebMCP Community Group draft shape current at
`webmachinelearning/webmcp@41d12f057167ccf5954dbcf49d99502cb6c84491`:
`document.modelContext.registerTool()`, registration lifecycle by
`AbortSignal`, and execution cancellation through callback options. This is an
experimental browser surface, not a ToS authority or availability guarantee.

## Independent projection utilities

The portable [partitioned projection store](contracts/projection-store.v1.md)
and [bounded Merkle diff](contracts/projection-diff.v1.md) are explicit library
utilities. Their adoption does not switch source exports, compile a query
store, change adapters, or activate source-to-prepared updates. The diff requires
a caller-admitted exact baseline and never certifies skipped target parts.
