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
  explanations, and later competent-human acceptance. It selects one unit for
  one machine-only triangulation run, creates zero human debt, accepts zero
  German units, and now opens only the private `ai_only` and `ai_human`
  experimental lanes through the later bounded citation-witness admission.
  The `human_only` lane remains forbidden for the visual-only solo operator.
- `critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json` freezes one exact
  eKGWB section locator for the first prepared German unit. It stores no
  eKGWB text, records the failed direct-fetch paths, and binds the locator to
  the fixity-verified local EPUB/PDF section boundary. That evidence proves
  only structural compatibility: the critical passage itself was not fetched
  or compared. Bibliographic and rights review remain pending, no human task
  is created, and no German unit or translation lane is admitted.
- `rights.ekgwb.za-i-vorrede-1.v1.json` is the later model-authored rights and
  transport assessment for that exact route. It records that CC BY-NC-ND 4.0
  permits non-commercial private local adaptation while prohibiting its
  sharing, and keeps the HTTP payload local-only, unauthenticated, unadmitted,
  and publication-blocked. It does not rewrite the digest-bound historical
  critical-witness packet.
- `german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.v1.json`
  records the first bounded machine-only comparison. The local eKGWB response
  and DTA part-I section agree on all 12 paragraph token sequences and 261
  normalized tokens after source-aware DTA dehyphenation; the Naumann
  automatic EPUB agrees on 260 reference tokens and retains one OCR
  replacement plus page furniture. The tracked packet contains no source text.
  Unencrypted HTTP, rights, bibliography, language competence, and all
  translation/semantic gates remain explicitly open.
- `critical-edition-citation-witness-decision.ekgwb.za-i-vorrede-1.v1.json`
  is the additive 2026-08-08 admission overlay over that frozen history. It
  binds the metadata packet, exact triangulation, institutional WARC
  corroboration, rights record, and ordered current research by SHA-256. It
  records the operator's `human_admitted_with_limits` decision for the locator
  and local-only observed reading as an edition-typed citation witness. The
  admission opens only private non-commercial `ai_only` and `ai_human`
  experiments. Accepted German, accepted translation, publisher
  authentication, German linguistic correctness, publication, semantics,
  graph truth, canon effects, and the `human_only` lane remain false.
- `edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json` makes the next
  source boundary exact. It admits only that the fixity-bound DTA Part I TEI,
  at its declared section selector and under its provider-documented
  OCR-plus-native-speaker-recheck method, supplies one edition-local reading;
  the distinct eKGWB critical witness independently corroborates the same 12
  paragraphs and 261 normalized tokens. The text-free packet creates one
  `edition_reading_attested` unit and permits a later v4 semantic contract to
  materialize source-observational work. Accepted German, linguistic
  correctness, Edition identity, author-finality, accepted translation,
  signs, semantics, graph truth, human tasks, transfer, and publication all
  remain zero or false. The companion
  `provenance.edition-reading-admission.jsonl` records that the local TEI
  header, not its source body, was inspected during admission.
- `experimental-translation-candidate.admitted-ekgwb.za-i-vorrede-1-opening.variant-a.v1.json`
  binds the first admitted experimental `ai_only` candidate to that decision,
  the current source-return overlay, and the exact private frozen A run without
  tracking source or candidate text. Direct Russian-surface inspection found
  obvious defects, so the candidate is retained as negative evidence and
  rejected before human review. It creates no human task, accepted German,
  accepted translation, semantic object, graph assertion, or canon effect.
- `experimental-translation-episode.e4b-direct.infrastructure-failure.v1.json`
  and `experimental-translation-episode.e4b-direct.russian-surface-rejection.v1.json`
  extend that method with one separately frozen resident E4B capacity probe,
  not a fourth historical A/B/C lane. The first episode retains a
  runner-contract failure before any candidate existed; the corrected episode
  returned one schema-valid private candidate that AI Russian-surface triage
  rejected before human review. The paired
  `provenance.experimental-translation-episodes.jsonl` binds run, candidate,
  triage, method, resource, and tracked receipt fixity without copying source
  text, candidate text, recognized translations, or private paths. It creates
  no human task, accepted German, accepted translation, semantic object, graph
  assertion, publication route, or canon effect.
