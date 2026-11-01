# Pass over the Antiquity stand: retellings versus what only the Tree can say

## Why

Tree of Sophia must not keep a second, retold copy of a work. Whatever the work
itself says is shown by the work: an exact fragment in the original language,
with its address, opened in the whole work, and in the reader's language through
the translations aligned with it. Our own words are only for what a quotation
cannot give: a reading or interpretation, a reconstructed thesis or argument and
its steps, an objection aimed at a specific step, a distinction, a connection
between two places or two works, historical or editorial context, a question, a
method of reading. Many texts of the Antiquity stand instead retell the works
"in slightly different words". Your job: find every such sentence in your slice,
say exactly which place of the original (Greek or Latin) it retells, and keep
everything that is genuinely the Tree's own. The same pass was already done for
the Zarathustra stand; do it with the same rigor.

The owner reads these works closely. Be exact. Read the Greek or Latin (and a
translation beside it) before you judge.

## Your input

- `slices/<your-slice>.json`: a list of authored forms, each with `path` (where
  it lives in `bonsai/antiquity/collection.json` or `first-branches.json`),
  `also` (other paths holding the identical text), `ru` and `en` (the Russian and
  English of the same form) and `context`: the passage it belongs to (`passage`,
  `work`, `doc` = the corpus document id, `locator` = the address the stand gives,
  `title`, `voice`, `sourceRefs` = the links the stand offers), or the branch
  edge (`from`, `to`), question position, work map stage (`span`), dictionary
  use (`use` with `work` and `citation`), author or era it belongs to.
- The corpus helper `bonsai/review/frag/zc.py`
  (run with `python3 <path>/zc.py --help`):
  - `zc.py docs` lists every document and its versions (language:role:id).
  - `zc.py chapters <doc> grc` - structure with unit ordinals.
  - `zc.py show <doc> grc 20 22` - full units with ids and references.
  - `zc.py find <doc> grc "λόγος|ἀρετή" --chapter "Книга 7" --limit 20`
  - `zc.py show <doc> en 20 22` - the English beside it (same references).
  - `zc.py check <unit-id> "exact quote"` - verifies a quote (slow: scans all
    documents; prefer `validate.py` at the end).
  - `<doc>` is the last part of the corpus id (`apology`, `republic`, `meno`,
    `ne` = Nicomachean Ethics, `lives` = Diogenes Laertius, `ench`, `disc`,
    `med`, `on-nature` = Lucretius, `de-officiis`, ...).
  Units are coarse: a Stephanus page (Plato: unit reference "17" holds 17a–e;
  the Republic "Книга 7 · 514" holds 514a–e), a Bekker page or a book/chapter
  elsewhere, and in `lives` a whole chapter. So an anchor nearly always needs a
  `quote` that narrows the unit to the exact words.

## Classes, per sentence

- `R` retelling: states what the work itself says, narrates, depicts or has a
  speaker say: theses as the text states them, arguments as the text runs them,
  events, images, the order of a dialogue, summaries of a passage, section or
  work, even abstract ones ("Socrates and Theaetetus successively test
  definitions of knowledge as perception, true belief, and true belief with an
  account"). Test: nothing is lost if the sentence is replaced by the passage
  itself. A summary is still a retelling. A position attributed to an author
  that the author states in that place is R.
- `N` navigation: only says where something is ("In Book VII", "17d-18a").
  Becomes an address.
- `I` interpretation: a claim the text does not make itself - a reading, a
  reconstruction of an argument's structure or of an implicit premise, a
  distinction drawn by the reader, an evaluation, an implication, a comparison
  of readings, a hypothesis, what a place "shows" or "tests" when the text does
  not say it.
- `L` link: a connection between two specific places (same or different
  works): "the second passage sharpens the difficulty left by the first". Give
  both places and a relation kind (one of: repeats, varies, answers, develops,
  contrasts, echoes, recalls, cites, prepares, interprets).
- `Q` question. `M` method: an instruction for reading or comparing. `C`
  context: historical, biographical, editorial, lexical or bibliographic facts
  not in the text (an author's dates, a dictionary sense). `T` term or name
  only.

Mixed forms are split into sentences. If an `I`, `L`, `M` or `Q` sentence leans
on specific places, give those places as anchors too (they become its grounds).

## Anchors

- Corpus works: anchor in the original-language version (`grc`, or `la` for
  Latin works): `{"doc": "apology", "lang": "grc", "units": [unit ids in reading
  order], "quote": "exact substring of the first unit"}`. `units` is the
  smallest run of units holding what the sentence retells; `quote` narrows it to
  the exact words (a sentence or a clause, seldom more than three sentences;
  for a span over several units quote the start in the first unit). Never
  anchor in a translation. Copy quotes character for character from `zc.py
  show` output (breathings, accents, elisions such as `ʼ`).
- The Letter to Menoeceus and the Principal Doctrines are inside Diogenes
  Laertius X (`lives`, Книга 10): anchor them there.
- Works not in the corpus (Aristotle De Anima, Seneca's Letters, Sextus'
  Outlines of Pyrrhonism, Xenophon's Memorabilia, fragments of the
  Presocratics, anything else): `{"work": "Seneca, Epistulae morales", "locator":
  "1.1", "lang": "la", "quote": "exact original words if you know them for
  certain, else null"}` with the canonical locator (Stephanus, Bekker, DK
  number, book.chapter.section, letter.section).
- If you cannot find the place, say so in `note`; do not guess.

## What else to check

- `context.locator` is the address the stand shows. If the form talks about
  another place than the locator gives, flag `wrong-locator`.
- `context.sourceRefs` are the stand's links. Flag `wrong-locator: <url> ...`
  when a link leads to another book or section than the passage (for example a
  Republic link into Book VII for a passage of Book II), and `coarse-link: <url>`
  when it leads only to a whole work or a whole book where the passage is meant.
- Flags, any that apply: `"wrong-locator: <what and why>"`, `"coarse-link:
  <url>"`, `"misattribution: <...>"`, `"factual-error: <...>"` (the form says
  something the text does not say, mistranslates it or confuses speakers),
  `"copyrighted-source: <work>"` (a modern translation or study is retold or
  quoted), `"duplicate-of: <path>"`, `"corpus-defect: <...>"` (a gap, a wrong
  reference or garbled text in the corpus), `"self-contradiction: <...>"`.

## Output

Write `out/<your-slice>.jsonl` (in
`bonsai/review/anti/`),
one JSON object per input form, in input order:

```json
{"path": "...",
 "verdict": "replace | trim | keep",
 "sentences": [
   {"ru": "...", "en": "...", "class": "R",
    "anchors": [{"doc": "apology", "lang": "grc", "units": ["tos.text-unit.sid-..."], "quote": "..."}],
    "link": null,
    "note": ""}
 ],
 "flags": []}
```

- `verdict`: `replace` when every sentence is R/N; `trim` when R/N sentences sit
  beside I/L/Q/M/C/T; `keep` when there is no R/N.
- `sentences`: Russian and English split so that they correspond one to one
  (merge sentences if the two languages split differently). Keep the text
  exactly as authored: the sentences of each language joined with spaces must
  give back the form.
- `link` (for L): `{"kind": "develops", "from": anchor, "to": anchor}`.

Validate before you finish: `python3 validate.py <your-slice>` (run inside the
`anti/` folder) must report 0 problems. Report back in a few lines: counts per
verdict and class, the families you noticed (with two or three paths each),
anything you could not anchor, and every flag. Do not paste the JSONL.

Work steadily and keep memory low: run one corpus command at a time, never
several heavy commands in parallel.
