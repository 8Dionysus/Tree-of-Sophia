# Foundation Pilot v1

This directory freezes the first source-visible laboratory slice for the
Zarathustra golden kernel. It is not a completed gold set.

## Frozen before outputs

- `sample-plan.json` fixes 36 units: 12 from each of the three acquired source
  items. It assigns exactly five gold candidates per source before any A/B/C
  result is inspected.
- `anchors.jsonl` binds those units to immutable file digests and either PDF
  page regions or EPUB members.
- `ocr-visual-samples.json` projects that frozen set into 36 visual OCR units:
  all 24 selected Russian PDF pages and 12 pages from the exact Naumann 1893
  parent scan. The only replacement is the nonvisual EPUB generator notice,
  replaced by PDF page 2 before any OCR output was visible.
- `ocr-anchors.jsonl` binds the 12 German visual units to the parent scan's
  immutable PDF digest. The projection fixes Poppler 26.01.0, 300 DPI, RGB
  full-page PNG rendering without crop, deskew, denoise, thresholding, or
  contrast adjustment; every variant must consume the same digest-frozen
  renders.
- `translation-samples.json` fixes 30 German fragments across the work before
  any translation draft. The recognized Antonovsky witness is sealed until
  independent human, AI, and AI+human drafts have been frozen.
- `translation-anchors.jsonl` records the proposed source selectors. The
  sentence boundary and German transcription still require source-visible
  human acceptance.
- `translation-source-review-plan.v2.json` freezes the successor method before
  any v2 candidate text: previous/current/next scan pages, layout posture,
  source-selection instruction, v1 failure lineage, sealed comparator, and
  two real-human passes. It forbids reuse of the rejected v1 candidate text.
- `german-assisted-source-review.v1.json` adds a solo+AI route without
  inventing German competence. It separates visual human evidence,
  critical-edition witnesses, independent machine candidates, sourced AI
  explanations, and later competent-human acceptance. It currently selects
  one unit for one machine-only triangulation run, creates zero human debt,
  accepts zero German units, and opens no translation lane.
- `critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json` freezes one exact
  eKGWB section locator for the first prepared German unit. It stores no
  eKGWB text, records the failed direct-fetch paths, and binds the locator to
  the fixity-verified local EPUB/PDF section boundary. That evidence proves
  only structural compatibility: the critical passage itself was not fetched
  or compared. Bibliographic and rights review remain pending, no human task
  is created, and no German unit or translation lane is admitted.
- `german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.v1.json`
  records the first bounded machine-only comparison. The local eKGWB response
  and DTA part-I section agree on all 12 paragraph token sequences and 261
  normalized tokens after source-aware DTA dehyphenation; the Naumann
  automatic EPUB agrees on 260 reference tokens and retains one OCR
  replacement plus page furniture. The tracked packet contains no source text.
  Unencrypted HTTP, rights, bibliography, language competence, and all
  translation/semantic gates remain explicitly open.
- `bounded-translation-research-input.za-i-vorrede-1-opening-sentence.v1.json`
  admits one deterministically selected 20-token DTA sentence to local blind
  machine-method calibration. Its source-bearing companion is gitignored; the
  tracked packet contains only identity, selector, transformation, digest,
  count, corroboration, rights, and gate evidence. The existing accepted
  translation plan remains at 0 of 30 accepted German units with every draft
  lane blocked. The recognized comparator and the pre-existing authored
  Russian/English canon surfaces are all sealed from calibration candidates.
- `provenance.bounded-translation-calibration.jsonl` records the closed
  private A/B/C machine-method run without copying source or candidate text.
  A returned a contract-complete but Russian-defective candidate, B retained a
  truncated invalid output, and C retained a GPU runtime failure with no
  candidate. A separate source-free diagnosis establishes the i915
  `GPU HANG` and context reset that preceded OVMS `CL_OUT_OF_RESOURCES`; it
  does not establish root cause or repair C. All accepted German, translation,
  semantic, graph, and canon effects remain zero.
- `translation-laboratory-plan.v1.json` freezes the full 17-stage
  source-to-adjudication order, model candidate posture, evaluation axes,
  separate personal read-aloud layer, and comparator reveal gate while current
  human source acceptance remains exactly 0 of 30.
