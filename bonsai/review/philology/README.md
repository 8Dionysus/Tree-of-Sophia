# The philological core

A stand reads its book with words. The words that carry the book's teaching are
kept here once, each with its origin: the word of the original, the place where
the book first says it, the word every translation shown beside it uses there,
and the word the Tree itself reads in every language. The stand's own texts name
a term with the Tree's word; the reader opens a term on its origin.

## The module

`bonsai/<stand>/terms.json`, declared in the stand's `bonsai.json` as the module
`terms`, schema `tos.bonsai.terms.v1`, checked by `bonsai/lib/terms.mjs` within
`node bonsai/lib/cli.mjs check`.

```jsonc
{
  "schema": "tos.bonsai.terms.v1",
  "work": "za",
  "editions": {
    "antonovsky-1913": {"lang": "ru", "label": {"ru": "пер. Ю. М. Антоновского, 1913", "en": "tr. Yu. M. Antonovsky, 1913"}},
    "common-1909": {"lang": "en", "label": {"ru": "пер. Т. Коммона, 1909", "en": "tr. Thomas Common, 1909"}}
  },
  "terms": [{
    "id": "ubermensch",
    "original": {"lang": "de", "lemma": "Übermensch", "forms": ["Übermensch", "Übermenschen"]},
    "first": {"w": "also-sprach-zarathustra", "c": "I.01", "u": ["<unit>", "<unit>"], "s": 0, "e": 33, "q": "Ich lehre euch den Übermenschen."},
    "renderings": {
      "ru": {"tos": {"term": "сверхчеловек", "forms": ["сверхчеловек", "сверхчеловека", "…"]},
             "witness": {"edition": "antonovsky-1913", "term": "сверхчеловек", "forms": ["сверхчеловеке", "…"]},
             "avoid": ["…"]},
      "en": {"tos": {"term": "overman", "forms": ["overman", "overmen"], "basis": "…"},
             "witness": {"edition": "common-1909", "term": "Superman", "forms": ["Superman"]},
             "avoid": ["superman"]}
    },
    "gloss": {"ru": "…", "en": "…"},
    "concept": "overman"
  }]
}
```

- `original`: the word as the original writes it (its lemma and the forms the
  book uses). `first` is the first place in the book's order where the original
  uses the word as this term, cited the way the core cites (`w`, `c`, `u`, `s`,
  `e`, `q`). The check finds a form of the original at that place.
- `renderings.<lang>.witness`: the word of the translation the stand shows in
  that language (Antonovsky 1913 for Russian, Common 1909 for English), with the
  forms it takes in that translation. The check finds one of them in the
  translation's counterpart of the first place.
  A stand with several editions may instead give an array of witness objects,
  each naming its edition, term and forms. In Antiquity these words are tied to
  the edition of the quoted passage; the Tree's own forms remain available in
  its authored text. The edition labels are recorded once in `editions`, each
  with a `short` name (translator and year) in both languages. The origin card
  gives one line per word with the editions that use it, and the Tree's word in
  the reader's language when no shown translation says it there.
- Antiquity cites a place directly by work and corpus unit range (or an external
  fragment locator), without a chapter key. Its original can be Greek or Latin;
  Greek matching ignores accents and breathings while retaining the spelling of
  the displayed witness. The first place is one of the collection's cited uses.
  For an especially ambiguous Tree word, `contextWorks` limits its activation
  in a work-specific article or quotation to the works evidenced by its uses;
  the original-language forms remain readable in any work.
- `renderings.<lang>.tos`: the word the Tree reads, with every form its texts
  use. Russian reads Antonovsky's word: the stand's Russian texts speak with the
  words of the Russian witness shown beside them. English reads the Tree's own
  translation of the term, chosen by the owner; `basis` says in one sentence why
  this word. A form belongs to one term only.
- `renderings.<lang>.avoid`: words the stand's texts do not use for this term in
  that language: the witness's word where the Tree reads its own, split variants,
  and words a language review took out. The language guard
  (`access/web/src/public-research/bonsai-language.test.mjs`) reads them.
- `gloss`: one sentence per language on what the word does in the book, in the
  book's own words where they serve, said affirmatively.
- `concept`: the id of the core's concept or figure the term names, when it has one:
  the node is the term itself (Schaffende → creation, Augenblick → present). A term
  that belongs to a concept without naming it (Erde, Kameel, Werthe) carries no
  `concept`.

## The reader

A stand's text that names a term with a node carries it as an active word, a link
to that node: in Zarathustra the concept or figure of `concept`, in Antiquity the
library's concept of `concept` or else the term's own library entry. Pointing at
the word shows the node's kind, title and opening line; following it opens the node.
A word does not link the node whose card holds it. A term without a node stays
plain text. The card speaks of the node; the term's philology (the original, the
translations' words, the first place, the gloss) stays with this core.

## The word layer

Every word of every witness is read at its exact place, in three layers kept
apart. The tools live here; their inputs and model runtimes stay outside the
checkout.

