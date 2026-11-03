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
- `concept`: the id of the core's concept or figure the term names, when it has one.

## The reader

A stand's text that names a term carries it as an active word: pointing at it
shows the origin (the original word, the translations' words, the first place,
the gloss); opening it opens the book at the first place.
