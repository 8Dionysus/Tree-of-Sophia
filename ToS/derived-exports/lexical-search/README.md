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