- `experimental-translation-episode.qwen3-8b-ovms-cpu.russian-surface-rejection.v1.json`
  records the separately admitted Qwen3-8B/OpenVINO CPU successor. Its private
  lineage retains three source-free topology failures, one pre-worker resource
  block, one request timeout, and one incomplete bounded JSON result before a
  1536-token run returned a schema-valid candidate. AI inspection was limited
  to the Russian surface and rejected the candidate before human review. The
  companion `provenance.experimental-translation-episodes.qwen3-8b-ovms-cpu.jsonl`
  binds those private receipts only by digest. No source or candidate text,
  private path, recognized translation, accepted German, accepted translation,
  semantic object, graph assertion, publication route, or canon effect enters
  the tracked tree. The owner-wrapper memory number explicitly excludes the
  rootless OVMS model process and is therefore not reported as model peak.
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
- `INITIAL_SIGN_PACKET_ROUTE.md`, the frozen
  `initial-semantic-source-observation-plan.v1.json`, and
  `initial-sign-packet.v5.json` preserve the fifteen-stage form-to-graph route.
  A deterministic source-observational rule selected one exact form inside the
  admitted DTA section without using stopwords, lemmas, semantics, or
  philosophical importance. All four section occurrences, two pages, exact TEI
  character-data offsets, and twelve-token page-and-section-bounded contexts
  are now materialized. Source values remain only in ignored mode-`0600`
  files; tracked evidence is hash/count/selector-only. The packet is
  `observational-analysis`, not a sign proposal: morphology, lemma,
  translation, sign candidate, human task, relation, concept, claim, graph,
  canon, transfer, and publication remain absent or blocked. German competence
  remains independently blocked and the historical 0-of-30 packet does not
  schedule this work.
- `semantic-source-recurrence-plan.v1.json` and its tracked receipt bind that
  already selected exact-form hash to exactly one existing four-part
  recurrence row without reopening selection. A deterministic private bundle
  returns all 145 occurrences to four fixity-bound raw TEI payloads: 17, 32,
  45, and 51 occurrences across the four parts, 96 pages, and 59 sections.
  All 145 Unicode-offset slices match the selected hash under
  `xmllint --nonet`; exact
  values and occurrence positions remain ignored and mode `0600`. The v5
  packet is unchanged, no ladder stage advances, and recurrence creates no
  German, morphology, lemma, sense, motif, philosophical-importance, sign,
  human, semantic, graph, canon, transfer, or publication authority.
- the separate DTA parts 1–4 lexical observation route materializes 86,287
  exact token occurrences across 506 body pages and 213 TEI divisions in one
  deterministic ignored SQLite/FTS5 database. Exact, normalized, prefix,
  phrase, section, page, language, and edition probes pass locally. Its
  tracked 11,352-row companion carries only form hashes, aggregate counts, and
  resource refs; it carries no source strings, sequence, context, or
  occurrence positions. Lemma, translation, sign candidate, sign, source
  acceptance, rights clearance, and public routing remain zero or blocked.
  The later v5 packet consumes one exact section-bounded observation without
  changing those authority limits.
- the tracked recurrence layer now derives one deterministic observation tuple
  for each of those 11,352 exact-form hashes without reading local payloads or
  the private SQLite database. It keeps A frequency, B part/division/page
  range, and C frequency+range+part-size-aware `DP`+maximum concentration as
  separate dimensions. It creates no leaderboard or importance score, exposes
  no source string, and has zero morphology, lemma, lexeme, sign-candidate,
  semantic, publication, or human-backlog effect. This remains queryable soil
  below the semantic ladder; the later v5 packet consumes only one
  independently selected, section-bounded observation from it.
- the question-scoped usage-context control materializes all 527 occurrences
  of one preselected Work-identity exact-form hash as a deterministic
  24-token-per-side, same-page KWIC bundle. The 875,930 source-bearing bytes
  remain ignored and mode `0600`; the tracked receipt exposes only fixity,
  counts, source-state and selector closure. All 527 occurrence, target-hash,
  page, and source-file selectors close; 524 section selectors close and the
  remaining three are explicitly unsectioned. This is a transparent context
  baseline, not a sentence, sense, lexeme, sign proposal, recurrence rank,
  public object, challenger schedule, or human-review trigger. It changes no
  accepted source, morphology, translation, semantic, graph, or canon state.
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
  been reviewed, and German-competent gold remains zero.
- `morphology-contextual-episode.selected-form-b.v1.json` freezes the first
  concrete context-specific challenger question before any B output is seen.
  One exact-form A row returns competing adverb and separable-verb-component
  analyses. Output-blind recurrence ranks 1, 73, and 145 select the first,
  inclusive-median, and last occurrences across parts 1, 3, and 4. The private
  mode-`0600` packet returns each exact target slice inside two source
  paragraphs and one verse group; the tracked receipt and provenance expose
  only fixity, structure, counts, selection closure, and execution posture.
  The later artifact-admission overlay records what happened without rewriting
  that pre-output freeze: the exact B wheel and its 41-wheel CPython 3.12
  closure were acquired into private owner cache, but required Cosign evidence,
  release lifecycle, non-local trust root, verification, and subject-store
  closure were absent. The exact runtime trust gate therefore denied
  admission. No runtime was built, no context packet was consumed, and no B
  output exists. Variant C remains blocked as question-inapplicable because
  exact and normalized form identity do not differ in this episode. This
  creates no German-competent gold, morphology acceptance, lemma, lexeme,
  semantic effect, or human backlog.
