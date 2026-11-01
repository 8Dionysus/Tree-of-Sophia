# Second round: the Antiquity stand's new wording, and English quotations left inside translations

Read `BRIEF.md` in this folder first: everything there holds (the catalogs,
the key rules, placeholders, spaces, consistency with the existing catalog,
titles, names, nothing from memory, the check). This round differs in two ways.

## 1. New forms

`todo2/<lang>.json` lists the English forms of the Antiquity stand (and a few
of Zarathustra's) that now lack a translation, as `{section, en, ru}`. The
Antiquity stand was rewritten like Zarathustra: what an ancient work says is
now shown as an exact quotation of the Greek or Latin, and the stand's own
words remain for readings, arguments, links between places, questions,
methods and context. Many forms are the old own words cut down: when an old
catalog entry already translates a longer form that contains this one, reuse
its wording for the part that remains. Quotations from a Greek or Latin work
inside an English form are translated into your language (German: use the
established German rendering of that work only if you can check it against a
source you can read; otherwise translate).

Write `translated2/<lang>.json` as `{"<lang>": {"<en>": "<translation>"}}`
with every item, and check it with the command of `BRIEF.md`, pointed at
`todo2/` and `translated2/`.

## 2. English quotations left inside existing translations

`round2/english-quotes.json` lists, for your language, existing catalog
entries whose translation still quotes English words, e.g. German
„… “I willed it thus” …“. For each, write the corrected translation of the
whole entry into `translated2/<lang>-fixes.json` as
`{"<lang>": {"<en key>": "<corrected translation>"}}`:

- a quotation of *Zarathustra*: in German, Nietzsche's own words from the
  first editions (search `bonsai/zarathustra/witnesses/*.json`,
  units under `units.de`); in Spanish and Chinese, translate the quoted words
  as the rest of the catalog renders that passage;
- a quotation of another work (Spinoza, Epictetus, Marcus Aurelius, Plato,
  Aristotle, Cicero, Seneca, Plutarch …): translate it into your language,
  keeping the quotation marks of your language;
- a phrase that is the stand's own wording in quotation marks (not a
  quotation of a work): translate it;
- an English title or a name of an English edition stays as it is.

Change nothing else in those entries. Report back briefly: the counts, and any
entry you left as it is and why.
