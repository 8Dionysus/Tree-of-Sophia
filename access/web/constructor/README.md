# Tree of Sophia — a philosophical walking demo

This local access entry opens an already connected philosophical tree. Sources,
images, concepts, interpretations, questions and comparisons occupy six
constellations. The operator can explore its meaning immediately, open a
prepared investigation, grow a complete new branch, or author a personal node
and relation. No initial manual assembly is required.

The scene retains the observatory's GPU atmosphere, rotation, pan, zoom,
selection and recording controls. It is a separate entry from the existing
observatory and Foundation API consumer.

## Live source exploration

`constructor.html?live=1` selects the real, read-only exploration adapter before
the demo library or personal workspace is loaded. It mounts the same owner sky,
camera and gestures. It neither imports demo knowledge into ToS nor modifies the
demo's saved state. Optional `focus=<exact API ID>&kind=node|relation&lang=ru|en`
opens an addressable origin; without a focus, search is the entry point.

Run `node node_modules/vite/bin/vite.js --config vite.constructor.config.mjs`
from `access/web`. The local server uses port 44257 and proxies `/api` to the
separately selected read-only backend at 127.0.0.1:44258. Its discovery must offer
exploration v2 and indexed or compressed search; there is no fixture fallback.
`TOS_WEB_CACHE_DIR` can place the development cache in owner-managed scratch.
This does not install, restart or activate the desktop demo.

`live-research.mjs` connects the bounded `ExplorationSession` to the sky through
`live-model.mjs`. Search returns at most six items of each kind per page. Exact
node/relation origins and continuation enter the existing immutable scene cache;
failed queries retain the last good space. Selection, positions and camera
survive continuation and local presentation/language changes. Only the first
opening, explicit new field or fit action frames the camera. Source and snapshot
changes require the existing explicit replacement boundary.

The three presentation modes expose compact paths, grouped carriers or raw
records; they do not invent semantic lenses. Expansion conditions come from
the catalog and apply to the next query, not retroactively to retained areas.
Reading uses complete version-checked forms and mandatory context. Comparison
retains at most two exact cards; independent cancellation and visible terminal
errors prevent stale or indefinitely loading cards. Neither comparison nor
source availability establishes semantic acceptance.

This entry currently implements reading, not source commands or a complete
research constructor. Path conditions, semantic comparison operations, direct
local-source opening and authorized growth still need joint integration.
Source references and existing human-form gaps remain visible. Unit checks in
`live-research.test.mjs`, `live-model.test.mjs` and
`../src/observatory/exploration-session.test.mjs` protect this adapter; real
browser and full-corpus checks remain necessary before foundation acceptance.

Search can seek past empty backend pages, stopping at the first result or
completion. One automatic window admits at most eight requests and stops
between pages after 1 MiB of re-encoded JSON or two seconds. These last two
values are thresholds, not hard transfer or wall-clock caps: an in-flight
bounded request may finish later or cross the byte threshold. Its valid result
is retained. A paused window exposes explicit continuation; changing the query
or closing search cancels stale work without changing the scene.

When the selected material supplies an exact `source_dossier_ref`, Sources can
open the existing metadata-only `/api/source/dossiers/{handle}?limit=64` route.
The dossier retains identity, rights posture, bounded chain and source refs.
It has no source/content revision binding, so the UI does not attribute the
card's version to it. This is not source-byte delivery or permission to use a
carrier. Ordinary local paths are not interpreted as dossier handles or file
URLs; exact local-source reading still requires the source owner's safe ABI.

## What the mockup contains

`atlas-data.mjs` owns prepared **demonstration material** and its explanatory
relations. The operator explicitly requested logically generated material to
make the product idea visible. These are authored interpretations, paraphrases,
questions and comparisons, not admitted ToS nodes, quotations or canon. The
fixture does not change the source corpus, accepted graph or review history.

A bounded selection of existing source materials can come from the private
`library.json` built by `build-library.py`. The exact DE/RU text, source addresses,
speaker context and three prepared English demo translations are retained.
Other archival passages remain available to the library builder but are not
used as the mockup's main reading surface. Source text is not committed here.

