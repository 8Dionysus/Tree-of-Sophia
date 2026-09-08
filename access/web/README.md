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

`/static/fixtures/reader.html` on the development server mounts the same
Observatory with an injected connection and explicitly artificial materials.
It exercises long and multilingual reading, absent descriptions, two-item
comparison, changed snapshots, delayed/offline/restricted reads, and the
40-node/80-relation scene budget. Its short frame sample is a local diagnostic,
not a sustained performance guarantee. Fixtures are not production build entries.

`/static/fixtures/lens.html` also exercises search, capability-bound conditions,
context and returns to paired reading at either 6 or 40 nodes. Its optional
ten-minute probe keeps bounded frame/event histograms and at most 121 samples.
Keep the application and tab visible: the embedded browser can throttle a hidden
task even when page visibility reports otherwise. Save the displayed report
before editing source, because development reloads reset it. The report separates
frame intervals, listener-to-next-rAF callback delay, slow Event Timing entries,
long tasks, DOM size and optional Chromium heap estimates. Panel delay ends at
that callback, not completed paint or remote response. Event counts include
automation; trusted events are counted separately. Heap estimates are not
retained-size measurements or proof of leak freedom. These diagnostics remain
local to the development fixture and are not a production telemetry service.

`dist/` is tracked. Rebuild it from source. Both HTTP and edge adapters load the
stable `/static/assets/tos-graph.js` bootstrap and `tos-graph.css`; imported view
JavaScript and CSS use content hashes. No CDN, inline scripts, iframe, or relaxed
CSP is required.

## Ownership and compatibility

- `src/entry.ts` selects the default Observatory. Existing `mode`/`view` links and
  `workspace=classic` still load `src/main.ts`, with its full research commands.
  The old shell is a compatibility route, not the template for new panels.
- `src/observatory/scene.js` owns camera, selection, sky and
  layout. `gpu-canvas.js` batches the accepted painter through Three 0.185.1;
  Canvas remains the fallback if GPU creation fails.
- `scene-history.mjs` compares the bounded pose and geometry while retaining the
  page-owned response packet by reference. Camera and selection history never
  serializes source text. A newly delivered packet stays a distinct boundary,
  including its packet-local query metadata, even with an equal fingerprint.
  The standalone scene fallback keeps at most 24 live views; the mounted
  Observatory delegates its journey to the persistent history adapter below.
- `knowledge-client.mjs` validates LensResult authority, revision, unique opaque
  IDs, closed relation endpoints, and a 40-node/80-relation display budget.
  A large corpus never directly determines per-frame scene size.
- `data-services.mjs` supplies one page-owned knowledge client and the existing
  query operations to all Observatory consumers. Request cancellation and
  contract checks remain with each consumer. `mountObservatory` accepts this
  connection explicitly; components do not create private API connections.
- `reader-model.mjs` and `reader-panel.mjs` provide **Читать** in the selected
  card and **Моё пространство → Инструменты → Чтение и сопоставление**.
  At most two inspected objects or relations remain in this browser page,
  alongside their exact identities, versions, source references and supplied
  qualifications. Wide windows show independent columns; narrow windows use
  keyboard-accessible tabs. Reading positions and return-to-place bookmarks
  survive panel handoffs. `reading-resume.mjs` persists only exact references,
  preferred language/form, active item, and up to eight bounded positions per
  item, scoped to the mounted pathname. Paragraph keys and offsets carry no
  source text and are valid only for the exact source/content revisions.
  Reopening fetches both materials from the current backend; a changed revision
  starts the affected reading at the beginning with a visible notice. Full
  response copies and graph return bookmarks remain page-local. Damaged storage
  and unseen competing-tab changes are not overwritten; save failures are shown.
  An explicit refresh obtains a new snapshot; mismatched scene actions stay
  disabled. Network failure retains a labelled earlier copy, while an observed
  403/404/410 removes its reading copy. Available language/form fields retain
  their delivered wording and identify fallback or unspecified language.
  This compares supplied material side by side; it does not generate semantic
  conclusions, full text, translations, or stable corpus text addresses.
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
- `lens-conditions.mjs` and `lens-condition-editor.mjs` add up to 12 conjunctive
  conditions each for selector roots and relations. Named display/status fields
  require catalog and schema support. Registered semantic properties additionally
  require the `property_id` capability and schema selector; the UI sends that
  identity unchanged and never substitutes its advertised internal field path.
  Operations are the intersection of the property's declared operations, the
  catalog value contracts, the LensSpec schema and supported value controls.
  Text, numbers, booleans and lists retain their types; no implicit language,
  unit or Unicode conversion is performed. Property definition and applicability
  remain visible. Missing properties or operations block execution until the
  condition is repaired or removed. Center-mode node conditions and disabled
  relation conditions remain explicitly dormant in the definition.