- `transfer-samples.json` closes the stack suite's cross-work sample reference
  with a source-gated `blocked-not-run` plan. Its three Mysl pages are
  title-page scouting boundaries only; all are ineligible for semantic
  transfer. A real run requires 30 accepted German units, 15 double-checked
  kernel gold units, accepted sign and translation packets, and 20
  double-checked content-bearing target passages. `transfer-provenance.jsonl`
  binds that non-run decision to the current source and gold gates.
- `transfer-target-passage-candidates.v1.json` now returns all thirty-five
  conservative routes to the exact ignored Mysl PDF. It records thirty-two
  page-intersecting and three rejected nonintersecting numbered-unit slices;
  private strings are mode 0600 under ignored
  `local-content/transfer-target-passages/v1/`, while tracked candidates and
  anchors contain only selectors, geometry, counts, digests, provenance, and
  zero-effect gates. Layer-exact automatic boundaries are not diplomatic or
  accepted Russian, German source passages, alignments, eligible units, gold,
  semantics, canon, publication authority, or human work.
- `transfer-source-passage-candidates.v1.json` returns those same thirty-five
  routes to the exact German source layers. All thirty-five routes now receive
  private mode-0600 candidates under ignored
  `local-content/transfer-source-passages/v1/`. Two missing PDF labels are
  closed only by exact embedded image-mask marker returns to the first
  following Poppler bbox record; four *Antichrist* routes retain exact JP2
  marker support. Tracked data contains only geometry, counts, digests,
  witness relations and thirty-five proposed anchors. The *Antichrist* Commons
  address Item and Internet Archive DjVuXML navigation Item retain a bounded
  two-page relation with no textual-identity assertion. This creates no
  diplomatic or accepted German, source-target alignment, eligibility, gold,
  human work, semantics, publication authority, or canon.
- `transfer-route-readiness.v1.json` joins those source and target candidate
  identities without reading either private text layer. All thirty-five routes
  have both mechanical candidates; thirty-two target slices intersect their
  frozen pages and three remain explicit target nonintersections. Every one of
  the twenty frozen pages still has at least one dual intersecting route. This
  is a text-free navigation/readiness projection, not a bilingual passage pair,
  alignment, accepted text, eligibility, gold, human task, semantic opening,
  publication grant, or canon effect.
- `transfer-source-visible-review.jenseits-187.v1.json` is the first additive
  source-visible overlay over one of those routes. Four 180-DPI page renders
  reproduce exactly. The private model transcript and the separately selected
  eKGWB critical section share all 156 alphabetic tokens, while the automatic
  German slice retains material OCR defects. On the Russian side, direct page
  inspection catches one complete line omitted at the page-307 boundary by the
  otherwise mechanically valid candidate. All source-bearing strings remain
  mode 0600 and ignored. This is a machine-triangulated candidate, not human
  evidence, historical-critical identity, accepted German/Russian, alignment,
  eligibility, gold, scheduled human work, semantics, publication, or canon.
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

`manual-error-ledger.jsonl` preserves its earlier empty-state record and then
adds the completed 2026-07-26 source-visible, method-blind, solo-human review
as one aggregate episode: 10 Russian pages, 30 candidate dispositions, the
observed outcome/error counts, and 8,334 seconds of confounded human time. The
episode returns only through the governed handoff and public-safe aggregate
derivative; its companion provenance binds all three digests. It embeds no
source text, unit judgment, reviewer identity, or private locator and accepts
no transcription, independent gold, method winner, content authority, or
routine human backlog.

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

The selected-form contextual morphology payload is stored at
`local-content/morphology/contextual-episodes/exact-form-0007489c-b-v1.jsonl`.
It is private, ignored, mode `0600`, and contains the three exact source
contexts needed for a B-only run. The repository tracks only its text-free
receipt, plan, and provenance; none authorizes source publication.

## Cross-repository execution

The frozen suite and run machinery are owned by
`abyss-stack:mechanics/inference-pilots/parts/tos-foundation-lab/`. ToS owns
source identity, sample identity, source anchors, rights, and review truth.
`abyss-stack` may read these tracked packets and local payloads through an
explicit run card; it must not write results into ToS canon or relabel review.