For a demo intended to be recorded, `build-fragments.py` prepares a separate
reading library from explicitly selected editions and reviewed text packets.
It retains the navigation identities while excluding the older private text
and archival records. [FRAGMENTS.md](FRAGMENTS.md) describes the complete-unit
reader, its build inputs and the distinction between text checks and rights
assessment. A private-library build alone is not a recording edition.

**Reading / Чтение** opens the [research reader](../src/reader/README.md).
It provides a fragment library, text search, independent translation columns,
reading preferences, version-specific bookmarks and a personal notebook.
Paragraph notes can become source-linked personal nodes in the local tree.
Reader state is stored separately and remains tied to exact text versions.
Changes to the authored fixture create a new graph/route namespace; they do
not overwrite the previous personal tree or silently rebind its references.

The authored fixture supplies readable Russian and English content for every
prepared thought and relation. External source links support contextual
comparisons; their presence does not convert those comparisons into source
claims or a proven historical genealogy. The reader's Origins tab exposes the
difference between source material and mockup interpretation.

## Following a question

`atlas-content.mjs` develops each prepared material with a substantial reading,
alternative perspectives, concrete examples and questions where useful. Source
introductions are stored separately from exact passages. The content remains
an authored demonstration; `SEMANTIC_REVIEW.md` records its interpretive limits
and the primary passages consulted for named comparisons.

The two `inquiry-*.mjs` material modules develop the reading with explicit
textual grounds. `source-references.mjs` gives each reference an author, work
and precise locator; each use also explains what the passage contributes.
Questions, exercises and separately grounded alternative readings are optional.
`relation-inquiry.mjs` explains the actual connection between passages, with a
question or limitation where useful. Source details open beside the reading;
available units open in the reader, while other works lead to external texts
or edition pages. A source node's exact text and provenance remain separate.
`inquiry-layer.mjs` checks coverage, references and bilingual fields, including
the independent grounds of an optional alternative. Available reader anchors
come from those references. These mechanics cannot decide whether the passages
warrant the interpretation; that requires the review recorded in
`SEMANTIC_REVIEW.md`.

`inquiry-foundation.mjs` supplies the central investigation and six lens
methods, including what each method leaves out. Open **How to investigate this
question** below the current view's title. In a walk this opens its starting
assumption, stakes and next inquiry. [SEMANTIC_FOUNDATION.md](SEMANTIC_FOUNDATION.md)
explains the demonstration's philosophical organization.

`close-reading-guides.mjs` offers targeted reading moves in the 13 available
passages. Open **Research → Relations** in the reader, expand a move and jump
to its paragraph in the selected RU or EN version. These local positions are
bound to an exact catalog digest; they are neither corpus units nor paragraph
alignment. The two link-only works keep their existing access boundary.

`atlas-routes.mjs` offers eight guided walks through actual relations, with
58 stops and 50 explained transitions. Each route has
an opening question, ordered stops, an explanation of every transition and an
open conclusion. The first route compares Zarathustra’s willing of a fate
with Camus’s joining of deeds through memory, then follows the creating will’s
unfinished response to the past in “On Redemption”.

Choose **Routes**, open a question and use **Back / Next**. Numbered stars and
the step strip follow the same sequence. The transition panel can open its
exact graph relation without losing the current step. The last stop gathers
the question rather than marking an interpretation as accepted. Returning to
the whole tree leaves a saved place; **Continue the walk** restores it after a
reload or application launch in the same profile. A completed walk also
retains its conclusion. Route state is separate from the editable graph.

The guided view displays only its named transitions, retaining their original
direction. A walk may traverse a relation in reverse without reversing the
claim. Starting a walk adds any missing prepared nodes and relations in one
undoable graph operation. Removing a required item interrupts the current view
and leaves a return point that can explicitly restore the walk.

## Looking and building

Six lenses expose different parts of the same saved tree:

- **The tree** shows the prepared constellations together.
- **Meanings** follows support, development, interpretation and challenge.
- **Sources** follows textual containment and source-linked interpretation.
- **Tensions** exposes questions and competing readings.
- **Images** follows symbols and their conceptual neighbors.
- **Resonances** compares thinkers and traditions across branches.

Each relationship has a type, a readable label and an explanation. Colors and
line styles distinguish the types; select a line or its label to read it.
Relationship filters affect the view, not the saved graph. A prepared collection
focuses an existing investigation, and Grow a branch adds a complete prepared
extension in one undoable action.

