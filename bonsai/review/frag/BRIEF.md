# Pass over the Zarathustra stand: retellings versus what only the Tree can say

## Why

Tree of Sophia must not keep a second, retold copy of a work. Whatever the work
itself says is shown by the work: an exact fragment with its address, opened in
the whole work. Our own words are only for what a quotation cannot give: a
reading or interpretation, a reconstructed thesis or argument and its steps, an
objection aimed at a specific step, a distinction, a connection between two
places or two works, historical or editorial context, a question, a method of
reading. Many node texts of the Zarathustra stand instead retell the book
"in slightly different words". Your job is to find every such sentence in your
slice, say exactly which place of the German original it retells, and keep
everything that is genuinely the Tree's own.

The owner knows Thus Spoke Zarathustra by heart. Be exact.

## Your input

- `slices/<your-slice>.json`: a list of authored forms. Each has `module`,
  `path` (where it lives), `ru` (authored Russian) and `en` (the English
  version of the same form). Atlas-content items also carry `node`.
- `context/nodes.json` (node id -> kind, cluster, titles), `context/edges.json`
  (relation id -> from, to, kind), `context/ground-targets.json` (paragraph
  units already chosen by hand for chapters, node grounds and route steps;
  useful hints, not authority).
- `zc.py`: corpus helper. Run `python3 zc.py --help`. Useful:
  - `python3 zc.py chapters za de` - German chapters with paragraph ordinals.
  - `python3 zc.py show za de 1821 1880` - full German paragraphs with unit ids.
  - `python3 zc.py find za de "Thorweg Augenblick" --chapter Gesicht`
  - `python3 zc.py find za en "gateway" --chapter VISION` (English, Common).
  - `python3 zc.py check <unit-id> "exact German quote"` - verifies a quote.
  - Other works in the corpus: `python3 zc.py docs`; e.g. Epictetus'
    Encheiridion is `ench` (grc, en, es), Marcus Aurelius `med` (grc).
  Key German chapters (ordinals in `za de`): Vorrede 24-195; Von den drei
  Verwandlungen 197-224; Auf den glückseligen Inseln 927-962; Von den
  Mitleidigen 963-1005; Von der Erlösung 1641-1694; Vom Gesicht und Räthsel
  1821-1880; Der Genesende 2585-2658; Das andere Tanzlied 2693-2794 (this
  public German text lacks the heading of "Die sieben Siegel", whose units sit
  at the end of this range); Das Nachtwandler-Lied (= Das trunkne Lied)
  3582-3665; Das Zeichen 3666-3746. English (Common) has the same chapters;
  units after 3936 in English are the editor Ludovici's notes, never anchor
  there.

Read the German passages before you judge. A sentence that "sounds like
interpretation" can still repeat the text: "Формула не обещает улучшенного
второго издания жизни" repeats "nicht zu einem neuen Leben oder besseren Leben
oder ähnlichen Leben" from Der Genesende.

## Classes, per sentence

- `R` retelling: states what the work itself says, narrates, depicts or has a
  character say: events, images, speeches, their order, summaries of a scene or
  chapter, even abstract ones ("The vision begins with ascent and the dwarf's
  weight, passes through questions about the Moment and leads to the
  shepherd's bite"). Test: nothing is lost if the sentence is replaced by the
  passage itself. A summary is still a retelling.
- `N` navigation: only says where something is ("In Prologue §1", "Section
  2:", "RU paragraphs 48-57; EN 49-58"). Becomes an address.
- `I` interpretation: a claim the text does not make itself - a reading, a
  reconstruction of a thesis or argument, a distinction drawn by the reader, an
  evaluation, an implication, a comparison of readings, a hypothesis.
- `L` link: a connection between two specific places (same work or different
  works): "the later 'I bit' in The Convalescent repeats the shepherd's act".
  Give both places and a relation kind (one of: repeats, varies, answers,
  develops, contrasts, echoes, recalls, cites, prepares, interprets).
- `Q` question.
- `M` method: an instruction for reading or comparing ("Compare...", "Trace...").
- `C` context: historical, biographical, editorial or bibliographic facts that
  are not in the text.
- `T` term or name only.

Mixed forms are split into sentences. If an `I`, `M` or `Q` sentence leans on
specific places, give those places as anchors too (they become its grounds).

## Anchors

- Zarathustra: anchor only in the German original, `{"doc": "za", "lang":
  "de", "units": [unit ids in reading order], "quote": "exact German
  substring" or null}`. `units` is the smallest run of paragraphs that holds
  what the sentence retells; `quote` narrows it to the exact words when they
  are less than the whole paragraph (it must pass `zc.py check` against one
  unit; for a span over several units give the quote of the first unit or
  null). Never anchor in English or Russian; other languages will follow
  through alignment.
- Other works: anchor in the original language when the work is in the
  corpus (`{"doc": "ench", "lang": "grc", "units": [...], "quote": ...}`);
  otherwise `{"work": "Spinoza, Ethics", "locator": "III P6 and demonstration",
  "lang": "la"}` with the canonical locator (DK number, Stephanus, Bekker,
  book/section, aphorism number).
- If you cannot find the place, say so in `note`; do not guess.

## Output

Write `out/<your-slice>.jsonl`, one JSON object per input form, in input
order:

```json
{"module": "...", "path": "...",
 "verdict": "replace | trim | keep",
 "sentences": [
   {"ru": "...", "en": "...", "class": "R",
    "anchors": [{"doc": "za", "lang": "de", "units": ["tos.text-unit.sid-..."], "quote": "..."}],
    "link": null,
    "note": ""}
 ],
 "flags": []}
```

- `verdict`: `replace` when every sentence is R/N (the form is a retelling and
  the fragment takes its place); `trim` when R/N sentences sit beside
  I/L/Q/M/C; `keep` when there is no R/N.
- `sentences`: Russian and English split so that they correspond one to one
  (merge sentences if the two languages split differently). Keep the text
  exactly as authored.
- `link` (for L): `{"kind": "repeats", "from": anchor, "to": anchor}`.
- `flags`, any that apply: `"per-language-paragraph-numbers"`,
  `"wrong-locator: <what and why>"`, `"misattribution: <...>"`,
  `"factual-error: <...>"`, `"copyrighted-source: <work>"`,
  `"duplicate-of: <path>"` (same content as another form you saw, also across
  nodes), `"corpus-defect: <...>"` (e.g. a typo or missing heading in the
  public German text), `"self-contradiction: <...>"`.

Validate before you finish: every line is JSON, every unit id exists, every
non-null quote passes `zc.py check`. Report back in a few lines: counts per
verdict and class, the families you noticed (with two or three paths each),
anything you could not anchor, and every flag. Do not paste the JSONL.
