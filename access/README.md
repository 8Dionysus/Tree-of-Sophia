# Tree of Sophia Access

`access/` is the standalone, source-read-only product surface for Tree of
Sophia. It gives native MCP, HTTP, CLI, the web application, and WebMCP one
query core and one fingerprinted projection set. Page commands may change the
browser presentation, but never ToS source, review, rights, or canon state.

Authored meaning stays under `ToS/`. This package reads allowlisted derived
exports and always returns their `source_ref` routes; it never promotes canon,
changes review state, or writes to the corpus.

## Development install

Create a local environment with `python -m venv .venv`, install the product
with `.venv/bin/python -m pip install -e 'access[mcp,dev]'`, then use
`.venv/bin/tos doctor`, `.venv/bin/tos serve`, or `.venv/bin/tos mcp`.

`tos serve` is loopback-only by default and serves the checked-in production
web assets plus the JSON API. `tos mcp` uses stdio unless a loopback-only
streamable HTTP transport is selected with `TOS_MCP_TRANSPORT`.

The backend exposes two source-navigation operations over the same corpus
index. `tos.source.descend` / `GET /api/source/navigation/{node_id}` walks from
an era, region, tradition, planting, or source object toward bibliographic
objects and Links. `tos.dossier.inspect` /
`GET /api/source/dossiers/{object_id}` returns a compact Work or Link dossier:
the connected Work/Expression/Edition/Item/File chain, observed Links, scoped
rights records, gaps, source refs, and a fail-closed `agent_summary`. A
downloadable URL is reported as technical access only; legal openness requires
an accepted human rights review.

## Backend-defined knowledge construction

The backend also exposes one normalized knowledge graph across the philosophy
projection, authored canon node relations, canon relation packs, source
navigation, reified source claims, semantic interchange routes, and indexed
repository structure. Every node carries both its stable `entity_id` /
`type_id` and its source-native identity and kind, plus a localized title, kind label, summary,
epistemic posture, provenance, source references, query attributes, and a
lossless public `source_record` with digest and field mapping.
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
See the [language transport contract](contracts/README.md) for compatibility
roles, fallback order and the still-distinct full Forms work.

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

The versioned ToS registries define the stable entity hierarchy and relation
contracts consumed by this graph. They keep Agent roles as relations, separate
Work/Expression/Edition/Item/File/Link, distinguish Place from a navigation
Region, and keep dates as TemporalAssertions. Source assertions retain Claim,
evidence, provenance, review, and supersession structure. Cross-layer
`projects` and `grounded-in` routes are admitted only from shared declared ToS
IDs or exact authored source references; `same-as` is never synthesized.

The read-only operations are available through all backend adapters:

- `GET /api/knowledge/contracts` returns the operation map, exact JSON
  Schemas, and semantic registries consumed by constructor clients, so neither
  a UI nor an agent must infer LensSpec shape or type meaning from examples;
- `GET /api/knowledge/search`, `/nodes/{id}`, and `/relations/{id}` search and
  inspect the normalized graph;
- `GET /api/knowledge/focus/{node_id}` resolves an exact normalized ID, a
  stable entity ID, or one unambiguous native ID and returns a bounded radial neighborhood with an
  explicit `focus` object. Ambiguous native IDs fail closed so the caller can
  disambiguate through search;
- `GET /api/knowledge/lenses/{lens_id}` executes a stored LensSpec;
- `POST /api/knowledge/lenses/compile` executes an arbitrary validated
  LensSpec. This `POST` carries structured query data only and creates no
  server state;
- native MCP exposes the corresponding `tos_knowledge_*` tools; CLI exposes
  `tos knowledge catalog|contracts|search|node|relation|focus` and
  `tos lens open|compile`.

