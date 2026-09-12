# Complete fragments in the local demo

A prepared star opens a complete, explicitly bounded passage through **Open
fragment**. The reader offers Russian, English and the supplied original-language
edition. **Original** identifies that edition's language; it is not a translation
into the interface language. Side-by-side reading compares two selectable versions.
Each version retains its own paragraphs; the columns do not imply sentence alignment.
The work, author, section and translator remain attached to the text. The
research panel's Source tab exposes the edition, license, preparation note and
attribution for a recorded video. The reusable [research reader](../src/reader/README.md)
adds search, version-specific places, bookmarks and a personal notebook.
Closing the reader preserves the selected star, guided-walk step and camera.

“Complete” means the whole declared source unit: a chapter, numbered section,
proposition with proof, or complete opening paragraph. It does not mean the
whole book. The binding explains why the selected passage is relevant; a shared
topic does not make it a quotation from the selected thinker. A work without a
reviewed display basis opens publisher links and an explanation instead.

## Preparing a recording edition

Keep text packets, acquired source evidence, rights/quality assessments, compiled
releases and their receipts outside the repository. `build-fragments.py` takes
explicit passage packets and a binding packet; it neither acquires texts nor
decides whether their use is lawful. The previous private `library.json` supplies
only seven navigation identities and their parent closure. Exact passages,
quotes, body text, private paths and unlisted archive entries are not copied.

From `access/web`, with storage already admitted by the host:

```sh
python3 constructor/build-fragments.py \
  --source-library /absolute/path/to/previous/library.json \
  --passages /absolute/path/to/reviewed-passages.json /absolute/path/to/link-only.json \
  --bindings /absolute/path/to/reviewed-bindings.json \
  --output /absolute/path/to/new-release \
  --receipt /absolute/path/to/assembly-receipt.json
node node_modules/vite/bin/vite.js build --config vite.constructor.config.mjs \
  --outDir /absolute/path/to/new-release
```

Use a new release directory. The builder refuses an existing HTML/library pair;
Vite preserves the prepared assets. Install only after catalog validation and
source-visible review. The desktop server's existing contained-asset allowlist
serves the digest-bound catalog; raw downloads and review evidence stay outside
the HTTP release.

## Input and failure boundaries

`fragment-catalog.mjs` owns the local `tos_demo_fragments_v1` reader schema. An
available passage carries bilingual metadata, a complete-unit declaration and
Russian and English versions. An additional original is declared by
`originalLanguage`, using its language code (`de`, `grc`, `la`, `fr`, `da` in the
current recording edition), with a matching entry in `versions`. Existing
bilingual catalogs remain readable without making an original-language claim.
Every supplied version is validated, including the original. Each has complete
paragraphs, the SHA-256 of their UTF-8
text joined by two newlines, an exact source revision, preparation notes and a
rights record that explicitly covers local reading and video display. A
link-only record has source links and an explanation, with no text versions.
Bindings name known materials, existing passages and a bilingual relevance note.

The browser checks the catalog file digest and the version text digests before
display. Missing, changed or malformed catalogs leave the graph usable and show
a reader error. All prose is rendered as text, without embedded source HTML.
No CSS truncation or summary substitution is applied to a passage.

These checks establish integrity and declared coverage. They cannot establish
translation accuracy, rights ownership, public-domain status, legal permission
or source/canon admission. Each recording edition needs a separate source-visible
quality review and a rights assessment for its exact originals, translations
and digital text layers. Online availability and private-use permission are
insufficient for video publication. Preserve attribution, adaptation notices
and the applicable text license when recording licensed material.

## Validation

Run the existing constructor model, atlas, lens and journey tests alongside
`fragment-catalog.test.mjs`, then Python `test_build_fragments.py` and the desktop
boundary tests. Validate the actual assembled catalog against the complete
prepared material set. In the browser check each supplied language, the last
paragraph, selectable parallel pairs, credits, unavailable works and return
to the same graph step.
The builder tests protect the continuing boundary against copying the previous
private library; the reader tests protect text integrity and display-basis
declarations. Neither suite acts as a semantic or legal review.
