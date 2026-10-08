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

The first release-safe downstream observation is frozen in
`recurrence-plan.v1.json`. Its generated companion is:

`ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-recurrence-v1.min.json`

For every exact-form hash it keeps three views over the same evidence:

- A: absolute occurrence frequency;
- B: range across parts, TEI divisions, and pages;
- C: the A+B tuple plus part-size-aware `DP`, maximum part concentration, and
  declared source-editorial/unsectioned residue.

The projection remains ordered by exact-form hash. It has no rank, fused
importance score, stopword judgment, source string, morphology, lemma, lexeme,
sign candidate, or semantic effect. Its provenance is
`recurrence-provenance.jsonl`; the research and calculation decision is in
`ToS/research-packets/foundation-laboratory-2026-07/EXACT_FORM_RECURRENCE_FOUNDATION_RESEARCH.md`.
The content-free initial sign packet stays unchanged.

The first question-scoped usage layer is separately frozen in
`usage-context-plan.v1.json`. It does not sample a recurrence winner. It uses
one pre-existing Work-identity control and materializes all 527 of its exact
occurrences as fixed 24-token, same-page KWIC rows. The source-bearing JSONL
stays mode `0600` under the pilot's ignored `local-content/usage-context/`
route. `usage-context-receipt.v1.json` and
`usage-context-provenance.jsonl` retain only digests, counts, source-state and
selector closure, rights posture, and authority limits; they carry no exact
string, sequence, context, or occurrence position. The baseline claims no
sentence or sense boundary, creates no linguistic or semantic entity, opens
no human backlog, and leaves `initial-sign-packet.v3.json` unchanged.

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

The maintained downstream commands are `tos zarathustra-recurrence-projection`
and `tos zarathustra-usage-context`. Select an absolute `--source-root` and
`--local-output-root`; usage context also requires `--local-input-root` for the
private database. `--check` reconstructs the complete retained result and
preserves its original provenance. Recurrence checks select an output root
containing the retained projection and provenance; usage checks select the
private packet root, with tracked receipt and provenance in the source root.

For a new generation, use `--build --generation NAME --event-at RFC3339
--scratch-bytes RESERVED_BYTES` with a separate output root. Reserve that
space through the host storage owner first. Generated names retain the
historical basename and add `.native-NAME`; the receipt records the Rust
generator. Replaying a matching generation preserves its files and completes
missing outputs. Conflicting bytes, wrong file modes and symlinks refuse.
The private usage packet remains mode `0600`; tracked metadata stays `0644`.

`tos semantic-source-recurrence` returns the complete selected recurrence to
the four fixity-bound raw TEI witnesses. `tos zarathustra-morphology-context`
freezes the planned first, inclusive-median and last contexts; select the
retained provider stream with `--a-raw-output ABS`. Both commands require
explicit source, private input and output roots and support the same retained
`--check` and fresh generation modes above. They run no morphology provider.
The context selection remains output-blind; private source strings and
positions remain in the mode-`0600` packet.
