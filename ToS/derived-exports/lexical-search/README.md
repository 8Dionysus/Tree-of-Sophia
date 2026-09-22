# Lexical Search Projections

This directory exposes source-returning, rebuildable lexical read models. Source text, linguistic assessment, semantics, runtime search and canon follow
their corresponding owner routes.

The first projection covers the four DTA first-edition part witnesses of
*Also sprach Zarathustra*. Because the exact TEI rights records are still
unreviewed and conflicting, the tracked projection is deliberately
non-sequential and string-free: it carries form hashes, counts, and references
to tracked TEI page/division resources. The exact searchable SQLite/FTS5
projection remains in the pilot's gitignored `local-content/` route.

A form hash identifies a lexical-search key. Lexeme, lemma and sign identities
require their own source records. Dictionary recovery is possible for
low-entropy words, so these hashes offer limited confidentiality. Future public/site routing
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

Question-scoped exact context retains its plan, text-free receipt and
provenance in the source-owned lexical index; exact KWIC rows remain ignored
local evidence. The direct return route binds usage evidence to that local
source. Public export, lexeme, sign and sense assessment follow their
respective source and permission contracts.
