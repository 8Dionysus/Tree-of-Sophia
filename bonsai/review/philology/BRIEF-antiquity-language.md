# Brief: the language of the Antiquity stand

Tree of Sophia is a public research site on philosophy. Its Antiquity stand
(Plato, Aristotle, the Stoics, Epicurus, Cicero, Seneca, Sextus and others)
shows branches of questions, passages, readings and comparisons
(`bonsai/antiquity/`: `collection.json`, `first-branches.json` and the other
modules). The works speak through exact fragments in the original (Greek or
Latin) and in an aligned translation; the stand's own words remain for what a
quotation cannot give: readings, reconstructed arguments, distinctions,
connections between places, questions, methods, context
(`bonsai/review/anti/EDIT-BRIEF.md` tells how it came to be).

The owner reads these works closely. The Zarathustra stand has just had a
language review; the Antiquity stand has not, and its own words carry the same
family of faults. Your job is that review for Antiquity.

## What to correct

1. **Negations and contrasts the works do not make.** "X, а не Y", "не X, а Y",
   «не тождественно», «не просто», «не только … но и» used as padding, "rather
   than", "is not merely". Say what is there, affirmatively. Keep a negation
   only where the work itself makes it (Socrates denying that he teaches; a
   definition refuted in the text) and then say it in the work's terms.
2. **Filler and words about the site.** Sentences that say nothing a reader can
   use, hedging, words about data, nodes, routes, branches as data structures,
   "в словесном окружении", "формула узла".
3. **Terms.** Within the stand one Greek or Latin term reads with one Russian
   and one English word (ἀρετή, ψυχή, λόγος, ἐπιστήμη, δόξα, εὐδαιμονία,
   φρόνησις, ἡδονή, φύσις, νόμος, τέχνη, ἀνάμνησις, εἶδος/ἰδέα, ἀταραξία, ἐποχή,
   virtus, honestum, voluptas, …). Where the stand is split, take the word it
   uses most when it is sound for the term, and list every choice you made.
4. **Calques and wrong words.** Words a Russian or English philosophical reader
   would not use for the thing, translation calques, anachronisms.

Never change what a text claims. Fix the wording; when the English counterpart
carries the same fault, correct it in parallel. Write the way the owner asks
for philosophy: affirmative, exact, without explaining down.

## How

1. Collect every reader text of the stand: the `ru`/`en` strings of its JSON
   modules (quotations are citations and are not texts of the stand).
2. Review them in slices; for each text you change, record
   `{path, ru?: [old, new], en?: [old, new], kinds: [...], why}` in
   `bonsai/review/anti/language-corrections-2026-11-02.json`
   (`{schema: "tos.bonsai.language-corrections.v1", date, reviewer, basis, corrections}`).
   `path` is the JSON path of the form; one entry may name the same old text in
   several places through `also`.
3. Write `bonsai/review/anti/apply_language.py`: it applies the ledger to the
   modules by exact match of the old text at the path, refuses an entry whose
   old text is not there, and is idempotent. Apply it and commit the modules.
4. Add `access/web/src/public-research/antiquity-language.test.mjs`, the
   counterpart of `bonsai-language.test.mjs` for the Antiquity modules: it reads
   every reader text and fails on the patterns of the faults you corrected
   (negation turns, filler phrases, the term variants you normalized).

## Finish

All passing: `node bonsai/lib/cli.mjs check`, `cd access/web && npx vitest run`,
`cd access/web && npm run build:public-research`. Commit in clear steps and push
your branch through the repository's normal flow (the AoA pre-push hook and its
checkpoint; never `--no-verify`).

Write `~/.local/share/claude-codex/out/antiquity-language-summary.json`:
corrections by kind, the term choices (`{original, ru, en, before}`), and every
place where you hesitated. Print it at the end.
