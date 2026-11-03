# Brief: the terms of Zarathustra and the English that names them

Tree of Sophia is a public research site on philosophy. Its Zarathustra stand
shows Nietzsche's book with its German original (first editions), Thomas
Common's English (1909) and Yu. M. Antonovsky's Russian (1913), aligned row by
row (`bonsai/zarathustra/witnesses/`), and reads it through a philosophical core
(`bonsai/zarathustra/core/`: concepts, figures, links, chains, voices,
occurrences) whose texts are authored in Russian and English.

The owner reads the book closely and has learned it by heart. Word choice is the
core problem of the stand, from translation up to philosophy. Today the core's
English names the Übermensch "Overman" while Common's text beside it says
"Superman"; the English is split between "eternal recurrence" and "eternal
return", "creator" and "the creating one"; Russian once said «домовые» where
Antonovsky says «горные духи». Read `bonsai/review/philology/README.md` first:
it defines the module you build.

## A. The terms module

Write `bonsai/zarathustra/terms.json` (schema `tos.bonsai.terms.v1`) and declare
it in `bonsai/zarathustra/bonsai.json` as the module `terms` with a `role` in
Russian and English like the other modules.

- **Which terms.** The words that carry the book's teaching and the core's
  reading: the concepts and figures of the core whose name is a word of the book
  (`core/concepts.json`, `core/figures.json`), and the recurring words the core's
  texts use as terms. Include at least: Übermensch, ewige Wiederkunft, der letzte
  Mensch, der grosse Mittag, Wille zur Macht, Geist der Schwere, die schenkende
  Tugend, Seiltänzer, Possenreisser, der Schaffende, die Mitschaffenden,
  Untergang / untergehen, Selbst-Überwindung, die höheren Menschen, Augenblick,
  Thorweg, Kobolde, Hinterweltler, Verächter des Leibes, Taranteln, Mitleid,
  Ekel, Erlösung, «Es war», Zufall, Kameel / Löwe / Kind, «Gott ist todt»,
  Prediger des Todes, Fliegen des Marktes, Fernsten-Liebe, Ja-sagen. Fifty to
  ninety terms in all. Spell the German as the witness spells it.
- **First place.** The first place in the book's order where the original uses
  the word as this term, cited exactly as the core cites (`w`, `c`, `u`, `s`,
  `e`, `q`). Use the units and offsets of `bonsai/zarathustra/witnesses/<chapter>.json`.
- **Witness words.** Read Antonovsky's and Common's counterpart of the first
  place (the rows and sentence pairs of the chapter) and record the word each
  uses there, with the forms that translation uses across the book.
- **The Tree's words.** Russian: Antonovsky's word, with every inflected form
  the core's Russian texts use. English: the Tree's own translation of the term
  (the owner's decision: the Tree keeps its own English for terms; Common's word
  is shown on its origin). Take the word the core's English already uses most
  when it is sound English for the German and faithful to it; where the core is
  split, choose one and say why in `basis`. Give every form the core's English uses.
- **Avoid.** English: Common's word where the Tree reads its own, and the split
  variants you did not choose. Russian: words that are not Antonovsky's for the
  term (the patterns in `access/web/src/public-research/bonsai-language.test.mjs`
  are earlier cases).
- **Gloss.** One sentence per language on what the word does in the book.
  Write it affirmatively and in the book's own words where they serve: say what
  is there. No "not X but Y", no hedging, no words about data, nodes or routes.
- `concept`: the core id (`overman`, `recurrence`, `dwarf`, …) when there is one.

`node bonsai/lib/cli.mjs check` verifies every first place and every witness word.

## B. The core's English and Russian name every term with the Tree's word

1. Find every reader text of the core (`ru`/`en` strings in the core's JSON
   files; the `en` of a record together with its `ru`) that names a registered
   term with a word from its `avoid` list or with a variant that is not the
   Tree's word.
2. Write the corrected whole text for each into
   `bonsai/review/zarathustra-graph/graph-words/assembly-inputs/term-corrections-2026-11-02.json`:
   `{schema: "tos.bonsai.language-corrections.v1", date, reviewer, basis,
   corrections: [{ru?: [old, new], en?: [old, new], term, why}]}`. Change only
   the word and what grammar needs around it; the sentence keeps its sense.
3. Apply it through the assembler: the corrections step of
   `bonsai/review/zarathustra-graph/graph-words/assemble.py` today reads
   `language-corrections-2026-11-02.json` keyed by the Russian text. Make it read
   every ledger listed in `assembly-inputs/corrections.json` (keep the existing
   one first), and apply an English-only entry wherever a record's `en` equals
   the old English. Count them in `core.json` like the existing counts. Replay
   the assembler as `graph-words/README.md` describes and commit the
   regenerated core. Do not edit the core's JSON by hand.
4. Extend `bonsai-language.test.mjs`: it reads `bonsai/zarathustra/terms.json`
   and fails when a Russian reader text uses a Russian `avoid` word or an English
   reader text uses an English `avoid` word (whole words; the English check is
   case-sensitive only where the avoided form is).

## Finish

All passing: `node bonsai/lib/cli.mjs check`, `cd access/web && npx vitest run`,
`cd access/web && npm run build:public-research`. Commit in clear steps and push
your branch through the repository's normal flow (the AoA pre-push hook and its
checkpoint; never `--no-verify`).

Write `~/.local/share/claude-codex/out/terms-summary.json`: every term as
`{id, de, ru, en, en_witness, en_basis}`, the number of corrections by language,
and every place where you hesitated. Print it at the end.
