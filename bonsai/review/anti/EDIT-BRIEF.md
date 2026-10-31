# Own words after the fragment: the last edit of the Antiquity stand's texts

## Situation

The Antiquity stand of Tree of Sophia (a public research site on ancient
philosophy: Plato, Aristotle, the Stoics, Epicurus, Cicero, Seneca, Sextus and
others, with branches of questions, passages, readings and comparisons) no
longer retells the works. Every sentence that only said what a work says was
replaced by an exact fragment of it: the reader now sees the cited place in
the original (Greek or Latin) and, where the public corpus has one, in a
translation aligned with it (mostly English; Russian and Spanish for the
Apology and the Crito), opened in the whole work. Our
own words remain only for what a quotation cannot give: readings, reconstructed
arguments, distinctions, connections between places, questions, methods,
context.

The removal was mechanical: in each form below, the retelling sentences were
cut and their places became citations; the remaining sentences are exactly as
authored. Some no longer read well: they point back with "this", "here", "he"
to a sentence that is gone, start with "And", or repeat what the fragment now
shows. Some forms also carry flags from a close reading (factual errors,
misattributions, duplicates). Your job is the last edit: make each form's own
words read right **after its fragments**, and correct what the flags name.

The owner reads these works closely; be exact, and change as little as possible.

## Input

`edit-slices/<your slice>.json`: a list of items, each with

- `path`: where the form lives in `bonsai/antiquity/collection.json` or
  `first-branches.json`; `also` lists other paths with the identical text,
  which take your edit too.
- `verdict`: `replace` (the whole form became citations: return empty strings,
  `changed` false), `trim` (retelling cut, own words kept) or `keep` (unchanged so far,
  but flagged).
- `original`: the form before the cut (Russian and English).
- `sentences`: the original sentences with their class (R retelling, N
  navigation, I interpretation, L link, Q question, M method, C context, T term)
  and the classifier's notes.
- `kept`: the own words as they stand now (`ru`, `en`) and the citations.
- `shows`: what the reader sees in place of the cut sentences, in order: the
  cited place in the original and in English where the corpus has it
  (shortened with " …" when long). For works outside the corpus it names the
  place only.
- `flags`: `factual-error: …`, `misattribution: …`, `wrong-locator: …`,
  `self-contradiction: …`, `per-language-paragraph-numbers`, `duplicate-of: <path>`,
  and so on.

The corpus helper
`bonsai/review/frag/zc.py`
(`python3 zc.py find apology grc "…"`, `show apology grc 5 6`, `show apology en 5 6`)
shows the originals and translations with unit ordinals and references; read
them to check a claim before you correct it.

## Rules

1. The fragments come first, then the own words. Own words must read right
   after them: resolve pronouns and "here/this" that pointed at a cut sentence
   (name the thing instead), drop a leading "And/But" that hangs, and remove any
   remnant that only repeats what the fragment shows.
2. Change as little as possible. Do not add claims, readings or examples; do
   not polish style for its own sake. Keep the author's terms.
3. Russian and English must say the same thing.
4. Flags:
   - `factual-error`, `misattribution`, `self-contradiction`, `wrong-locator`
     (when the own words name a place): correct the own words with the
     smallest change that makes them true to the text. If the error lived only
     in a cut sentence, nothing is left to fix; say so.
   - `per-language-paragraph-numbers`: own words must not name paragraph
     numbers of one translation or edition ("RU paragraphs 48–57"); remove
     them (the citation now gives the place for every language).
   - `duplicate-of: <path>`: if the own words say the same as the other form's
     own words, keep them only in the form where they belong (the more specific
     node or relation) and empty them here, unless that would leave a required
     field empty with no fragment (then keep the shortest form).
5. Remove editorial leftovers (labels such as «Сравните:» left without what
   followed, notes to the editor). A form whose own words were a summary of a
   whole work or a section (a work map stage, an author's body) may well end
   empty: the cited places stand for it.
6. If nothing of the own words should remain (everything left was a hanging
   remnant), return empty strings: the fragments alone then stand.
7. Never touch the citations; if one looks wrong, say so in `why`.

## Output

Write `edit-out/<your slice>.jsonl` (in
`bonsai/review/anti/`),
one line per input item, in input order:

```json
{"path": "...", "ru": "...", "en": "...", "changed": true, "why": "one short line, English"}
```

`changed` is false when the kept own words stand as they are. Validate with
`python3 check_edits.py <your slice>` (in that folder) until it reports 0
problems. Report back in a few lines: how many changed, what kinds of changes,
every flag you could not resolve and every citation that looked wrong.
