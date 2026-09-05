# Tree of Sophia web client

The default entry is the Observatory: a full-window, living star field with
bounded knowledge neighborhoods and floating search, source, and research panels.
It is a read-only consumer of the access backend. Notes, hypotheses and proposals
remain in the existing local research workspace; they do not write to ToS.

## Run and verify

From this directory, run `npm ci`, `npm run typecheck`, `npm test`, and
`npm run build`. `npm run dev` proxies `/api` to the local review backend on
`127.0.0.1:44258`; change that development target for another local backend.
Production always uses same-origin APIs.

`dist/` is tracked. Rebuild it from source. Both HTTP and edge adapters load the
stable `/static/assets/tos-graph.js` bootstrap and `tos-graph.css`; imported view
JavaScript and CSS use content hashes. No CDN, inline scripts, iframe, or relaxed
CSP is required.

## Ownership and compatibility

- `src/entry.ts` selects the default Observatory. Existing `mode`/`view` links and
  `workspace=classic` still load `src/main.ts`, with its full research commands.
  The old shell is a compatibility route, not the template for new panels.
- `src/observatory/scene.js` owns camera, selection, navigation history, sky and
  layout. `gpu-canvas.js` batches the accepted painter through Three 0.185.1;
  Canvas remains the fallback if GPU creation fails.
- `knowledge-client.mjs` validates LensResult authority, revision, unique opaque
  IDs, closed relation endpoints, and a 40-node/80-relation display budget.
  A large corpus never directly determines per-frame scene size.
- `knowledge-ui.mjs` handles paged search, inspection, scene request cancellation
  and explicit retry. A failed request retains the current graph. Inspection
  checks both source and content revisions before entering the card cache.
- `workspace.mjs` rebuilds source dossiers, notes, hypotheses, proposals, source
  gaps and source-bound word-analysis preparation in floating panels. It reuses
  `query-operations.ts` and `research-workspace.ts`; there is no second backend.
  Unavailable source material is shown as unavailable, not reconstructed.
- `research-actions.ts` validates a complete proposal before changing the local
  journal. Existing workspace storage and export packets remain compatible.
- `evidence-panel.mjs` presents grounds and reading comparison in a floating
  reading sheet. Source routes, permitted conclusions, open questions and
  competing/contextual relations come from the existing Evidence Lens. The
  selected relation is excluded from its own comparison; partial coverage and
  the absence of competing readings do not establish agreement or truth.
- `evidence-model.mjs` binds that older query only through explicit `source_graph`
  and `native_id`, checks selection identity, source overlap and authority, and
  checks the knowledge snapshot before and after the query. Philosophy records
  use the philosophy route; canon records attempt the bounded corpus route graph.
  A 404 means the object is outside that route, not that evidence does not exist.
  Other layers show their inspected provenance without synthesizing Evidence Lens.
  The old endpoint has no revision token: these checks are not an atomic snapshot
  guarantee, and evidence responses are deliberately not cached.
- `panels.mjs` coordinates auxiliary window visibility and resize invalidation.
  Registering another panel does not change the camera or painter. This is a local
  UI seam; integration with the backend UI constructor remains a later slice.
- `navigation-panel.mjs` presents paged connections, start/end selection, bounded
  alternative paths, temporary query exclusions, and a return to the starting
  camera/view. An exploration page keeps its own schema and query provenance;
  it is not cast to a LensResult. Requests use 10 primary nodes and 14 relations
  so even focus and endpoint context fit the 40-node scene. Pages replace the
  visible area; they are not accumulated into an unbounded graph. Continuation
  checks snapshot, query, focus and page sequence, and shows cumulative counts
  explicitly as counts for this traversal, not the corpus total.
- `navigation-model.mjs` uses the existing philosophy path service only for
  records with explicit `source_graph=philosophy` and `native_id`. Every returned
  route is compiled through native-ID filters on that source, then bound to
  exact knowledge IDs with endpoint, direction, exclusion and provenance checks.
  A route has at most 8 edges, a request at most 5 alternatives, and only one
  route is displayed at a time. The old path service has no atomic revision
  token; surrounding source/content checks and current hop validation do not
  prove snapshot atomicity. Other knowledge layers support neighborhood
  exploration but do not yet have a path-between-two-objects backend contract.
  Query exclusions stay in this panel; they do not edit the graph or the local
  research journal. Saved route comparisons remain a later integration slice.
- `webmcp.ts` can restrict registration to commands implemented by the active
  shell. IDs, source references and deep links are returned intact. Legacy
  saved path comparison remains on compatibility routes. Path start/find,
  rerouting without a selected relation, and neighborhood opening are available
  in the Observatory; path tools require the explicit source capability. Evidence and
  reading comparison are also available in the Observatory when the selection
  has an explicit evidence route, independently of path-routing capability.