1. **Spelling** — `bonsai/<stand>/spelling.json` (`tos.bonsai.spelling.v1`).
   A word the German first editions break at a line end with `¬` reads whole.
   Each historical spelling gets today's spelling with its ground: Wiktionary
   files it as an obsolete, archaic or Swiss spelling of that word
   (`wiktionary`: Thier → Tier, gieng → ging, Räthsel → Rätsel), or the
   orthographic rules give a word Wiktionary knows (`rules`: Thiere → Tiere,
   heisst → heißt), or a compound whose parts it knows (`rules+compound`:
   Thorweg → Torweg). The rules are listed in the module. A word Wiktionary
   records as another word (ward for wurde) keeps its own form. The witness's
   text never changes.
2. **Words** — `bonsai/<stand>/tokens.json` with `tokens/<chapter>.json`
   (`tos.bonsai.tokens.v1`). Stanza reads each unit, the German in today's
   spelling, and gives every word its lemma, part of speech, features and
   syntactic head (Universal Dependencies). Rows count Unicode code points of
   the unit's exact text; each witness's units carry the digest of the text
   the layer was made from, and the check fails when a witness changes until
   the layer is made again.
3. **Lemma** — each lemma is checked against Wiktionary's forms. `a`: the
   model's lemma is among Wiktionary's lemmas for the form and part of speech.
   `l`: the model's lemma is no word (geingen for gieng, Mitleide for
   Mitleiden), and Wiktionary's lemma that fits the part of speech and
   features stands in its place. `m`: Wiktionary does not know the form, and
   the model's lemma stands alone. A real word the model chose in context is
   kept.

The check (`node bonsai/lib/cli.mjs check`) reads every row against its unit
and reports, as drift, the forms of a term that the layer finds under the
term's lemma and the term does not list.

```sh
python3 lexicon.py kaikki.org-dictionary-German.jsonl $LEX/de.sqlite --lang de
python3 lexicon.py kaikki.org-dictionary-Russian.jsonl $LEX/ru.sqlite --lang ru
python3 annotate.py <repo> zarathustra --lexicon-dir $LEX        # spelling.json, tokens/
python3 origins.py <repo> zarathustra --lexicon-dir $LEX         # origins.json, origins/, renderings.json
python3 histories.py told dewiktionary.jsonl.gz ruwiktionary.jsonl.gz --out $TOLD
python3 histories.py attest <repo> zarathustra --cache $DTA/dta.jsonl
python3 histories.py build <repo> zarathustra --lexicon-dir $LEX --told-dir $TOLD --attest $DTA/dta.jsonl   # histories.json, histories/
node bonsai/lib/layers.mjs manifest bonsai/zarathustra             # layers.json: the new digests
node bonsai/lib/layers.mjs send bonsai/zarathustra                 # new files onto layers/transfer
```