- The constructor separates edited conditions from the query of the displayed
  packet. Empty results offer explicit scope/search/condition adjustments and
  retain the previous graph; errors do the same. Recorded inclusion distinguishes
  selector matches from traversal context and names delivered traversal witnesses.
  A contextual button reopens the current lens, and previous/original view
  actions preserve scene history, camera and the independent reading shelf.
  Local definitions and links now use v2; v1 definitions migrate on read. Older
  clients reject v2 instead of silently losing the new conditions.
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
  deep link takes precedence. Named places support renaming, update, removal and immediate
  undo. Storage failures remain visible and do not replace a damaged place list.
- `travel-model.mjs` and `travel-panel.mjs` add **Назад / Вперёд / История**,
  including Alt+Left/Right outside text fields and direct jumps to named steps.
  The list and cursor survive reload in local browser storage, scoped to the
  mounted pathname. A new material, area or lens after going back starts a new
  branch. Camera movement, card sections and refreshed data update the current
  stop while preserving its identity and the forward branch. Consecutive old
  camera-only rows collapse on load or import, preserving the current pose;
  explicitly saved places are independent and remain intact. Step names identify
  the material or lens. Persistence retains at most 100
  steps and 1.2 million JSON characters (older entries are trimmed first).
  Only bounded place requests, identities and poses are stored, never source
  packets or exploration cursors. Every jump reloads current owner data before
  moving the cursor. Failure, cancellation and late replies preserve the current
  view. Changed revisions and unavailable selections are reported. Storage
  failures keep navigation usable in this page; malformed or competing-tab
  records are not overwritten. The history panel offers retry and explicit
  clearing, which retains the current view. Clearing browser data clears history.
  **В места** pins any history step without navigating away, retaining its own
  query and pose. Repeated pinning retains the existing named place. Names can
  then be edited in **Моё пространство → Места**.
- `settings-panel.mjs` adds a permanent **Настройки** button outside the scrolling
  tool strip. Existing text, star-label, docking, pinned-tool and window-size
  preferences live here, alongside interface language, theme and action controls.
  Dragging can rotate or pan; Shift temporarily reverses that choice. Scrolling
  defaults to automatic action selection: smooth pixel gestures pan, notched
  deltas zoom, and pinch zooms. Browsers do not expose device identity, so smooth
  wheels can use an explicit pan/zoom override. Sensitivity has three levels.
  Older mouse preferences preserve their zoom choice; other old records gain
  automatic defaults. Ambient motion remains a persistent footer control.
  Resetting appearance and controls leaves the interface language, places,
  notes, history and reading intact.
  The panel also links to history, the full workspace copy and last-view reset.
- `ui-i18n.mjs` and `ui-catalog.mjs` provide Russian, English and Spanish interface
  text. Switching language updates explicitly marked UI text and attributes in
  place, preserving nested controls, focus and unsaved form contents. Raw source
  titles, descriptions, vocabulary labels and user notes remain opaque even when
  their wording matches an interface message. Each reading item keeps its own
  existing choice among delivered language/form variants; interface language
  never requests or invents a translated source. Both language and theme persist
  in the local preferences and workspace copy.
- `themes.css` adds a light interface with warm paper surfaces and dark text for
  panels, controls and contextual hints. The space and stars retain their night
  palette. The existing dark theme remains the default.