The direct agent loop is `search -> focus -> inspect or refine`. For example,
`tos knowledge search 'Ницше' --kind agent` returns the namespaced Nietzsche
Agent, and `tos knowledge focus source-navigation:tos.agent.friedrich-nietzsche
--sources source-navigation --depth 1` constructs the author-to-works
neighborhood. The same operation can be restricted by direction, predicates,
node limit, and relation limit. A raw LensSpec can also set
`seed.focus_node_id`; its result always repeats the resolved center separately
from the ordinary node array.

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
Relation IDs are ordered within each expanded node; first discovery uses BFS.
Only edges expanded from nodes below `max_depth` belong to this neighborhood,
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

The local core indexes one immutable normalized snapshot once and reads local
adjacency thereafter. It still loads that snapshot in memory; checkpoint copying
and serialization scale with visited state. The serialized checkpoint cache is
bounded to 128 entries / 32 MiB, separately from the graph and adjacency index.
This is not multi-process durable storage or production load qualification.
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

The offline builder now schedules a resumable dependency DAG for normalization:
source/type -> node -> endpoint title -> relation. Unchanged intermediate output
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
inspection and bounded exploration/continuation. Use `--report /absolute/scratch/query.jsonl`
to retain the timings and exact client/HTML hashes outside the checkout. This checks the named
producer-consumer seam, not browser rendering, production or deployment. Neither
command belongs in the fast test lane or modifies the UI checkout.

Cold normalization preserves the existing public revision byte protocol. A
bounded in-process cache reuses at most 4,096 short string tokens (up to 256
characters); longer values are streamed without retention in that cache.
Finalization still copies nodes for isolation, but reuses their existing
content revision when neither claim metadata nor view membership changes.
Catalog sampling stops after five examples per field while continuing to count
every item, value type and source. These optimizations do not skip semantic
validation or turn the build-time processing cache into a query dependency.


## Standalone archive

Build a release candidate with
`python access/packaging/build_standalone_bundle.py --output dist/tos-standalone.zip`
and validate it with
`python access/packaging/validate_standalone.py --bundle dist/tos-standalone.zip`.
Validation requires the adjacent external `.zip.manifest.json` digest sidecar;
use `--manifest` when the sidecar is stored under another path.

The archive contains the installable `access/` package, prebuilt web assets,
and only the runtime data allowlist. It contains no Git metadata, sibling
repository, restricted source payload, lexical projection, Neo4j database, or
AbyssOS runtime dependency.

The full local Tree may additionally expose a source-bound Zarathustra word
analysis capability. It resolves a German, Russian, or English query to one
exact German occurrence and prepares morphology, syntax, historical sense,
cited etymology, contextual semantics, Russian comparison, and English
rendering for the calling agent. Native MCP, local HTTP, and WebMCP use the
same read-only core operation. The standalone archive deliberately omits the
local provider and exact text, so the call returns an explicit
`available: false` packet rather than fabricating weaker evidence.

After extraction, install the full standalone profile from any location:
`python -m pip install '/path/to/tree-of-sophia-standalone/access[mcp]'`. Then
run `tos verify --profile standalone`, `tos serve`, or `tos mcp`.

## Contracts

- `contracts/runtime-manifest.v1.json` defines dual runtime posture.
- `contracts/runtime-data.v1.json` is the publication/bundle allowlist.
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
deterministic trace digest. It is always local, `pending_human_review`, and
`canon: false`. None of these actions writes to candidate intake, review
ledgers, authored ToS source, or canon; export is the only handoff from this
surface.

The product shell makes this shared surface visible instead of assuming the
Codex browser integration worked. Its header panel reports WebMCP availability,
registered and selection-bound tool counts, the current context revision, and
any registration failure. Without WebMCP it explains that the page should be
opened in Codex's built-in browser; the atlas remains usable and `tos mcp` is
shown only as optional off-page access. The same panel contains three
bilingual, copyable prompts for the core demonstration loop: inspect evidence,
reroute around a disputed edge, and move from comparison through a local
hypothesis to a traceable proposal that remains pending human review.

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
