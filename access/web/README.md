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
The public profiles provide Russian, English, Spanish, German and Simplified
Chinese display names for the selected works; source metadata and quoted text
remain unchanged.

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

The published `/` and `/zarathustra/` entries open the Zarathustra research
space; `/antiquity/` opens the Antiquity space. Both reuse the constructor's
Rust graph model, live sky, lenses, typed relations, comparisons and personal
branches alongside the shared public corpus reader and research workspace.
Prepared readings remain proposals. A bibliographic pointer becomes an exact
reading action only when it binds to a unit in the selected public version.
Unavailable works retain their metadata without substituting private text.
The interface and prepared paths support Russian, English, Spanish, German and
Simplified Chinese (`ru`, `en`, `es`, `de`, `zh-Hans`). The language control is
available in the space and the reader; URLs and the local preference retain the
choice across profiles. Source passages
keep their edition language. Plain entry and profile switching open the space;
an explicit reading hash opens its exact passage.

The shared Library also exposes the retained research collection: all fifteen
Antiquity branches, author and era browsing, work maps, close-reading prompts,
argument readings and lexical dossiers, with presentation in all five languages.
The collection lives in the stand's bonsai
([`bonsai/antiquity/collection.json`](../../bonsai/antiquity/collection.json)); it
records its authored source revision and excludes witness text and quotation
anchors. Published section entries use the separate
provider structure; they are not exact word-span alignments. The readable corpus
selection remains separate from this larger bibliographic collection.
Zarathustra's source pointers and independently grounded alternatives use the
same reading actions. Extending the Antiquity graph copies its explicitly
compatible predecessor through the current Rust validator and retains the
original namespace; browsing a route does not fill the personal graph.

The space follows the tree's own rule: tree for orientation, graph for
relation, source for authority. The left rail holds lenses and research actions;
a path line above the map names where the reader stands (space › branch or
route › move or step › chosen star) and each level climbs back up; the reading
column on the right holds the space's overview with its branches, an open branch
read top to bottom, or the chosen star or link. On a narrow screen the column is
a sheet over a tool bar. Map controls sit at the map's corner; the camera never
turns or refits on a selection and pans only to bring a hidden star into view.

Both spaces name what their data already says, without adding relations or
sources ([`space-legibility.mjs`](src/public-research/space-legibility.mjs)).
A link's kind is a word, a line pattern and a weight, not a hue alone; the
legend lists the kinds, filters by them and says what a star's form means: a
disc for a source or its context, a ring for an authored proposal, a broken
ring for a personal record. The panel places each star on the ladder of
interpretation (source, reading, comparison) and says on what footing a link
stands; star positions group material and measure nothing, except in the
measuring lenses below. Antiquity restores its four authored transitions (continuation,
qualification, contrast, analogy) and walks branches by moves, each named by
author, work and place. Zarathustra
walks routes by steps; a step names the places of its own grounds, since its
node is a concept or thinker shared by other routes. A route that walks an
atlas link against its direction states the link as authored. Search finds
branches, routes and steps by their own names before their prose.

Each stand reads its own bonsai — the mini-Tree under [`bonsai/`](../../bonsai/README.md)
that keeps its chronology, places and lens settings in the Tree's shapes —
through [`bonsai.mjs`](src/public-research/bonsai.mjs). The rail sets the lenses
that measure apart from those that group: Time, Space and Proximity place a star
only by a value its bonsai states, with that value's source.

Time ([`space-timeline.mjs`](src/public-research/space-timeline.mjs)) is a scale
of years in Antiquity. A move stands within the span its bonsai dates the
writing of its passage or work — a date, an interval, a phase of a life, or the
author's life or years of activity where no source dates it closer — each a
Tree `historicalDate` with its source and footing. Moves sharing one span in a
branch spread over it in step order; each branch is a row whose name opens it;
an era's bracket spans its authors' lives. A concept chosen above the panel is
traced through the stars that carry it in the order of the scale; a star off
the scale carries it without joining the line. In Zarathustra the horizontal is the
order of the book's chapters: a star stands at the first chapter it rests on,
each part names the years it was written, and stars grounded only in other texts
stand outside the scale.

Space ([`space-geography.mjs`](src/public-research/space-geography.mjs)) is a map:
a star stands where its bonsai says its text was written, or where its author
lived in the dated span, at the gazetteer's representative point (Pleiades,
Wikidata), over Natural Earth coastlines; stars at one place gather round it,
and a star whose place the sources leave open stands apart past the map's edge.
Proximity ([`space-proximity.mjs`](src/public-research/space-proximity.mjs))
measures from the chosen star by one criterion the bonsai names — shared
concepts or grounds, steps along links, years, chapters or kilometres — in
rings that carry the scale; the direction groups by branch or circle, and the
centre lists its nearest stars.

