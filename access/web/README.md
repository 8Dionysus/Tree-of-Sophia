# Tree of Sophia web client

The web client is a source-read-only presentation of the selected Access
backend. It does not author ToS meaning, assess claims, change rights or review,
or write to canon. Browser-local notes, hypotheses and proposals remain
explicitly provisional; export is their only handoff.

## Presentation contract

Shared renderers under `src/observatory/` present validated backend packets.
Interface labels follow the selected UI language. Source titles, quotations,
form wording and vocabulary labels keep their delivered wording; the UI does
not translate them. Missing language variants show their actual fallback.

Readable context keeps source qualifications, exact number lexemes and field
order. Recognized fields receive interface labels, unknown fields remain
available as additional information and in the exact context disclosure, and
missing required context stays visible as a gap. IDs, hashes, paths, execution
traces and routine transport state are kept out of ordinary reading panels.
These presentation checks do not establish the correctness of an owner's
semantic assessment or rights decision. The
[human-form delivery contract](../contracts/human-form-delivery.md) owns exact
selection, revision and context requirements.

## Observatory and research space

The default entry is the Observatory: a full-window star field with bounded
knowledge neighborhoods and floating search, source and research panels. The
separate `/static/research.html` entry provides the cosmic research interface;
both use the same knowledge client, source reader, HumanForms and personal
shelf. A missing or unavailable backend capability remains explicit. The demo
constructor is a development entry, not a public data source.

The main interactions are:

- Search and inspect source-linked records. Service/projection objects can be
  hidden from the research view without changing the retained packet.
- Read one material or compare up to two exact objects or relations. Reader
  positions bind to the delivered source/content revisions, language and form;
  a changed revision does not silently reattach a position.
- Build a bounded lens from catalog vocabulary and path conditions. Preview
  does not change the graph until applied. Definitions and links recompile
  against current data; they are not frozen research snapshots.
- Continue a bounded neighborhood exploration. Each Exploration area is at
  most 200 nodes, 600 relations and 4 MiB; LensResults remain at most 40 nodes
  and 80 relations. Page counts describe the selected traversal, not corpus
  totals.
- Keep a local research workspace with notes, exclusions, route comparisons,
  hypotheses and typed proposals. These records stay separate from ToS source,
  review ledgers, candidate intake and canon.

The personal shelf stores exact material/form/text/lens/route references and
notes in browser storage; it does not copy source text or transport handles.
The notebook, shelf, reading-position and last-view stores have distinct
lifecycles. Refresh fetches current owner data. An observed access revocation
clears the displayed copy; a network failure can retain only a visibly marked
earlier in-page copy. Storage failures are shown, and damaged or competing-tab
state is not silently overwritten.

Only the most recent scene/query context and exact selection are retained in
page memory. At most two previous areas can keep their bounded result and
camera in memory. Browser history and named places store bounded IDs, query
state and poses, not source packets or exploration cursors. The full local
workspace export/import has a separate explicit replace flow; see the
[research shelf guide](src/research-shelf/README.md).

## WebMCP

When the browser provides `document.modelContext`, WebMCP registers actions for
the live page, including view, search, selection, focus, cancellation and
available research tools. Selection-bound actions carry the captured context
revision and fail closed after the human changes selection. The page remains
usable without WebMCP. It embeds no model, does not call an AI API, and does
not require a separate MCP connection for the browser-native path. WebMCP is
an experimental browser surface, not an authority or availability guarantee.
The native `tos mcp` process remains an optional off-page access route.

## Development and verification

From this directory:

```sh
npm ci
npm run typecheck
npm test
npm run build
npm run dev
```

`npm run dev` proxies `/api` to the local backend on `127.0.0.1:44258`;
`TOS_DEV_API_TARGET` selects another local target. Production uses same-origin
APIs. `dist/` is generated output and must be rebuilt from source. The
standalone validation route is documented in the repository's
[`VALIDATION.md`](../../VALIDATION.md); local browser checks, CI, release and
deployment remain separate evidence.

Development fixtures are served under `/static/fixtures/`. The reader fixture
uses artificial materials to exercise multilingual text, gaps, access expiry,
and bounded scene behavior. The lens fixture checks bounded lens and reading
flows. Fixtures and local timing probes are not production data, sustained
performance guarantees or deployment acceptance. Keep the app visible when
measuring interaction; hidden browser tabs may be throttled.

Browser asset output uses a stable bootstrap at `/static/assets/tos-graph.js`
and `tos-graph.css`; imported JS/CSS use content hashes. The build does not
require a CDN, inline script, iframe or relaxed CSP.

## Owner guides

| Area | Maintained detail |
| --- | --- |
| Native corpus reading | [`src/corpus-reader/README.md`](src/corpus-reader/README.md), [data contract](src/corpus-reader/DATA-CONTRACT.md), [storage](src/corpus-reader/STORAGE.md), [native multi-span requirements](src/corpus-reader/NATIVE-MULTISPAN-REQUIREMENTS.md) |
| Reader interaction | [`src/reader/README.md`](src/reader/README.md) |
| Personal shelf and notebook | [`src/research-shelf/README.md`](src/research-shelf/README.md) |
| Lens constructor and demo | [`constructor/README.md`](constructor/README.md) |
| Native/browser API and authority | [Access guide](../README.md), [contracts](../contracts/README.md) |

The `software_browser` validation lane covers the shipped entry and selected
browser persistence behavior. It does not prove physical gesture feel,
cross-device account sync, whole-corpus performance, semantic acceptance, or
production deployment.