- `translation-reference-register.v1.json` records 14 dated dictionary,
  corpus, critical-edition, Nietzsche-lexical, Russian-translation, and
  English-translation candidates. It preserves version, scholarly role,
  access, rights, citation, contact, and admission separately; all content
  admission and human bibliographic/rights review counts remain zero.
- `HUMAN_TRANSLATION_PRE_DRAFT_WORKSHEET.md` is a content-free interface for a
  future real-human-only morphology-to-interlinear pass. It forbids AI aids,
  other-lane analysis, comparator exposure, and premature drafting; it does
  not claim that any human work has occurred.
- `INITIAL_SIGN_PACKET_ROUTE.md` and `initial-sign-packet.v3.json` preserve the
  fifteen-stage form-to-graph route as a tracked
  `blocked-not-materialized` packet. It contains no source form, occurrence,
  lexeme, sign candidate, human task, relation, concept, claim, or graph
  projection. Its gate is one packet-local accepted German source bundle plus
  language-competence evidence; the historical 0-of-30 packet does not
  schedule this work.
- the separate DTA parts 1–4 lexical observation route materializes 86,287
  exact token occurrences across 506 body pages and 213 TEI divisions in one
  deterministic ignored SQLite/FTS5 database. Exact, normalized, prefix,
  phrase, section, page, language, and edition probes pass locally. Its
  tracked 11,352-row companion carries only form hashes, aggregate counts, and
  resource refs; it carries no source strings, sequence, context, or
  occurrence positions. Lemma, translation, sign candidate, sign, source
  acceptance, rights clearance, and public routing remain zero or blocked,
  and the initial sign packet stays unchanged.
- `morphology-evaluation-plan.v1.json` turns that exact-form floor into one
  bounded question without pretending that all morphology is ready. Its
  ignored 11,352-row input packet covers every exact form and represents
  86,287 token occurrences; the tracked receipt exposes only fixity and
  counts. The direct DWDSmor Open 0.18.0 A census has now executed twice over
  the same exact-form order with a byte-identical provider-output stream. Its
  tracked result receipt contains no source or provider-lemma strings: 6,610
  of 11,352 form types and 75,872 of 86,287 token-weighted occurrences
  received at least one provider analysis; 4,742 types with weight 10,415
  remain mechanically unknown. Coverage is not accuracy, the residue has not
  been reviewed, and German-competent gold remains zero. A contextual A/B/C
  sample therefore remains unmaterialized until reviewed A residue or a
  concrete source/translation/sign/retrieval question triggers it; B and C
  remain unacquired, and no accepted morphology, lemma, lexeme, semantic
  effect, or human backlog is created.
- `transfer-samples.json` closes the stack suite's cross-work sample reference
  with a source-gated `blocked-not-run` plan. Its three Mysl pages are
  title-page scouting boundaries only; all are ineligible for semantic
  transfer. A real run requires 30 accepted German units, 15 double-checked
  kernel gold units, accepted sign and translation packets, and 20
  double-checked content-bearing target passages. `transfer-provenance.jsonl`
  binds that non-run decision to the current source and gold gates.
- `semantic-samples.json` uses the task-specific semantic v2 contract while
  retaining the universal 30-source/15-gold gate only as a digest-bound
  historical snapshot without scheduling or execution authority.
  `llm-tasks.json` uses the separate task-specific LLM v2 contract. Both
  remain `blocked-not-materialized`, contain zero tasks and zero runs, and
  keep their own provenance and readiness law.
- `llm-tasks.json` freezes one content-free
  `sign-candidate-and-refusal` question, its output boundary, the
  10-random/10-hard shape, and the `abyss-stack` resident-profile route. It
  still contains no anchor, source text, task instance, model output, human
  task, result, or promotion. Its next gate is exactly twenty task-specific
  accepted source units, followed only after materialization by an unassisted
  human baseline for those subjective tasks. The older 30/15 gate is retained
  with exact digests as non-authoritative historical evidence: it schedules
  no work and cannot authorize execution.
- `retrieval-queries.json` fixes 20 query intents and model-proposed expected
  anchors; exact restricted query text remains in ignored local content.
