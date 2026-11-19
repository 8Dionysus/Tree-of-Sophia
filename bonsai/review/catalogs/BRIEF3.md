# Third round: the core of Zarathustra

Read `BRIEF.md` in this folder first: everything there holds (the catalogs, the
key rules, placeholders, spaces, consistency with the existing catalog, titles
of works, names, nothing from memory, the check). This round brings into the
catalogs the core of the Zarathustra stand, `bonsai/zarathustra/core/`: the
names of its subjects, figures, routes and states, their theses, answers and
questions, the verbs and senses of its links, the lines of its witnesses and
of the places of the book, the history of its reviews, and the forms of the
constructor that came after the second round.

## Your part

Your task names your language and your part:

- `todo2/<lang>-titles.json`: the names of subjects, figures, routes and
  states. This part is done first; every other part names its subjects by it.
- `todo2/<lang>-<k>.json`: the rest, in order of where each form stands, so
  that one record's forms come together.

Each item is `{section, en, ru, where}`. `where` names the file, the field and
the record the form belongs to: `zarathustra/core/links.json.verb · link
overman → love-of-the-farthest` is the verb of the link from the overman to the
love of the farthest.

Write `translated2/<lang>-<part>.json` (`<part>` is `titles` or the number) as
`{"<lang>": {"<en>": "<translation>"}}` with every item of your part. Write it
as you go, a hundred or two items at a time, so that your work stands on disk
at every moment and a new turn continues from where the file ends.

## The rules of this round

- **Names.** A part with a number reads `translated2/<lang>-titles.json` whole
  before it begins, and calls every subject, figure, route and state by the
  name it has there, in every form that names it.
- **Verbs of links.** The verb of a link (`links.json.verb`,
  `chains.json.steps.verb`, `path.verb`, `label`, and their earlier forms in
  `proposed` and `verb_revision_history`) is a predicate whose subject is the
  first subject of its link: “directs love of the farthest” says what the
  overman does. Give it as a predicate of your language that follows the name
  of its subject, with no subject of its own.
- **The book's words.** A line of a witness or of a place
  (`appearances.line`, `voice.line`, `occurrences.line`, `counter_voices.line`)
  that quotes the book is, in German, Nietzsche's own wording from
  `bonsai/zarathustra/witnesses/<chapter>.json` (units under `units.de`); the
  chapter stands in the record's `cite.c` in the core file. In Spanish and
  Chinese translate the quotation from the English and the Russian.
- **Terms.** A term of the book keeps the word the catalogs of your language
  already give it (search `bonsai/locales/<lang>.json` and
  `access/web/src/public-research/locales/<lang>.json`); German takes
  Nietzsche's own word for it (Übermensch, ewige Wiederkunft, der Geist der
  Schwere).
- **Register.** The Russian is the author's wording. Say what it says, in its
  affirmations, precisely and plainly; add no explanation, hedge or contrast of
  your own.

## Check

```sh
python3 -I -c "import json,re,sys;l,p=sys.argv[1:3];t=json.load(open(f'translated2/{l}-{p}.json'))[l];todo=json.load(open(f'todo2/{l}-{p}.json'));m=lambda s:sorted(re.findall(r'\{[^{}]+\}',s));bad=[x['en'] for x in todo if not (t.get(x['en']) or '').strip() or m(t[x['en']])!=m(x['en'])];print(len(todo),'items,',len(bad),'problems');print(bad[:5])" <lang> <part>
```

Run it until it reports 0 problems. Report back in a few lines: the count, any
form you left unchanged and why, and any term you chose that the catalogs did
not already settle.
