# Translating the stand's new wording into Spanish, German and Simplified Chinese

## What this is

Tree of Sophia's public stands (Zarathustra and Antiquity) are shown in
Russian and English from their own modules, and in Spanish, German and
Simplified Chinese through display catalogs that map each English form to its
translation:

- `access/web/src/public-research/locales/<lang>.json` (section `public`)
- `bonsai/locales/<lang>.json` (section `bonsai`)

The stands were just rewritten so that they never retell a work in their own
words: what a work says is now shown as an exact quotation of it, and the
stands' own words remain only for readings, arguments, distinctions, links
between places, questions, methods and context. Many own-word forms were cut
down or reworded, and the bonsai gained new modules (the witnesses of
Zarathustra, places of other works, typed objects of thought, alignment). The
list you get holds every English form that now lacks a translation in your
language.

## Your job

Your language is given in the task. Read `todo/<lang>.json` in this folder
(`bonsai/review/catalogs/`):
a list of `{section, en, ru}`. Translate each `en` into your language, using
the Russian `ru` (the author's original wording) to settle any ambiguity of the
English. Write `translated/<lang>.json` as `{"<lang>": {"<en exactly as given>": "<translation>"}}`
with every item of the list.

Rules:

- Keep the key exactly as given (character for character, including spaces,
  quotes and dashes).
- Keep placeholders such as `{count}` or `{name}` exactly, in the same number.
- Keep a leading or trailing space or newline of the English exactly (not
  needed for Simplified Chinese).
- Translate the meaning, not word for word; keep the register: precise,
  scholarly, plain, without decoration. A short interface label stays short.
- Stay consistent with the existing catalog of your language: before you
  start, read a few hundred entries of both existing catalogs and keep their
  terms (for instance how they render "eternal recurrence", "reading",
  "branch", "move", "route", "lens", "ground", "witness"). Search the existing
  catalogs for a term whenever you are unsure.
- Titles of works: use the established title in your language (e.g. German
  *Also sprach Zarathustra*, *Die fröhliche Wissenschaft*; Spanish *Así habló
  Zaratustra*, *La gaya ciencia*; Chinese 《查拉图斯特拉如是说》, 《快乐的科学》),
  as the existing catalogs do.
- A quotation from Zarathustra inside the English: in German, give Nietzsche's
  own wording; the German text of the book is in
  `bonsai/zarathustra/witnesses/<chapter>.json`
  (units under `units.de`, each `[id, kind, text, section, ...]`). Search it with
  a short script. In Spanish and Chinese translate the quotation from the
  English and Russian. Never invent a quotation.
- Names of people as the existing catalog writes them.
- Do not translate an English form that is a code, an identifier or a proper
  name only; give it unchanged.
- Nothing from memory about the content: if a form is unclear, read the
  Russian, and if still unclear, find the form in the stand's modules
  (`access/web/constructor/*.mjs`,
  `bonsai/*/*.json`) to see its context.

Check your file with:

```sh
python3 -I -c "import json,sys;t=json.load(open('translated/<lang>.json'))['<lang>'];todo=json.load(open('todo/<lang>.json'));import re;m=lambda s:sorted(re.findall(r'\{[^{}]+\}',s));bad=[x['en'] for x in todo if x['en'] not in t or not t[x['en']].strip() or m(t[x['en']])!=m(x['en'])];print(len(todo),'items',len(bad),'problems',bad[:5])"
```

until it reports 0 problems. Report back in a few lines: the count, any form
you left unchanged and why, and any term you chose that the existing catalog
did not already settle.