- `panel-geometry.mjs` gives every popup, including search, lenses and the selected
  card, a draggable header and eight resize edges/corners. Arrow keys on the move
  handle move the window; the corner handle changes its size. Home resets the
  respective geometry. Sizes and normalized positions persist locally, adapt to
  available viewport space and travel with the workspace copy. Window gestures
  do not change the camera, graph selection, source data or navigation history.
- `context-hints.mjs` provides one bounded contextual tooltip for controls,
  stars and relations. Hover waits 400 ms; keyboard focus reveals the same
  explanation, Escape dismisses it before the enclosing panel, and gestures
  hide pending/visible hints. It preserves focus and selection, stays inside
  the viewport, and can be hovered for reading. Important control instructions
  remain in settings on touch devices; a star's inclusion explanation is also
  available in its card under **Почему звезда в этой области**. Inclusion
  describes query execution, never philosophical authority.
- `graph-preview.mjs` builds compact star hints from every currently supplied kind
  and directed relationship label, without a browser-owned role allowlist.
  Authored/source-derived summaries take precedence; otherwise up to two visible
  relationships provide context. Each field is bounded. Synthesized technical
  metadata is not substituted for a substantive description. Canvas relationships
  highlight on hover and open their existing owner-backed card on click. One
  keyboard entry cycles visible relationships with arrows and opens with Enter;
  it avoids adding a button for every edge to the tab order.
- `workspace-copy.mjs` and `workspace-copy-panel.mjs` provide **Моё пространство
  → Сохранить и перенести исследование** and **Исследование → Полная копия
  исследования**, also available in **Настройки → Локальные данные**. The versioned local JSON copy includes history and cursor,
  named places and lens definitions, last view, interface preferences, reading references/positions,
  and saved research records, hypotheses, proposals and route comparisons.
  It excludes delivered source packets, reading text, page-local response
  caches, exploration cursors and unsaved form input. Existing record-only
  export/import and the WebMCP research packet keep their narrower contracts.
  Import checks every owner section and the whole file limit, previews counts,
  offers a download of the previous copy, and requires an explicit replace.
  Changed storage since the preview aborts replacement. A failed storage write
  rolls back completed writes; a rollback failure remains visible. Successful
  import suspends old page writers and reopens the page through the normal
  owner readers, fetching current backend material. LocalStorage writes across
  sections are synchronous with rollback, not a crash-atomic database transaction.
- `interface-model.mjs` restricts composition to the registered local adapters.
  Search, lenses, research, navigation, lens constructor, evidence and sources can
  be pinned and reordered; all remain available in the tool list. Preferences
  contain no executable code, service endpoints or additional write authority.
  Existing WebMCP and backend action contracts remain the execution boundary.

Camera changes include the validated saved-view bridge, initial-load sequencing,
card-section/read-position restoration and configurable action routing. Projection
and GPU painting remain the baseline; normal sensitivity retains existing gains. New
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

The subsequent interface refinement passed 132 frontend tests, TypeScript,
Vite build, and the standalone access lane (110 tests plus source-profile
validation). Browser checks covered RU/EN/ES switching with saved notes and an
unsaved draft intact, persistent window geometry, explicit and automatic scroll
actions, Shift drag routing, edge hover/click/keyboard access, and responsive
390x844 windows without horizontal overflow. Light-theme reading controls and
the mobile reading header were checked after contrast/layout corrections. The
built preview loaded ten real ToS objects with the new assets and no sampled
console warnings/errors. Physical touch hardware, broad performance profiling,
CI, deployment and integration with the future backend remain separate checks.

### Complete human form delivery

`KnowledgeClient.readMaterial` reads an isolated, validated, full `LensResult`
with a content-language preference and zero traversal depth. The visible graph
does not consume this separate result. Restoring a relation may first use the
existing inspect route to discover its endpoints; a single full lens response
then owns the displayed record and endpoints at the checked revision. There is
no new backend endpoint or invented inspect response schema.

`human-forms.mjs` checks the delivered selection, exact form references, material
revision, language declaration, mandatory context and authority flags. It does
not select among candidates, materialize source forms or perform assessment.
The canonical ToS human-form materialization and access selection contracts
remain the owners; this isolated UI branch does not copy their implementation.

