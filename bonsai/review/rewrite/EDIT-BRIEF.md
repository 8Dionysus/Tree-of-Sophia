# Own words after the fragment: the last edit of the Zarathustra stand's texts

## Situation

The Zarathustra stand of Tree of Sophia (a public research site on Nietzsche's
*Thus Spoke Zarathustra* and the thought of eternal recurrence) no longer
retells the book. Every sentence that only said what the book (or another
work) says was replaced by an exact fragment of the work: the reader now sees
the cited place in German (the first editions) and in their own language
(Antonovsky's Russian of 1913, Common's English), opened in the whole work. Our
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

The owner knows the book by heart; be exact, and change as little as possible.

## Input

`edit-slices/<your slice>.json`: a list of items, each with

- `path`: where the form lives (module object and field).
- `verdict`: `trim` (retelling cut, own words kept) or `keep` (unchanged so far,
  but flagged).
- `original`: the form before the cut (Russian and English).
- `sentences`: the original sentences with their class (R retelling, N
  navigation, I interpretation, L link, Q question, M method, C context, T term)
  and the classifier's notes.
- `kept`: the own words as they stand now (`ru`, `en`) and the citations.
- `shows`: what the reader sees in place of the cut sentences, in order: the
  cited place in German, Russian and English (shortened with " …" when long;
  " ¶ " separates paragraphs). For other works it names the place only.
- `flags`: `factual-error: …`, `misattribution: …`, `wrong-locator: …`,
  `self-contradiction: …`, `per-language-paragraph-numbers`, `duplicate-of: <path>`,
  and so on.

The German text and the translations are in
`bonsai/zarathustra/witnesses/<chapter>.json`
(`units.de|en|ru` = [id, kind, text, section]); you may read them to check a
claim. The corpus helper
`bonsai/review/frag/zc.py`
(`python3 zc.py find za de "…"`, `show za de 1821 1880`) shows the public
German and English texts with ordinal numbers, which the flags use ("#2644").

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
5. The chapter "Vom Gesicht und Räthsel" is «О видении и загадке» / "The Vision
   and the Enigma" in our own words (Antonovsky's own title «О призраке и
   загадке» appears only inside his text). Remove editorial leftovers such as
   «Авторский вопрос к этой перспективе:».
6. If nothing of the own words should remain (everything left was a hanging
   remnant), return empty strings: the fragments alone then stand.
7. Never touch the citations; if one looks wrong, say so in `why`.

## Output

Write `edit-out/<your slice>.jsonl` (in
`bonsai/review/rewrite/`),
one line per input item, in input order:

```json
{"path": "...", "ru": "...", "en": "...", "changed": true, "why": "one short line, English"}
```

`changed` is false when the kept own words stand as they are. Validate with
`python3 check_edits.py <your slice>` (in that folder) until it reports 0
problems. Report back in a few lines: how many changed, what kinds of changes,
every flag you could not resolve and every citation that looked wrong.