The Observatory requires the knowledge/lens API. The integration branch combines
backend `7f4aae92` with UI `2b1d6800`; their common base `61cc594c` alone cannot
serve this client. Local integration is not publication: verify the combined
release and production Worker before making this default client public.

## Accepted visual and input baseline

The source was ported from the approved connected prototype `5237fadde36a`.
The accepted gesture HTML had SHA-256
`97ac2a949ad0b66efc4be121b60171f33c05a20bb0a3036f65bba698df9ff6d2`;
the GPU painter had SHA-256
`c8d4575cf448b8707cdf55ae779d2743482a02f7348ae5e66f4972983bfe18cb`.
The painter differs only in its npm import. Projection, focus, panel placement,
star/fog drawing, frame smoothing, wheel/pinch and pointer-drag blocks were
compared directly to that source during migration.

Preserve pan gain 0.55 and response 22/s; pinch gain 0.006 and response 30/s;
zoom bounds 0.65–2.2; gentle selection zoom ×1.08 without accumulating on a
reselected open card. Background motion respects pause, reduced motion,
visibility and offscreen state. The app fits the viewport rather than retaining
the standalone export's 740px minimum. Mobile auxiliary windows are capped at
42dvh. User confirmation of physical pinch on the earlier prototype is not a
physical-device acceptance test for this built application.

## Migration verification (2026-09-05)

- 48 frontend tests; TypeScript and Vite build passed.
- 39 access tests and standalone source validation passed on the clean base.
- Local real HTTP handler + current backend lane served the built assets with
  its normal CSP. Initial knowledge area contained 29 nodes and 29 relations.
- Browser checks covered selection zoom, source dossier, local note persistence,
  WebMCP search/select/inspect, long IDs, source-gap selection, anchored notes,
  proposal staging and undo, and 390×844 mobile layout.
- Simulated browser wheel packets reached pan and pinch handlers. Physical
  trackpad feel and final visual approval remain with the operator.

This is source/build/browser verification, not CI, merge, release or deployment.

## Evidence reading slice verification (2026-09-05)

56 frontend tests, TypeScript, Vite build, 39 base access tests and standalone
source validation passed. Real HTTP/browser checks covered the Archaic Tribute
node and its contested relation, distinct comparison subjects, intact source
references, WebMCP evidence/comparison, unchanged camera while switching tabs or
scrolling, source-panel handoff, keyboard tabs/Escape, offline error/retry and a
390×844 sheet with no horizontal overflow. The source-navigation work correctly
shows provenance with an unconnected evidence route. Camera/input and GPU painter
code stayed unchanged; only panel obstacle discovery changed in the scene module.
Large-data performance and the backend constructor remain subsequent slices;
this does not renew physical trackpad acceptance.

## Connections and paths slice verification (2026-09-05)

63 frontend tests, TypeScript, Vite, 39 base access tests and standalone source
validation passed. Real HTTP checks bound two paths from the A01 clay tablet to
the Uruk III corpus; excluding one first hop leaves one alternative, excluding
both yields a bounded no-path result. Exploration resumes with the same snapshot
and closed endpoints. Browser checks covered endpoint search with disambiguation,
variant switching, exclusion, identical camera before/after switching and return,
restoration of the original node set, four Zarathustra exploration pages, and
WebMCP start/find/neighborhood commands. Unsupported source-navigation paths
show a capability message. A 390×844 panel measured 360×354.48 (42dvh); a
1280×720 panel measured 450×440, with no horizontal page overflow at either size.
The painter, scene CSS, input/focus code and classic app source stayed unchanged.
New gesture-device acceptance, full performance profiling, CI and deployment
were not run in that UI slice. The subsequent local integration is described below.

## Local backend integration (2026-09-05)

The integration preserves every UI source and built asset from `2b1d6800`;
the Vite rebuild reproduces the same bytes. Shared documentation currentness
is rebuilt from the combined sources. The combined tree passes 63 web tests,
110 access tests, 20 Worker tests, TypeScript checks and standalone source
validation. The actual UI adapters also pass a loopback HTTP check for focus,
search, node/relation inspection, four exploration pages, contested evidence,
a bound path and exclusion yielding no path for that bounded query.

Browser verification on the combined local server covers initial rendering,
selection/inspection and exploration continuation. This does not renew physical
trackpad acceptance, prove atomicity of the legacy evidence/path endpoints, or
establish production performance. Cold local graph construction remains slow;
CI, clean release/archive validation and deployment are separate gates.