- `visual-retrieval-plan.v1.json` reuses those exact 20 query bytes against
  the 36 digest-frozen page images, with a complete source-anchor to
  page-image-anchor crosswalk. Completed text retrieval A and B remain
  immutable controls; direct visual challenger C is fixed to
  `Qwen/Qwen3-VL-Embedding-2B` revision
  `9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda` before download or output.
  Page images and query content remain local-only. The plan schedules no
  routine human work and establishes no transcription, quotation, semantic,
  winner, publication, or promotion result.
- `visual-retrieval-result.c-qwen3-vl-embedding-2b.v1.json` is the text-free
  return receipt for the owner-local audit-complete r9 execution. An
  independent Tree recorder rechecks frozen input, PNG, control, model,
  runtime, index, normalization, ranking, anchor, resource, and r8/r9 parity
  evidence from the private run while exposing no query/source strings, page
  images, vectors, or absolute owner paths. It records 20/20 stable rankings,
  200/200 resolved anchors, 19/19 evaluable model-proposed target presence,
  one source-return recovery, and four hard-negative outranks as advisory
  mechanics only. The preserved r8 result is classified
  `executed-audit-incomplete` because its 20/20 identical rankings and scores
  lack the persisted 76-row normalization audit. Two declared conditions open
  only the unscheduled criteria-review set `003`, `009`, `010`, `011`, and
  `020`; human judgments, human debt, winner, adoption, promotion, and
  publication remain zero or blocked, and no review interface is created.
- `graph-claims.jsonl` fixes 13 unreviewed claims across bibliographic,
  textual, provenance, and interpretive layers. Two alternative families are
  deliberately unresolved.
- `graph-queries.json` fixes 10 graph questions and the exact claim-set digest
  before any JSON, Neo4j, or RDF projection is run. `graph-provenance.jsonl`
  records that freeze separately from source segmentation.

## Truth boundary

The selection was visually inspected by a model. It therefore carries
`model_source_visible`, never `human_source_visible`. A source anchor can be
mechanically well formed while the chosen region, sentence boundary, or
transcription is still wrong.

`gold-status.json` is the frozen v1 state: 15 candidates and zero accepted
gold units. `gold-assurance.v2.json` is an additive, digest-bound assurance
overlay; it does not rewrite that history or promote any unit. Its current
human-work schedule preserves one Russian calibration observation, leaves
fourteen packet units unscheduled, reports zero human debt, and keeps the five
German units `language_competence_blocked`.

When a real Russian review trigger exists, the human evaluates page identity,
legibility, visible structure, reading order, fidelity, completeness, and
errors against the visible source. Full diplomatic transcription is a small,
rare independent-reference task, not the default interaction. A prepared
packet row, an unfinished count, or a UI regression is not a review trigger.

When an anchoring-independent reference is needed, a unit needs:

1. a model draft or independent human transcription kept as its actual maker;
2. a first human comparison against the source at full resolution;
3. a separate pass-1-hidden source-visible punctuation, lineation, and
   boundary recheck;
4. explicit uncertainty for unreadable glyphs;
5. a review receipt and error-ledger entries before `human_double_checked`.

The same human may perform the recheck only in a later session after the
declared local 24-hour floor. That evidence measures intra-annotator stability
and may support calibration metrics with disclosure. It never proves
inter-reviewer agreement or independent multi-human gold. The 24-hour value is
an operational separation rule, not a universal scientific threshold.

No validator may promote these states. Human-only translation must be
performed by a real human without AI and before the recognized translation is
revealed; generated text cannot stand in for that lane.

Accepted German is only the next gate. The nine pre-draft stages must then be
frozen independently in real-human-only, AI-only, and AI-alternative packets.
Machine-produced findings cannot seed the human-only packet, and a human edit
cannot be hidden inside a model packet. Only after each lane's own pre-draft
packet freezes may its translation draft begin; the AI+human draft may begin
only from already frozen independent human and AI drafts.

The OCR projection independently begins with five candidates per source and
zero materialized human-gold transcriptions. Its formal CER, WER, human
correction time, and quality winner remain blocked. Exploratory engine output
may be frozen and inspected, but the embedded ABBYY text and automatic EPUB
OCR remain sealed until all independent variant outputs and digests are
frozen; neither may seed a draft or gold transcription.