On the lenses that group, the map is a space read in depth
([`space-depth.mjs`](src/public-research/space-depth.mjs)) by the steps its
bonsai declares: a source lies deepest, a reading before it, a comparison of
readings before that, a personal record nearest. A drag turns the space and a
turn let go while moving coasts to rest; any press stops it. Shift, the right
button or two fingers move it, from the sky or from a star; the wheel or a pinch
zooms, a pinch about the point between the fingers. Fingers on a panel belong
to the panel. A quick drag from a star turns the space too, while a star held
still is picked up and moves within its plane; depth is a view, so the graph
keeps the star's own depth. Arrows turn the space while the map has focus or
was pressed last, and otherwise scroll as usual (with Shift while a branch or
route is open, whose steps the arrows walk); the globe among the map tools shows
the turn, turns the space under a drag and, pressed, brings it back to rest.
While it turns, the planes of the steps appear, named; at rest the space keeps
only its stars, a little turned so that its depth shows. Time, Space and
Proximity stay flat.

Nothing on the stand explains itself in standing text. One hint
([`space-hints.mjs`](src/public-research/space-hints.mjs)) answers a pointer
that rests on something, or a keyboard that reaches it: a tool says what it does
and its key, a star its kind, step, date and place, a link the meaning of its
kind; Escape closes it first. The gestures, which nothing on the screen can
show, appear once in a controls window
([`space-controls.mjs`](src/public-research/space-controls.mjs)) after the
opening turn, for a mouse or for fingers. "Don't show again" is kept in this
browser; without it the window returns on the next visit, it reopens from the
map tools, and where the browser keeps nothing it does not insist.

The space is meant to be played the way a knowledge game is played, with no
score and nothing hidden ([`space-play.mjs`](src/public-research/space-play.mjs)):
what grows is the reader's own understanding. Where a branch or route step asks
a question, and at a riddle (a question star), the reader may write one line of
their own before the readings open; it becomes a personal record that rises out
of the star it is about to the near plane. The readings then unfold; setting
the record beside the reading it stands nearest moves it there and shows what
that reading rests on (its grounds in the source) and what disputes it. From a
chosen star on the whole tree, threads lead into the dark toward stars the space
does not hold yet; following one brings that star in. A branch or route card
lights its way through the map while it is pointed at. The records stay the
reader's own proposals in the research workspace, never readings of the Tree.

Each stand loads only its own bonsai; Zarathustra takes the shared library's
Antiquity entries in a quiet moment after its first view, or when the library is
opened, and Antiquity never loads Zarathustra's prepared atlas.

A chosen star states when (◷) and where (⌖) its text was written, and on
Proximity how near it stands (⊙); each fact opens to its statement and sources.
Measure and completeness ([`space-measure.mjs`](src/public-research/space-measure.mjs))
counts what the bonsai dates and places, by kind, precision (a dating within a
phase or a life no firmer than that span), footing and era, and draws how each
branch or route walks time or the book. These positions are
display only and never enter the saved graph. Zoom and pan follow the scale, an
open branch or route reads top to bottom on Time and Space, and `treeLens`,
`treeCriterion` and `treeConcept` link to a view.

The scene uses Canvas2D when only a software WebGL renderer is available;
the same graph and actions remain available. Browser-local graph exports and
the complete research transfer preserve personal work without source admission.

The complete research export contains saved notebook, workspace, shelf and graph
records. An open thought form has its own guarded export/import. The temporary
exact-source basket survives profile changes in the same tab; saving a thought
or task includes its references in the corresponding saved owner. Import previews
name conflicts and add only available records. Question-history groups retain
every editing snapshot; restoring one archives the current saved question.

New external-agent tasks use `tos.public_research.agent_task.v2`. Their sources
carry the unchanged quote and exact reference, a bounded surrounding context,
`contextRange` and verified `editorialAnnotations`. Ranges are half-open Unicode
code-point offsets in the exact version/unit; annotations do not claim a complete
critical apparatus. Saved v1 tasks and v1 responses remain readable against their
original task IDs. Changed questions or workspace revisions require a new task.
Nested objections can be selected separately; the original received response is
retained locally even when an objection is omitted from application. The strict
packet shapes and bounds live in [`agent-bridge.mjs`](src/public-research/agent-bridge.mjs).

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

### Public presentation languages

The public adapter's [locale helper](src/public-research/locale.mjs) selects
authored display forms and messages from its per-language catalogs and the
stand-owned [Bonsai catalogs](../../bonsai/locales/). The existing bilingual
library is constructed and fingerprinted before display translation; choosing
a language preserves graph namespaces, material IDs, edition revisions and
exact source addresses. Plain source strings and personal text remain literal.
The reader's language control changes its interface while keeping the selected
edition and passage. A language of the interface does not imply an edition in
that language. New Zarathustra ground actions keep the existing German default
for Russian presentation and use German for German presentation; other interface
languages use the English fallback. Explicit reading links and an already open
edition keep their exact addresses when the interface changes.

Catalog keys retain the exact English presentation message. A context key is
used where the same word has different roles, such as `corpus-reader:Reading`
for reading a book rather than an interpretative reading. Translation checks
compare language coverage, placeholders and the authored Bonsai display forms.

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
