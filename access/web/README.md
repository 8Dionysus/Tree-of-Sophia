# Tree of Sophia web client

The web client presents the selected Access backend read-only. Browser-local
notes, hypotheses and proposals are provisional; export is their sole handoff.

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
The [human-form delivery contract](../contracts/human-form-delivery.md) owns exact
selection, revision and context requirements.

## Observatory and research space

The published `/` and `/zarathustra/` entries open Zarathustra; `/antiquity/`
opens Antiquity. Both share the constructor's Rust graph, sky, lenses, typed
relations, comparisons and personal branches, plus the public corpus reader and
research workspace. Prepared readings are proposals. A bibliographic pointer
opens an exact reading when it binds to a unit in the selected public version;
unavailable works retain metadata without private-text substitution. The
interface and prepared paths support `ru`, `en`, `es`, `de` and `zh-Hans`.
Language controls in the space and reader
preserve the choice in URLs and local preference; passages retain their edition
language. Entry and profile switching open the space; an explicit reading hash
opens its exact passage.

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

The left rail holds lenses and research actions. A path above the map shows
space › branch or route › move or step › chosen star and lets the reader ascend.
The right column shows the space overview, an open branch top to bottom, or the
chosen star or link; on narrow screens it becomes a sheet over a toolbar. Map
controls sit at the corner. Selection keeps camera orientation and scale; the
camera pans only to reveal a hidden star.

Both spaces use their existing relations and sources
([`space-legibility.mjs`](src/public-research/space-legibility.mjs)). Link kind
uses a word, line pattern and weight; the legend filters kinds and identifies
the disc for source or context, ring for authored proposal and broken ring for
personal record. The panel places stars on the source, reading and comparison
ladder and states each link's footing. Positions group material except in the
measuring lenses. Antiquity's transitions are continuation, qualification,
contrast and analogy; each branch move names its author, work and place.
Zarathustra route steps name their grounds because nodes may belong to multiple
routes. Reverse traversal states atlas links as authored. Search finds branch,
route and step names before prose.

Each stand reads its own [`bonsai/`](../../bonsai/README.md) chronology, places
and lens settings through [`bonsai.mjs`](src/public-research/bonsai.mjs). The
rail separates grouping from Time, Space and Proximity: these measuring lenses
place stars by sourced values in the bonsai.

Time ([`space-timeline.mjs`](src/public-research/space-timeline.mjs)) scales
Antiquity by years. A move uses its passage or work's `historicalDate`, source
and footing:
a date, interval, life phase, or the author's life or years of activity when
the source gives no closer date. Moves sharing a span spread in step order;
each branch is an openable row and each era bracket spans its authors' lives.
The chosen concept traces its stars in scale order; off-scale stars retain the
concept without joining the line. Zarathustra instead orders chapters: a star
uses the first chapter it rests on, each part names its writing years, and stars
grounded only in other texts stay off-scale.

Space ([`space-geography.mjs`](src/public-research/space-geography.mjs)) maps
the bonsai's writing place or the author's home during the dated span, using
Pleiades or Wikidata representative points over Natural Earth coastlines.
Stars gather around shared places; a star with unknown place stays beyond the
map edge.
Proximity ([`space-proximity.mjs`](src/public-research/space-proximity.mjs))
measures from the chosen star by a bonsai criterion: shared concepts or grounds,
link steps, years, chapters or kilometres. Rings carry the scale, direction
groups by branch or circle, and the centre lists the nearest stars.

The grouping lenses show depth ([`space-depth.mjs`](src/public-research/space-depth.mjs)):
source, reading, comparison and personal record from deepest to nearest. A drag
turns the space, coasts on release and stops on any press. Shift, right-button
or two-finger drag pans from the sky or a star; wheel or pinch zooms around the
pinch midpoint. Panel touches belong to the panel. A quick star drag turns the
space; holding a star still picks it up for movement in its plane. The graph
retains its own depth.
Arrows turn the focused or last-pressed map and otherwise scroll; Shift with an
open branch or route walks its steps. The globe shows and controls the turn,
and a press resets it. Named depth planes appear during rotation; at rest stars
retain a slight turn. Time, Space and Proximity stay flat.

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

### The reference

The reference mode reads the same stand as a book
([`reference-book.mjs`](src/public-research/reference-book.mjs)). Its contents
read the tree from the crown to the roots beside a figure of the tree; the index
lists every subject by name and kind; the library holds the eras, authors,
works and places of Antiquity and the parts and chapters of Zarathustra with
the subjects resting on each chapter. The stand's outline
([`reference-outline.mjs`](src/public-research/reference-outline.mjs)) gives
every article its path and the articles before and after it.

An article opens with its kind, name, source, dating and place, its question
and its lead in the stand's own words, beside the place of its star on the
tree; the figure opens the space at that star. The body follows the fields the
stand authored for that kind of subject
([`reference-articles.mjs`](src/public-research/reference-articles.mjs)): a
branch reads as its moves and the turns between them, a move as its reading and
the roles of its reasoning, a passage as its text and the readings resting on
it, a question as the answers of its voices. The stand's words keep the reading
measure and the names of their fields hang in the left margin; the places they
rest on stand in the right margin with their source and first lines and open
whole where they stand. A place the page quotes in full is quoted once. Links
stand grouped by kind in the word, colour and line of the map.

### Public presentation languages

The public adapter's [locale helper](src/public-research/locale.mjs) selects
authored display forms and messages from its per-language catalogs and the
stand-owned Bonsai catalogs for [Spanish](../../bonsai/locales/es.json),
[German](../../bonsai/locales/de.json) and
[Simplified Chinese](../../bonsai/locales/zh-Hans.json). The existing bilingual
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
browser persistence behavior.
