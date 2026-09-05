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
  and explicit retry. A failed request retains the current graph.
- `workspace.mjs` rebuilds source dossiers, notes, hypotheses, proposals, source
  gaps and source-bound word-analysis preparation in floating panels. It reuses
  `query-operations.ts` and `research-workspace.ts`; there is no second backend.
  Unavailable source material is shown as unavailable, not reconstructed.
- `research-actions.ts` validates a complete proposal before changing the local
  journal. Existing workspace storage and export packets remain compatible.
- `webmcp.ts` can restrict registration to commands implemented by the active
  shell. IDs, source references and deep links are returned intact. Legacy
  path comparison, exclusions and Evidence Lens remain on compatibility routes;
  their replacement in the Observatory is still a separate UI slice.

The Observatory requires the knowledge/lens API prepared in the backend lane.
The base commit `61cc594c` does not include it. Land and verify that backend before
making this default client public. This branch was checked against the backend
lane's local source, not a production Worker deployment.

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
