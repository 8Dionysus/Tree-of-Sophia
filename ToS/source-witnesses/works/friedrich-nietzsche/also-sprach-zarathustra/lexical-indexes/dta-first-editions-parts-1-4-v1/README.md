# DTA Parts 1–4 Lexical Observation Index v1

This route owns the first whole-work lexical observation plan for the
Zarathustra foundation. It reads the four fixity-bound DTA TEI payloads for
the first public editions of parts 1–4.

The plan declares two deliberately different outputs:

- a local gitignored SQLite/FTS5 database with exact forms, normalized search
  keys, page-local token order, source context, structural filters, and opaque
  occurrence IDs;
- a generated source-withholding projection at the logical output path
  `ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json`,
  materialized through the selected data artifact. This software checkout tracks
  the plan and provenance, not the `.min.json` bytes.

The projection does not carry exact strings, token sequence, snippets, contexts,
or occurrence positions. Its low-entropy hashes are navigational fingerprints,
not confidentiality. Public/site routing remains blocked until the source
rights records receive an explicit review.

This route observes forms in an exact digital witness. It does not accept the
German text, establish a critical edition, create a lexeme or lemma, infer a
phrase, nominate a sign, open the initial semantic packet, or authorize
publication. Frequency is never semantic sufficiency.

The first downstream observation is frozen in
[`recurrence-plan.v1.json`](recurrence-plan.v1.json). Its declared logical
output path in the selected data artifact is:

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

The plan is [`index-plan.v1.json`](index-plan.v1.json). The maintained
producer is the native `tos-research lexical-index` command. Its `build` action
writes a fresh private candidate from an explicit source cut; `validate` checks
that candidate, while `validate-legacy` checks a retained legacy projection.
The prior Python builder bytes are retained at
`ToS/research-packets/retained-builder-inputs/build_zarathustra_lexical_index/69e6475b17774b2aa41ab4be77050b6ada04ecba67cd8833c25b6d62e27502a2.py`;
the earlier historical bytes remain at
`ToS/research-packets/retained-builder-inputs/build_zarathustra_lexical_index/44bb4f9ad6f3bcc76d877366a87efb77f0eacb6cc999eec0f461ab5f35c37372.py`.
Provenance keeps the original [historical lexical builder](https://github.com/8Dionysus/Tree-of-Sophia/blob/c04257b4f2270587856ac94d3fb28a5a9d5afa25/scripts/build_zarathustra_lexical_index.py)
logical reference. Retained bytes are not executable fallbacks. The declared
source-withholding output path is not cleared for publication:

`ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json`

The local database belongs at:

`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3`

The implementation and explicit build/validate requirements are owned by the
[native compiler route](../../../../../../../rust/crates/tos-compiler/README.md)
and [derived-export route](../../../../../../derived-exports/README.md). The
read-only source/projection closure command is:

```bash
tos-research lexical-index validate-tracked \
  --source-root /absolute/selected-source \
  --derived-input-root /absolute/selected-data \
  --local-output-root /absolute/private-artifacts
```

This closes the selected source, generated controls, and optional private
SQLite bytes. The repository route is the `source_witness_foundation` lane in
[ToS validation](../../../../../../VALIDATION.md); the lexical check is also an
explicit step in that lane manifest.

The first downstream experiment is deliberately narrower than lexical
promotion. The source-gated plan and text-free materialization receipt live at:

- `gold-sets/foundation-pilot-v1/morphology-evaluation-plan.v1.json`;
- `gold-sets/foundation-pilot-v1/morphology-input-receipt.v1.json`.

They admit an exhaustive direct-form DWDSmor coverage census only. The exact
input stays ignored, and contextual A/B/C, German acceptance, lemma/lexeme
promotion, signs, and semantics remain separate blocked stages.

The maintained downstream commands are `tos-research zarathustra-recurrence-projection`
and `tos-research zarathustra-usage-context`. Select an absolute `--source-root` and
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

`tos-research semantic-source-recurrence` returns the complete selected recurrence to
the four fixity-bound raw TEI witnesses. `tos-research zarathustra-morphology-context`
freezes the planned first, inclusive-median and last contexts; select the
retained provider stream with `--a-raw-output ABS`. Both commands require
explicit source, private input and output roots and support the same retained
`--check` and fresh generation modes above. They run no morphology provider.
The context selection remains output-blind; private source strings and
positions remain in the mode-`0600` packet.