These files live in R2 and stay out of git; `layers.json` is their digest
manifest ([layers kept in R2](../../README.md#layers-kept-in-r2-tosbonsailayersv1)),
and `layers.mjs fetch bonsai/zarathustra` brings them into a checkout.

`annotate.py --finalize-only` rebinds the layer's digests and chapter index
without reading the text again. The pinned runtime is in
[`requirements.txt`](requirements.txt); Wiktionary's per-language extracts
come from [kaikki.org](https://kaikki.org/dictionary/) (Wiktextract).

## The origin of a word

`bonsai/<stand>/origins.json` with `origins/<language>-<shard>.json`
(`tos.bonsai.origins.v1`) says, for every lemma of every witness, where the
word comes from:

- its descent, oldest form first, as Wiktionary's etymology records it: each
  generation a language and a form, marked where the word passed on otherwise
  than by inheritance (borrowed, a calque, a semantic loan);
- the parts it is made of (Übermensch: über- + Mensch), each part with its own
  descent;
- its kindred words in other languages;
- how the first editions write it, where that differs from today (Thier);
- for a translation's word, the word of the original it renders, found by Dice
  association over the sentence alignment of `lib/align.mjs`
  (сверхчеловек: Übermensch, of which it is a calque);
- its first place in the book.

### The history of a word

`bonsai/<stand>/histories.json` with `histories/<language>-<shard>.json`
(`tos.bonsai.histories.v1`, written by `histories.py`) holds, for every lemma
of the original and the Russian witness, what is told and found of the word
beyond its descent:

- `told`: its etymology as each Wiktionary edition tells it, in the edition's
  own language: the English edition (in the lexicon of `lexicon.py`), the
  German edition (*Herkunft*, which often says since when the word is attested:
  Mitleid since the 14th century, in today's form since the 17th) and the
  Russian (*Этимология*, with the dictionary it draws on: Vasmer, Chernykh,
  Krylov). Each telling is the entry's first sentences, without the lists of
  kindred forms that the origin's *Kindred words* already holds.
- `attested`, for the original's words: the word in the Deutsches Textarchiv
  (DTA), German texts of the 15th to the early 20th century under CC BY-SA 4.0,
  read through the DTA's own lemmatizer outside Nietzsche's works. The year of
  its earliest use; its uses per quarter century, read against the corpus's
  nouns in that quarter century; the earliest uses by up to three authors, each
  with its sentence as the work spells it and the page of the work in the
  archive; Nietzsche's works in the archive that use it. Übermensch: Herder
  1769, Goethe's *Faust* 1790, Jean Paul 1809, then *Also sprach Zarathustra*
  1883.
- `book`: how often the book says the word, part by part.

`histories.py told` indexes the Wiktextract extracts of the German and Russian
Wiktionary (kaikki.org); `attest` asks the DTA's public corpus search (D*,
`ddc.dwds.de/dstar/dta`), a few questions at a time, and keeps every answer in
a cache file, so that `build` runs offline from it and a second `attest` asks
only what the cache lacks. The manifest records the extracts' digests, the
queries and when the archive was asked.

The origin sheet tells the word first (the reader's language, then the word's
own edition, then English; the other editions folded beneath, each with its
entry linked), then its descent, parts and kin, then *In German texts*: the
first year, the frequency by quarter century with the quarter of the book
marked, the earliest sentences with the word marked and the archive's page
linked, and Nietzsche's works; under all of it, how often the book says the
word. For a translation's word the original's word comes first, with its whole
history.

### The story of a word

`bonsai/<stand>/stories.json` (`tos.bonsai.stories.v1`, written by
`stories.py bundle`) holds, for each single-word term whose word has a
history, a short story of its word in Russian and English, told by a writer
model from the stand's own sources only. Each story stands on the original and
its own language, as its reader sees them: the Russian on German and Russian
pieces and Antonovsky's words, the English on German and English pieces and
Common's; the check refuses a piece, a word or a letter of the other one. A term's word has its history under
its own lemma, its modern spelling or the lemma its forms take in the book
(Thier → Tier, Werthe → Wert); `packets` names the terms without one. `stories.py packets` gathers each term's packet, every piece with
its id: the three Wiktionary tellings (`W.*`), the descent (`E`), the archive's
frequency and earliest uses (`A`, `A1`…`A3`), Nietzsche's works there (`N`),
the book's count (`B`) and first place in each part (`B1`…`B4`), the places by
which the Tree reads the term's concept or figure (`K1`…`K4`, its formulas and
appearances with the Tree's line on each), the translators' words by their
places (`T`) and the etymology of Antonovsky's commonest word (`V.*`). The
writer follows [`BRIEF-word-stories.md`](BRIEF-word-stories.md); every
sentence names the pieces it stands on. `stories.py check` reads each story
against its packet: every number, quotation and name of a sentence stands in
the pieces it cites, or among the names the story declares with their source
form; a second model reads the stories against the packets for what the check
leaves to reading (a chapter, the sense of a paraphrase). `bundle` writes the
layer only when every such term has a story that passes and stands on the
current packet, each story with every cited piece as the stand shows it.

The origin sheet tells the story at the top of the original's word, each
sentence followed by its pieces' numbers. A number leads to its piece: the
sheet's own telling, descent, archive use or count where the sheet shows it,
else the piece drawn under the story (a place of the book with its
translations and an *Open* button, the translators' words, the Russian word's
etymology).

```sh
python3 stories.py packets <repo> zarathustra --out $OUT
#   the writer: BRIEF-word-stories.md, stories into $OUT/stories/<term>.json
python3 stories.py check $OUT
python3 stories.py bundle $OUT <repo> zarathustra      # stories.json
```

### Where a selected word offers its origin

A single selected word of the book's three witnesses shows a small menu beside
it, wherever the stand shows that text, once the word layer reads the word at
that place:

| Where the word stands | What the menu offers |
| --- | --- |
| a quotation on a node's card, in the original or the translation | *Origin of the word*; *Translations* for a word of a term |
| a place under a word of the node's *Translations* tab | the same |
| the parallel reading of a chapter (*Read in the whole work*), either column | the same, above the dialog |
| a place in a sheet the menu opened (the origin's *Open*, a term's renderings) | the same, above the sheet |
| the reader, in the first editions (DTA), Antonovsky or Common | the same; the origin opens in the reader's own *Origin* tab |

The menu shows nothing for a phrase, for the stand's own text (titles,
questions, explanations), for the reader's other editions and the front matter
of an edition (the layer has no words there), and for a stand without a word
layer (Antiquity). *Origin of the word* opens its descent; for a translation's
word the descent of the original's word comes first. *Translations* opens the
term's renderings in the translation shown beside the original (for a word of
the original) or in the word's own translation. Nothing loads until the reader
selects a word, and the menu stands in the top layer, inside the dialog that
holds the word.

## How the translations render a term

`bonsai/<stand>/renderings.json` (`tos.bonsai.renderings.v1`) holds, for every
single-word term, each translation's words for it place by place. A place is a
word of the original whose form the term lists; the translation's words are the
lemmas most associated with those places over the aligned sentence pairs (Dice);
each place stands under the strongest of them that its aligned sentence holds.
The stand shows it in its own view, *Translations*: one band per word, as wide
as its places, and under each word the sentences where it stands. A term's node
has it as a tab; a word of a term selected in the book offers it beside its
origin, for the original's word in the reader's translation and for a
translation's word in that translation.
`origins.py` writes the module with the origins.