Three explicit operations connect two selected thoughts: draw a new explained
relation, traverse an actual existing path, or compare both cards side by side.
The path follows graph adjacency in either direction and preserves each
relation's original direction. It is not a newly inferred claim.

User thoughts can be created, edited, moved and removed. Developing a prepared
thought creates an editable personal interpretation linked to its starting
point. Prepared source identities remain library-owned.

## Persistence and limits

The constructor model retains a bounded graph in browser storage, with 64 undo
steps, 200 nodes, 600 relations and a 1 MB workspace limit. Imports bind to the
exact combined source-library and fixture fingerprint. A changed fixture uses
a new storage key; it does not silently replace a saved personal tree.

Collections and extensions merge atomically, retain personal edits, and fail
without partial changes at capacity. View lenses do not mutate saved graph
state. Node offsets remain available when another arrangement is chosen.
Invalid stored data is protected from automatic overwrite. Download and import
preserve the graph and layout; the active lens is presentation state.

The existing research-export adapter treats demonstration material as local,
unreviewed hypotheses and never exports its external links as source locators.
There is no corpus writeback, review submission or admission action.

## Build and serve locally

Keep the private source library and compiled output outside Git. On the Abyss
host, reserve output storage and use the resource launcher for builds and the
service. With an existing private library at the admitted output directory:

```sh
node node_modules/vite/bin/vite.js build --config vite.constructor.config.mjs \
  --outDir "$TOS_MOCKUP_DIR"
python3 constructor/serve.py "$TOS_MOCKUP_DIR" --port 44338
```

The server binds only loopback and serves `constructor.html`, `library.json`
and contained assets. It does not serve receipts, QA files or source files.
The original private-library builder remains reproducible against its owner
source and the previous three-translation demo packet; see its command help.

## A separate application window

The optional Linux launcher in [`desktop/`](desktop/README.md) installs a menu
entry and an on-demand loopback service for one exact built release. It uses a
dedicated Chromium profile and cache, preserving the ordinary browser profile.
Install a completed release after reserving its output and runtime storage;
see the desktop README for the dry run, install command and lifecycle checks.
The application can run independently of the repository dev server or Codex.

The installed source snapshot, compiled release and private source library
remain separate. Keep an immutable previous release when developing another
version; changing this fixture creates a new workspace fingerprint and does
not silently migrate a personal graph.

## Controls and validation

Click a star to read; click a line to inspect its relationship. Drag a star to
move it, drag the background to rotate, Shift-drag to pan, and use the wheel to
zoom. H hides controls, F toggles full screen, O frames the current view, and
Ctrl/Cmd+Z undoes a graph change. A hidden tab pauses the atmosphere; reduced
motion disables ambient movement and transitions.

Run the focused model, lens, journey and desktop tests, compile the separate entry, validate the
actual prepared graph, and exercise the complete result in a browser. Local
checks do not establish CI, merge, Foundation integration, public publication,
translation review or semantic admission.

## Личное поле при смысловом обновлении

Новая редакция содержания имеет новый полный fingerprint библиотеки.
`semantic-workspace.mjs` допускает перенос только между точно проверенными редакциями:
исходные тексты, идентичности и иерархия материалов, топология связей и порядок
шагов маршрутов совпадают. Перед копированием личное дерево проходит полный
валидатор новой библиотеки; собственные мысли, связи, позиции и намеренно пустое
поле сохраняются. Прежние ключи остаются нетронутыми, существующая новая редакция
никогда не перезаписывается. Запуски окон этой редакции согласуются через
Web Locks перед чтением рабочего поля; без этого механизма перенос не выполняется.
Ошибка чтения или записи видна в интерфейсе.
Из подходящих прежних редакций выбирается самая новая, в которой есть сохранение.
Повреждённая новая запись не заменяется более старой. Это сохраняет работу и тех,
кто пропустил промежуточное обновление, и тех, кто уже работал в нём.
Для другой редакции нужно сначала проверить совместимость и обновить точную привязку;
совпадение одного имени или ID не разрешает перепривязку к другому источнику.
Заметки читальни продолжают принадлежать прежним неизменённым версиям текста.
