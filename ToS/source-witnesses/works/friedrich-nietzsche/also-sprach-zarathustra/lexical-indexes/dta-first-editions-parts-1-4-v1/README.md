# DTA Parts 1–4 Lexical Observation Index v1

This route owns the first whole-work lexical observation plan for the
Zarathustra foundation. It reads the four fixity-bound DTA TEI payloads for
the first public editions of parts 1–4.

The index has two deliberately different outputs:

- a local gitignored SQLite/FTS5 database with exact forms, normalized search
  keys, page-local token order, source context, structural filters, and opaque
  occurrence IDs;
- a tracked generated projection containing only SHA-256 form keys,
  aggregate counts, and page/division resource references.

The tracked projection does not carry exact strings, token sequence, snippets,
contexts, or occurrence positions. Its low-entropy hashes are navigational
fingerprints, not confidentiality. Public/site routing remains blocked until
the source rights records receive an explicit review.

This route observes forms in an exact digital witness. It does not accept the
German text, establish a critical edition, create a lexeme or lemma, infer a
phrase, nominate a sign, open the initial semantic packet, or authorize
publication. Frequency is never semantic sufficiency.

The plan is `index-plan.v1.json`. The generated source-withholding companion
(not cleared for publication) is:

`ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json`

The local database belongs at:

`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3`

Rebuild and validation commands are documented in `scripts/AGENTS.md`.

The first downstream experiment is deliberately narrower than lexical
promotion. The source-gated plan and text-free materialization receipt live at:

- `gold-sets/foundation-pilot-v1/morphology-evaluation-plan.v1.json`;
- `gold-sets/foundation-pilot-v1/morphology-input-receipt.v1.json`.

They admit an exhaustive direct-form DWDSmor coverage census only. The exact
input stays ignored, and contextual A/B/C, German acceptance, lemma/lexeme
promotion, signs, and semantics remain separate blocked stages.
