# An independent check of the fragments the two stands now show

Tree of Sophia's public stands (Zarathustra and Antiquity, in
`the repository root`) no longer retell works: what a work
says is shown as an exact fragment of it, and the stands' own words remain
only for readings, arguments, distinctions, links between places, questions,
methods and context. You did not take part in this work; check it with fresh
eyes and report what is wrong. Do not change any file in the repository.

## What a fragment is

An authored form is `{ru, en, cite}`; `cite` lists places:

- `{w: 'also-sprach-zarathustra', c: '<chapter>', u: [first, last], s?, e?}`:
  units of the witnesses in `bonsai/zarathustra/witnesses/<chapter>.json`
  (`units.de|en|ru`, each `[id, kind, text, section, ...]`; `s`/`e` are
  character offsets into the first and the last unit's text of the original).
  `bonsai/lib/witnesses.mjs` (`readChapter`, `fragmentOf`) shows how a
  fragment is read and how the translations are found.
- `{w: <corpus work key>, u: [firstUnitId, lastUnitId], s?, e?}`: units of a
  work in the public corpus (original language); the translation shown comes
  from the stand's `alignment.json` (sentence pairs; `bonsai/lib/corpus-align.mjs`).
  The corpus is read with
  `bonsai/review/frag/zc.py`
  (`python3 zc.py show <work> grc|la|en <first> <last>`, `find <work> grc "…"`);
  run one corpus command at a time (little memory).
- `{w: <key>, loc: '<locator>'}`: a place of another work, kept with its text,
  witnesses and rights in the stand's `fragments.json`.

Content lives in `access/web/constructor/*.mjs` (Zarathustra) and
`bonsai/antiquity/collection.json`, `first-branches.json` (Antiquity).

## Check

Take a random sample (seed it, say how): 20 Zarathustra cites, 30 Antiquity
corpus cites, and 16 places of the two `fragments.json` files (8 each); read
the rights record of every witness in both `fragments.json` files. For each:

1. The original shown is exactly the text of the place named (the span, the
   unit, the locator): no missing or extra words at the edges that change
   what the place says, no neighbouring sentence instead of the place.
2. The translation shown is the same place in that translation (for corpus
   works, read the sentence pairs and the translation's text; say when the
   translation starts or ends with a neighbouring sentence).
3. The own words beside it do not retell what the fragment shows, and read
   right after it (no "this", "here", "he" pointing at something not shown).
4. For `fragments.json`: the text matches its source (fetch the source named
   by `url`/`links` from the shell when it is on Wikisource, Gutenberg, GitHub
   or the Internet Archive; never through a tool that rewrites pages), the
   label names the place correctly, and the rights reasoning holds: public
   domain where the author, translator and editor died long enough ago under
   Russian (ГК РФ ст. 1281; ФЗ 231-ФЗ ст. 6), US and EU law; a short quotation
   only where the work is protected, with author and source named. Say
   plainly if something is placed that may not be, and also if something
   that may be placed was left out through a too cautious reading.

Never use a personal e-mail address anywhere (User-Agent included); do not get
around bot checks, logins or paywalls.

## Report

Write `bonsai/review/verify/report.json`
as a list of `{where, kind, what, evidence}` (kind: wrong-text, wrong-place,
wrong-translation, retelling, dangling-reference, label, rights, other), and
report back in a few lines: what you sampled, how many were right, and every
problem with its evidence.
