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
  On desktop, a tool inherits the inspector's free side (or opens opposite the
  selected star) so the window does not cover the focus. Tool-to-tool handoffs
  retain that side; mobile sheets retain their bottom placement. This changes
  neither the camera nor the painter.
  Registering another panel does not change the camera or painter. This is a local
  UI seam shared by evidence, navigation and the lens constructor.
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
- `lens-panel.mjs` adds the constructor under **Линзы → Конструктор линз**.
  It selects roots from the original area, an exact focused star, or the whole
  tree, with source, text, kind, predicate, direction and depth controls. All
  vocabulary comes from the live catalog and LensSpec schema. Root filters do
  not masquerade as conditions on traversal context; the preview distinguishes
  selected roots, returned context and truncation.
- `lens-model.mjs` compiles these controls through the existing lens API, bounded
  to 40 nodes, 80 relations and depth 3. Changes compile after a 400ms pause
  and update the scene directly, retaining camera pose and established star
  positions. The status shows applied counts or explains an unchanged result.
  Closing, changing conditions or changing the area invalidates pending work. Empty results and failures keep the graph.
  The original-view action restores its scene and camera bookmark.
  Up to 12 definitions are saved under a separate browser-local storage key,
  updated by name. URL links carry validated conditions and exact original-area
  IDs, not data snapshots or authority; reopening recompiles current data.
  Arbitrary backend LensSpecs, server persistence and paginated custom-lens
  expansion are outside this first constructor slice.
- `lens-vocabulary.mjs` groups choices by advertised registry roles and relation
  definitions, filters them by exact source mappings, and sorts readable labels
  or catalog-wide frequency. Groups start collapsed and share one panel scroll;
  search or existing selections open their groups. Selected
  filters outside the current sources remain visible and removable; unknown
  mappings remain discoverable. This is presentation, not a new semantic registry.
  Search spans all groups; longer groups reveal further choices without losing
  selection. Changing the vocabulary list never silently changes the query.
- `webmcp.ts` can restrict registration to commands implemented by the active
  shell. IDs, source references and deep links are returned intact. Legacy
  saved path comparison remains on compatibility routes. Path start/find,
  rerouting without a selected relation, and neighborhood opening are available
  in the Observatory; path tools require the explicit source capability. Evidence and
  reading comparison are also available in the Observatory when the selection
  has an explicit evidence route, independently of path-routing capability.

The Observatory requires the knowledge/lens API. The integration branch combines
backend `7f4aae92` with UI `6518e240` (including the panel docking fix over
`2b1d6800`); their common base `61cc594c` alone cannot
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

The integration preserves every UI source and built asset from `6518e240`;
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

## Lens constructor verification (2026-09-05)

71 frontend tests, TypeScript, Vite, 110 access tests and standalone source
validation passed. The live backend advertised 7 sources, 79 node kinds and
115 predicates. Real HTTP checks covered exact area roots, global source/kind
filtering (10 displayed of 40 selected works), empty results and focus traversal.
Browser checks covered preview without changing the graph, apply and return
with identical camera pose, saved-definition selection, link reload, empty
results, actual offline failure and retry, and closing during a delayed apply.
The latter retained the previous graph. At 390×844 the sheet measured
360×354.48 with a 195.48px scrolling body and no horizontal page overflow.
The scene changed only to pass an initial lens link; camera, gestures, painter
and scene styling remain unchanged. Browser context includes the custom lens
sources, active predicates and full deep link. Saved links recompile current
data; they are not frozen research snapshots. CI, production deployment,
large-corpus performance profiling and physical gesture acceptance were not
performed in this slice.

## Lens interaction correction (2026-09-05)

Operator feedback rejected the original two-step count-preview/apply flow as
unresponsive. Conditions now update the actual graph after a short pause, with
explicit same-composition and empty-result messages. Technical and substantive
choices are grouped using the backend registry. The prior slice's two-step
interaction is superseded; its camera, budget and source-ownership boundaries
remain. Validation for this correction is recorded in the task's commit review.

