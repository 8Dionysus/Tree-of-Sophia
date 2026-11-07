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
python3 origins.py <repo> zarathustra --lexicon-dir $LEX         # origins.json, origins/
```

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

In the reader and in the stand's quotations, a selected word shows a small
menu beside it; *Origin of the word* opens its descent. For a translation's
word the descent of the original's word comes first. Nothing loads until the
reader asks.

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