The graph claim set follows the same law. Mechanical traceability can show
that a projected edge returns to a claim, maker, event, evidence, and explicit
review state. It cannot turn `unreviewed` into a true bibliographic,
translation, sign, or semantic relation.

The cross-work transfer plan is stricter still. Variant C is not available
until reviewed kernel content actually exists. General contracts, model
proposals, blank worksheets, and a recognized translation cannot substitute
for human-accepted sign and translation packets. The three currently frozen
other-work title pages support structure and retrieval scouting, not semantic
evaluation. A separate deterministic preparation route now freezes twenty
private, content-bearing whole-page candidates from three other Nietzsche
works before any variant output: ten digest-random and ten mechanically hard.
Their text remains in ignored `local-content/transfer-targets/v1/`; tracked
records contain only anchors, digests, byte counts, mechanical metrics, and
review state. These candidates are explicitly ineligible, create no target
gold or routine human debt, and cannot become A/B/C inputs without the
triggered source-visible gate.

The first materialized translation-source packet was subsequently inspected
against all 30 proposed pages by `model:codex`. That advisory inspection
rejected the v1 selector method: 19 proposals selected headings, two selected
page-start sentence tails, five carried visible OCR contamination, and one
boundary remained unresolved without preceding-page context. Seven candidates
were structurally plausible only with limits; none gained human source
acceptance. The frozen plan is retained unchanged as negative evidence. The
tracked report and hash-bound receipt live in
`ToS/research-packets/foundation-laboratory-2026-07/`.

The successor v2 plan was then frozen and materialized as a private blind
source-review packet. It contains 30 review units, 89 unique scan-page renders,
two local workbooks, and a 30-row blank JSONL. Independent checks found exact
closure over the frozen page set, no emitted v1 candidate among all 30 prior
strings, no emitted Antonovsky identifier, and no human attestation,
transcription, or source decision. The packet manifest SHA-256 is
`8db6f88131355e858b861696899ae5edc37b238ec31493d898c559153010850c`.
This prepares the authority surface but does not create gold. The current
solo reviewer may judge only page identity, legibility, visible structure, and
reading order on German pages; German textual fidelity, orthography, grammar,
semantics, translation, and source acceptance remain blocked. The separately
reviewed assisted protocol now exists as a prepared-not-run route, but it does
not erase those blocked claims or accept source text by itself. Full
materialization evidence is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/TRANSLATION_SOURCE_REVIEW_V2.md`.

The separate 15-page OCR/structure human-gold interface has also been
materialized. It closes five frozen candidates per source over 41 unique
previous/current/next-page renders at 300 DPI; all 15 current-page hashes and
dimensions match the exact frozen OCR contestant inputs. Two deterministic
blind workbooks and a 15-row blank JSONL contain no OCR output, model answer,
embedded/reference OCR, recognized translation, human identity,
transcription, or acceptance. The packet manifest SHA-256 is
`a40f3d244fce19782c0c983634e37f252c36fdd6973def4ac01a3a633820c23d`.
Its legacy fail-closed exact-gold gate remains at pass 1: 0, pass 2: 0,
accepted: 0. Separately, the active solo+AI schedule now preserves
`tos-sample-antonovsky-p011` as one un-attested calibration observation, fixes
the unchanged autosave digest, marks the remaining fourteen units
`unscheduled`, and reports `human_debt_units: 0`. It creates no gold or source
acceptance. Paths, artifact hashes, independent checks, and the triggered
human-work law are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/HUMAN_GOLD_REVIEW_PACKET.md`.

## Local content

Restricted transcriptions, crops, prompts, drafts, and comparator text belong
under `local-content/`, which is ignored by Git except for its route card.
Tracked files carry metadata, anchors, digests, method, rights posture, and
review status only.

## Cross-repository execution

The frozen suite and run machinery are owned by
`abyss-stack:mechanics/inference-pilots/parts/tos-foundation-lab/`. ToS owns
source identity, sample identity, source anchors, rights, and review truth.
`abyss-stack` may read these tracked packets and local payloads through an
explicit run card; it must not write results into ToS canon or relabel review.
