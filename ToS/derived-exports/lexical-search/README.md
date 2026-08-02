# Lexical Search Projections

This directory exposes source-returning, rebuildable lexical read models. It
does not own source text, linguistic acceptance, semantics, runtime search, or
canon.

The first projection covers the four DTA first-edition part witnesses of
*Also sprach Zarathustra*. Because the exact TEI rights records are still
unreviewed and conflicting, the tracked projection is deliberately
non-sequential and string-free: it carries form hashes, counts, and references
to tracked TEI page/division resources. The exact searchable SQLite/FTS5
projection remains in the pilot's gitignored `local-content/` route.

A form hash is not a lexeme, lemma, sign, or secrecy mechanism. Dictionary
recovery is possible for low-entropy words. Future public/site routing
therefore remains independently blocked even though no source sequence or
context is tracked here.

`zarathustra-dta-first-editions-parts-1-4-recurrence-v1.min.json` is a second,
fully rebuildable hash-only read model over that lexical projection. It keeps
frequency, structural range, part-size-aware `DP`, maximum part concentration,
and explicit residue as separate fields for all 11,352 forms. It reads no
payload or private database and introduces no score, ranking, accepted German,
linguistic identity, sign proposal, semantic claim, public route, or human
task. Its frozen plan and provenance remain source-owned beside the lexical
index.

Question-scoped exact context does not become a third tracked lexical export.
Its plan, text-free receipt, and provenance stay with the source-owned lexical
index, while exact KWIC rows remain ignored local evidence. This preserves a
direct return route without turning source sequence into a reusable public
read model or confusing usage evidence with a lexeme, sign, or sense.
