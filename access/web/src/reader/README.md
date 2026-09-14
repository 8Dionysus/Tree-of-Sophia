# Research reader

`createResearchReader` is a reusable browser view over supplied, verified text
documents. The constructor is its first host. Open **Reading / Чтение** in the
header to resume the last document, or open a passage from a star to carry its
research context into the reader.

The text occupies the central page. The library, research notebook and source
details are contextual panels. Choose a paragraph number to bookmark it, leave
a note or copy a quotation with attribution. The host can turn a personal note
into a local graph node linked to the passage's prepared material. This action
does not edit a corpus source or submit anything for review.

The reader supports literal search within the selected version (`Ctrl/Cmd+F`,
Enter / Shift+Enter), separate scroll positions for each version,
independent columns, type size, line spacing, page width and night/paper themes.
Side-by-side mode makes no paragraph-alignment claim. The original strings are
inserted with `textContent`; search marks preserve every original character.

## Adapter seam

Import `reader.mjs` and create a notebook from `notebook.mjs`. Pass:

- `host`: the DOM parent, defaulting to `document.body`.
- `documents`: an ordered array of already verified documents.
- `notebook`: the `createReaderNotebook` instance.
- `locale()`: the interface language (`ru` or `en`). Text versions remain
  independently selectable.
  When the document declares `originalLanguage`, its matching version is marked
  **Original** with its actual language code. Russian, English and the original
  are individually readable. Side-by-side mode shows two versions, with an
  explicit comparison selector when more than two are supplied. It initially
  pairs a translation with the original; the chosen pair stays with that
  document during the current reader session. Positions and notes remain
  independently persistent for every version in the existing notebook schema.
- `related(documentId)`: optional `{id,title,context}` entries in the host tree.
- `context(contextId, documentId)`: optional wording for the entry context.
- `guide(documentId)`: optional host-authored close-reading guidance with
  bilingual `orientation`, `question`, `limit`, and `moves` containing `title`,
  `body` and version-keyed zero-based `paragraphs`. The host must bind these
  positions to the exact verified text edition. The view checks paragraph
  bounds and offers a jump in the selected version; it creates no note and
  asserts no alignment between translations. Changed source versions require
  re-reviewed positions. The constructor withholds its guides when the catalog
  digest differs from the edition they were read against.
- `onReveal(id)`: optional synchronous host navigation.
- `onDevelop({document,anchor,text,citation,contextId})`: optional synchronous
  creation of a personal thought. Throw on failure; the reader then stays open.

The returned object has `open({documentId?,contextId?})`, `close()`, `isOpen()`
and `destroy()`. The host must suspend its own global keyboard shortcuts while
`isOpen()` is true. Closing restores focus to the entry button.

Each document has `id`, bilingual `title`, `author`, `work`, `locator`, and
`status`. An `available` document has an explicit `boundary` and `versions`
keyed by language. Each version has nonempty `paragraphs`, `sourceRevision`,
`textSha256` (SHA-256 of paragraphs joined with two line feeds), `translator`,
`edition`, `editorialNote`, `sourceUrl`, and `rights` with `basis`, `credit`,
`label` and `url`. A `link-only` document has a `reason` and `links` instead.
The reader preserves the supplied availability; its library does not assert
that whole works or the entire ToS corpus are available.

The constructor's `bindFragmentCatalog` validates completeness declarations,
material bindings, digests and reviewed audience metadata before passing this
envelope. The view does not independently grant rights or assess translation
quality. A different host must validate its own source envelope and allowed
use before calling it. The view imports no constructor or observatory code.

## Personal notebook

The independent storage namespace is `tos-reader-notebook-v1`. The schema is
`tos_reader_notebook_v1`; graph data and guided-route state keep their existing
keys. A notebook anchor consists of document ID, version, source revision,
text digest and a **zero-based local paragraph ordinal**. These ordinals are
not stable ToS corpus unit IDs. Changed or absent versions retain their notes
for export and are never silently retargeted. A future corpus adapter must
provide explicit version/unit migration or use a new notebook representation;
it cannot equate these selectors with the existing observatory anchor ABI.

The notebook holds at most 64 recent version positions, 256 bookmarks and 200
notes of 4,000 characters each, within a 1.5 MB UTF-8 JSON envelope. Notes save
after 450 ms and flush before reader navigation, close and page hide. Emptying
a note removes it. Font and layout changes restore the version's paragraph
and its fractional position, rather than transferring a raw scroll offset.

Storage contains user-written notes, selectors and preferences, without
implicitly copying source paragraphs. **JSON** exports the entire notebook;
import validates the entire packet before replacing it. **Markdown** exports
user notes/bookmark citations without copying the source text. **Copy
quotation** is an explicit source-copy action and includes the selected
paragraph, edition, source URL, preparation and attribution.

If stored data is invalid, it is preserved. If storage is unavailable or
another tab has changed the stored packet, subsequent writing remains in
session memory and a visible message directs the reader to download it.
Reloading cannot preserve those unsaved session-only changes. There is no
server synchronization or account storage.

The constructor limits a graph note, including its source citation, to 4,000
characters; an overlong handoff leaves the notebook entry intact and asks the
reader to shorten it. It never silently truncates a citation or source quote.

## Verification

Run the focused contract tests from `access/web`:

```sh
node node_modules/vitest/vitest.mjs run src/reader/notebook.test.mjs \
  src/reader/search.test.mjs constructor/model.test.mjs \
  constructor/fragment-catalog.test.mjs --maxWorkers 1
```

The notebook tests protect version identity, atomic imports, corrupt-storage
preservation, cross-tab conflicts and capacity limits. Search tests protect
literal matching, Unicode offsets and bounded output. Also build the
constructor and check actual browser behavior: entry from a star, search,
reload, RU/EN restoration, parallel columns, keyboard focus, source details
and an undoable personal-node handoff. These checks establish the local access
behavior, not CI, corpus admission, release publication or backend integration.

A version may provide its own bilingual `locator` for the reading header and
citation, and an original-language `title` for the header. Thus the original
never borrows a translation edition’s pagination. Different historical
witnesses remain separately credited; comparison does not establish that one
was the translation’s source edition.