Inspector and pinned reading share `human-forms-view.mjs`. All seven roles are
available: name, caption, hover, statement, grounds, history and technical.
Ready wording and every mandatory context value are one complete scrolling
unit. Unknown context fields, bindings, false, zero, null and empty values remain
intact. Provenance, form identity and source-snapshot assessment details stay
separate from semantic acceptance. Role-specific diagnostics do not attribute
another role's stale or restricted candidate to the selected role. Ambiguity,
missing delivery, unavailable forms and delivery limits remain explicit.

Small graph hints refer to the full card instead of truncating a human form.
Legacy display fields retain their existing navigation role. Content-language
choices query the backend; fallback reports the actual delivered language.
Interface localization does not translate source wording or mandatory context.

Reading stores references and positions only. The position key includes the
source and material revisions, requested language and exact selected form
identities, states and actual languages. Earlier four-part position keys remain
readable. Reload fetches current packets; invalid or revoked material clears
the displayed copy. Late language responses cannot replace a newer choice.
Network failure can retain an explicitly marked earlier in-page copy.
Only the shared, bounded reading-anchor grammar leaves page memory. A local
diagnostic anchor such as a candidate without a role exports as `anchor: null`,
retaining its numeric position and allowed details state. The importer still
rejects unknown anchors; one diagnostic cannot invalidate the saved pair.

The inspector follows `scene.compact.claim_paths`. A selected Claim uses
`KnowledgeClient.readClaimMaterial` to fetch the exact path nodes, both legs,
declared evidence relations and their endpoints as one full LensResult at the
captured revision and requested language. Exact selectors with zero traversal
need no focus seed; a focus can suppress a path when it also supplies grounds.
Every returned identity and content revision is checked. The new response owns
the wording pointer, entire form packet, semantic, epistemic and relation
context; it is never combined with wording from the older displayed scene.
The current request must still own the same scene, selection and language
before the card becomes ready. Loading, damaged or denied delivery clears the
earlier wording. Ordinary node reads remain isolated to one node.

This supports the existing two-leg Claim path with `claim-supported-by` details.
New path-detail relation kinds require an explicit compatible owner contract;
they fail closed here. Visual path collapsing, explicit candidate selection and
an over-budget recovery endpoint are outside this UI slice. The painter, camera,
gestures, motion and panel placement retain their existing owners.

`fixtures/human-forms.html` provides synthetic full, ambiguous, stale,
over-budget, damaged-context, delayed and restricted deliveries. Its optional
preservation probe reports reading references and positions without exporting
source text. These examples are test data, not ToS knowledge or assessment.
The `?forms=compact` and `?forms=diagnostic` variants exercise the real inspector
call path and pair restoration with a role-less diagnostic respectively.

This slice passed 149 frontend tests, TypeScript and the Vite build. The access
lane passed 109 tests in its full run; its remaining schema test passed after
an unnecessary schema copy was removed. Source-profile validation passed.
Browser checks covered all seven roles, role-specific states, damaged-context
rejection, late-response cancellation, a 390 px viewport and two-material
reload with separate languages and scroll positions. The built UI also read
a real Duden Claim with its complete qualifications and context, an explicit
Spanish-to-Russian fallback and unavailable undeclared original forms. Vite
reports the application chunk above its 900 kB warning threshold (905.15 kB).
This is local delivery evidence; CI, merge, deployment, backend assessment,
physical touch and performance acceptance remain separate.

The subsequent compact-reading and persistence repair passed 163 frontend tests,
TypeScript and Vite. Its browser fixtures verified distinct RU/EN packets,
late-response suppression, unavailable original, missing context, access denial,
and a saved diagnostic position restored for both materials after reload.
The real canonical HTTP canary exercised the actual inspector consumer with
four Duden Claim nodes and three context relations. EN and ES requests preserved
the backend's RU fallback; undeclared original had no wording. The ordinary
HTTP client canary also passed focus, search, exploration, evidence, paths and
known/restored relation equality. The application chunk warning remains
(908.78 kB, 255.86 kB gzip). Full access and release lanes were not repeated for
this UI-only repair; the affected frontend and real HTTP consumer checks provide
its bounded validation evidence.