## Reading continuity and personal space (2026-09-05)

The next UI slice builds on the live lens constructor at `6440248cf`:

- `reading-state.mjs` keeps bounded page-local reading positions by exact object,
  source revision and section. It restores expanded references and a text anchor
  after tab changes or asynchronous content arrival. The panel host captures before
  hiding a surface. A contextual return trail goes from sources to evidence and
  back to the selected card; another graph/selection cannot reuse that trail.
- The registered panel host supports remembered window sizes, pointer and keyboard
  resizing, and automatic/left/right docking. The scene stays dominant: desktop
  sheets are limited to 48% width and mobile sheets to 42dvh. Long descriptions
  and available source text use a readable serif measure, with a larger-text option.
  This does not manufacture full text when the owner API only provides metadata
  and source links. External source links keep their existing explicit routes.
- `scene-feedback.mjs` marks additions briefly and differentiates selector/focus
  from traversal/endpoint inclusion using the advertised query-execution evidence.
  These cues describe inclusion in a view, not truth or semantic importance.
  Existing ID-based positions survive changes; the GPU painter is unchanged.
- `place-model.mjs` and `view-state.mjs` store at most 12 named places: bounded
  query/draft, opaque identities, layout and camera, selection, and card section.
  They never persist source packets or resumable exploration cursors. Reopening
  always makes a fresh validated read; changed source revisions are reported.
  An exploration page saves its visible IDs as a bounded view, not its traversal
  continuation. Empty or failed reads leave the current scene in place.
- `studio.mjs` adds **Моё пространство → Места / Инструменты**. The last view is
  remembered locally and restored on a matching URL or home; an explicit different
  deep link takes precedence. Named places support update, removal and immediate
  undo. Storage failures remain visible and do not replace a damaged place list.
- `interface-model.mjs` restricts composition to the registered local adapters.
  Search, lenses, research, navigation, lens constructor, evidence and sources can
  be pinned and reordered; all remain available in the tool list. Preferences
  contain no executable code, service endpoints or additional write authority.
  Existing WebMCP and backend action contracts remain the execution boundary.

Camera changes in this slice are limited to the validated saved-view bridge,
initial-load sequencing, and card-section/read-position restoration. Accepted
wheel/pinch/pan gains, projection and GPU painting remain the baseline. New
visual arrival effects respect reduced motion. Small-screen connection lists,
previously hidden by prototype CSS, are available again. Resize grips support
arrows and Home; tabs retain roving keyboard focus.

Verification for this slice is local. Physical device feel, cross-device account
sync, arbitrary remote panel plugins and production rollout are separate scopes.

Validation: 84 frontend tests, TypeScript, Vite build, and the `standalone_access`
lane (110 tests plus source-profile validation) passed. Browser checks covered
named-place save/open, exact desktop and mobile camera restoration after reload,
reading return (636px to 636px), resize controls and keyboard arrows, tool pinning,
docking and text preferences, offline failure/retry, and closing a delayed read.
Twenty repeated studio open/tab/close cycles retained the camera and graph.
At 390x844 the sheet measured 360x354.48 without horizontal page overflow; reduced
motion held the background clock still. Actual custom views at 10 and 40 nodes
showed the corresponding inclusion marks. One 40-node/25-relation sample measured
2.80ms drawing and 22.40ms frame intervals under concurrent host load. This is a
bounded responsiveness check, not sustained 60fps or whole-corpus profiling.

The operator rejected the persistent inclusion diamonds. Inclusion now uses a
subtle warm halo for matched/focus nodes and a quieter cool halo for context.
A bounded explanation appears on hover or keyboard focus, is linked through
`aria-describedby`, and can be dismissed with Escape or a scene gesture. Tooltip
placement is measured on opening only; it follows the existing node transform.
The painter, camera and gesture handlers are unchanged by this presentation fix.
