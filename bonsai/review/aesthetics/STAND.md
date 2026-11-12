# The Aesthetics stand: how the space reads the field

Plan of 2026-11-11 for `/aesthetics/`, the third stand of the site. It says what
the stand shows, where each thing comes from in the bonsai, and which places of
the space engine learn a third profile. It is removed when the stand is open
and its tests carry what is written here.

## What the bonsai holds for the stand

| Module | What the stand takes from it |
|---|---|
| `field.json` | 36 areas in groups A–D, each with its question; concepts; conceptions (one work's answer in a named place: `of`, `areas`, `work`, `agents`, `locus`, `formula.cite`, `thesis`, `voice`, `distinction`); links between concepts, named by a verb and resting on conceptions (`grounds`) |
| `library.json` | works and authors, with names in Russian, English and the original |
| `fragments.json`, `alignment.json` | the words of every place a conception cites (`{w, loc}`), in the original and in translations, with edition and rights; places of works the public corpus holds are aligned to it |
| `chronology.json` | the life of every author (Wikidata) and the dating of every work (the register's wording and source) |
| `identities.json` | the graft map |

The stand shows a work's own words beside the thesis. A conception's formula is
a `cite` in the shape the Antiquity collection uses for its bodies, so the
fragment reader (`access/web/src/public-research/fragments.mjs`) serves both.

## What a reader sees

| In the field | In the space | Material id |
|---|---|---|
| area | a branch: a question star, its conceptions as moves in the order of their works' datings | `aesthetics:route:<area id>` |
| conception | a reading: titled by the concepts it is of and its author, its body the thesis, its question the area's, its voice where the field names one | `aesthetics:reading:<conception id>` |
| place of a formula | a source under its work: the work's title and the place's label; its words come through the fragment reader | `aesthetics:source:<work id>:<loc>` |
| work, author | catalogue entries; an author carries the life, a work the dating | `aesthetics:catalog:<id>` |
| concept with at least one conception | a catalogue entry; the twelve categories of group A are marked as categories | `aesthetics:catalog:<concept id>` |
| link | an edge between its two concepts, labelled by the verb; the card names the readings it rests on | `aesthetics:relation:<link id>` |

A concept without a conception, an area without one and a link whose grounds
the field does not hold are not shown: the space opens with what a place of a
work bears.

Lenses, as Antiquity names them in its manifest (`tree`, `sources`, `time`,
`proximity` by shared concepts; `meaning` and `tensions` when the field gives
them something to group by):

- **tree:** the crown is the areas, set in their four groups; the trunk is the
  concepts; the roots are the works in the order of their authors' lives.
- **sources:** work → place → reading.
- **time:** the years of the works from the chronology; rows are the groups A–D.

Depth is Antiquity's: source, reading, comparison, the reader's own record.

## Where the engine learns the third profile

`access/web/src/public-research/` knows two profiles today. The places found
on 2026-11-11 (main at `acc1539`):

1. `app.mjs`: the profile is read as `antiquity` or else `zarathustra`; the
   page's own words (`copy`, `copyEn`) exist for two.
2. `space-library.mjs`: `createPublicSpace` builds the library through
   `makeZarathustra` or `makeAntiquity`, both from documents of the public
   corpus (`selectedDocuments` asks `manifest.profiles[profile]` and scene
   anchors). Aesthetics has no document of its own in the corpus; its library
   is built from the field (`makeAesthetics`), and the corpus profile
   (`access/public-corpus/selection.v1.json`) lists the corpus works the
   alignment names, with no anchors.
3. `collection-data.mjs`: `extendPublicCollection` is Zarathustra or else
   Antiquity.
4. `bonsai.mjs`: `LOADERS` has no entry for aesthetics (the manifest is
   already imported).
5. `fragments.mjs`: fragment and alignment loaders per profile; the rule for
   reading languages is `exact` for Antiquity alone.
6. `public-space.mjs`: about forty branches on `profile==='antiquity'`
   (timeline, tree layout, route words, near groups, passage lookup, cards).
   `space-tree.mjs` (`antiquityTree`, `AREA_TITLES`, `SCHOOL_ERA`),
   `space-timeline.mjs` (`antiquityTimeline`), `reference-outline.mjs`
   (`antiquityBook`), `reference-book.mjs`, `term-view.mjs` and
   `personal-kind.mjs` (the namespace pattern of saved graphs) name Antiquity.
7. `stands.mjs`, `access/web/aesthetics/index.html`, `vite.config.ts` (two
   inputs), `src/home/home.mjs` (the card's words in five languages),
   `public/trees/aesthetics.svg` (drawn by hand by the owner, never
   generated), `public/og/aesthetics-{en,ru}.jpg`.

The Antiquity path is the nearer one: a question with moves, readings with
sources, a catalogue. Where a branch on `antiquity` is about that shape, it
becomes a branch on the shape; where it is about Antiquity's own words, eras
or schools, Aesthetics gets its own.

## Order

1. Data (done here): `chronology.json`, its generator and test.
2. `makeAesthetics` and the loaders: the stand opens with stars, cards and the
   tree lens.
3. Time and sources lenses, proximity.
4. The page shell, the list of stands, the home card; the tree drawing and the
   images from the owner.
5. The manifest's `lenses` and `depth`, the stand's tests, the public build.

## Open

- A conception has no title of its own. The stand derives one from its
  concepts and author; a short authored title would come through the
  planting line.
- A work the register dates by an edition after its author's life (Goethe's
  *Maximen und Reflexionen*, 1907) stands on the time lens at that year.
- Nine areas hold no conception yet; their readings wait in works the
  planting line's rights rule does not reach.
