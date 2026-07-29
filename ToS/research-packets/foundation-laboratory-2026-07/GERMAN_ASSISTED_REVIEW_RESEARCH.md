# German Assisted Source Review for Solo+AI

Status: focused method research and protocol decision; no German source
acceptance, transcription, translation, or human-language competence claim

Research snapshot: 2026-07-29

## Question

What useful source review can one Russian-speaking human perform with AI and
scholarly tools when that human cannot independently judge German orthography,
grammar, or meaning?

The answer is not “none”, but it is also not “German verified by a human”.
ToS needs separate evidence types:

- visual witness evidence from the page;
- published critical-edition evidence;
- machine triangulation;
- Russian-language evaluation within the operator's competence;
- German-language philological acceptance, which remains unavailable without
  declared competence.

## 1. Standards and official sources

### OCR-D Ground Truth Guidelines

OCR-D separates text ground truth from structure ground truth and requires the
chosen transcription depth to be explicit. Its current guidance recommends
Level 2, preserves the historical language level, minimizes typographic
interpretation, retains printing errors, and treats the named level as a
representation rule rather than a quality seal.

This permits a non-German-speaking reviewer to witness page identity, region
identity, reading order, visible glyph correspondence, and unresolved glyphs.
It does not turn that reviewer into authority for German spelling, grammar, or
meaning.

Sources:

- [OCR-D Ground Truth Guidelines](https://ocr-d.de/en/gt-guidelines/trans/)
- [OCR-D ground-truth levels](https://ocr-d.de/en/gt-guidelines/trans/trLevels.html)
- [OCR-D transcription law](https://ocr-d.de/en/gt-guidelines/trans/transkription.html)
- [OCR-D transcription fundamentals](https://ocr-d.de/en/gt-guidelines/trans/trGrundsaetze.html)

### TEI P5

TEI P5 4.11.0, updated 2026-02-18, keeps source, responsibility, certainty,
correction, conjecture, unclear text, and alternative readings separately
representable. ToS should therefore store the page-visible observation,
critical-edition reading, OCR/LLM proposals, and any later editorial choice as
distinct assertions with distinct makers.

Sources:

- [TEI P5 Guidelines](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/index.html)
- [TEI sources, certainty, and responsibility](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/ST.html)

### Nietzsche Source eKGWB

Nietzsche Source describes eKGWB as the digital version of the Colli/Montinari
German reference edition, proofread word by word against the print edition,
with about 6,600 philological corrections integrated and exposed through
stable work/section addresses. It is TEI-encoded and citable.

That makes eKGWB a strong published textual witness. It is not the same item
as the 1893 Naumann scan and must not silently replace edition-specific
variants. Its rights page currently states a BY-NC-ND 4.0 posture and requires
source citation; admission of text or derivatives still needs a scoped ToS
rights review.

Sources:

- [eKGWB description](https://doc.nietzschesource.org/en/ekgwb)
- [Nietzsche Source rights](https://doc.nietzschesource.org/en/rights)

### BBAW CLARIN / DWDS and historical corpora

The BBAW CLARIN center describes DWDS lexical resources and historical German
corpora, including Deutsches Textarchiv materials, as curated research
resources available through standardized corpus services. These resources can
support attested historical forms and usage. Dictionary or corpus presence is
evidence about language use; it does not prove which glyph appears on a
particular scanned page.

Sources:

- [BBAW CLARIN service center](https://fedora.dwds.de/de/)
- [BBAW CLARIN corpora](https://fedora.dwds.de/de/corpus/)

## 2. Established methodological work

Artstein and Poesio's agreement survey requires the identity and role of
coders to remain explicit. Agreement between OCR systems, an LLM, a critical
edition, and a human visual witness is not inter-coder agreement among
German-competent humans.

Schroeder, Roy, and Kabbara's preregistered 2025 study found that visible LLM
suggestions changed human label distributions and increased confidence
without making annotators faster. A user approving an AI explanation must
therefore be recorded as AI-assisted evidence, not independent human German
judgment.

Sources:

- [Inter-Coder Agreement for Computational Linguistics](https://aclanthology.org/J08-4004/)
- [Just Put a Human in the Loop?](https://aclanthology.org/2025.findings-acl.1323/)

## 3. Fresh evidence checked on 2026-07-28

Gu et al. (ACL 2026) found useful time savings in one expert event-annotation
workflow while still concluding that the LLM was not a good independent
annotator. This supports an assistant lane with typed adoption and rejection,
not authority transfer.

Kunilovskaya et al. (preprint submitted 2026-06-01) audited 2,667 annotation
tasks and found language proficiency among the details frequently omitted
despite their importance to validity. ToS must keep proficiency and allowed
claims first-class.

DocOCR-Eval (work-in-progress preprint submitted 2026-05-05) proposes
correction-based OCR ranking without ground truth and reports improved
alignment with annotation-based rankings when multiple models are aggregated.
It is useful for selecting which disagreements deserve attention. It does not
produce human ground truth or German competence.

Sources:

- [Large Language Models Are Effective Human Annotation Assistants, But Not Good Independent Annotators](https://aclanthology.org/2026.findings-acl.4/)
- [Who Annotates in NLP?](https://arxiv.org/abs/2606.02255)
- [DocOCR-Eval](https://arxiv.org/abs/2607.16203)

## 4. Focused official-route refresh on 2026-07-29

The first source-ordered discovery run had correctly preserved a negative
result: the documented HTTPS routes did not return the passage through the
available network path. A later inspection of the public Nietzsche Source SPA
exposed its own static HTML include route for `#eKGWB/Za-I`. The exact HTTPS
form again failed to connect twice. The exact public HTTP form returned status
200 twice in 0.979062 and 0.964702 seconds; both responses were 180,138 bytes
and had SHA-256
`3ad9b3f601eccf45446c71f2758d1aa5fd70fb01cc67c81453526f1b7eeb7061`.

This is a useful originating-project route, but not an authenticated transport:
HTTP is unencrypted. The response therefore remains a local-only
critical-edition candidate. Its discovery, acquisition, hash, endpoint,
declared BY-NC-ND posture, and transport warning are preserved; no response
text is tracked or authorized for publication.

A source-aware machine comparison then isolated `Za-I-Vorrede-1` without
copying its text into Git:

- the eKGWB section contains 12 content paragraphs and 261 normalized
  alphabetic tokens;
- DTA part I, selected at
  `TEI/text[1]/body[1]/div[1]/div[1]`, has the same 12 paragraph token
  sequences and all 261 tokens after the DTA printed-hyphen marker `¬` is
  removed with its following whitespace;
- generic whitespace normalization initially manufactured three false token
  splits, which are retained as a normalization failure control;
- the two bounded Naumann automatic-EPUB members contain 392 tokens: 260 of
  the 261 reference tokens occur in three equal runs, with one one-token OCR
  replacement, 15 page-furniture tokens, and 116 trailing tokens belonging to
  the next section.

This is `machine_triangulated_candidate`, not accepted German. It establishes
neither HTTPS authenticity, critical-edition admission, orthographic or
grammatical correctness, semantics, translation correspondence, rights
clearance, nor canon.

## Protocol decision

### Evidence lanes

1. **Page-visible human lane**

   The operator may decide page/region identity, legibility, layout, reading
   order, visible glyph correspondence, and whether an unresolved glyph needs
   escalation. The operator does not decide German correctness or meaning.

2. **Critical-edition witness lane**

   A stable eKGWB or other reviewed critical-edition anchor may supply a
   published German reading with edition, rights, citation, and provenance.
   It remains a separate witness from the Naumann item; discrepancies become
   apparatus candidates rather than silent corrections.

3. **Independent machine lanes**

   OCR/LLM candidates are generated independently and compared at character,
   token, and region level. Lexical/corpus checks may flag unattested or
   historically unusual forms. Agreement narrows attention but never becomes
   human evidence.

4. **AI explanation lane**

   AI may explain each disagreement in Russian, cite the exact German forms
   and external sources, expose alternatives, and abstain. Its maker, prompt,
   model, source access, and prior candidate exposure remain recorded.

5. **Human decision within competence**

   The operator may accept a visual correspondence, accept or reject a
   Russian explanation as useful, or defer. The operator cannot attest German
   orthography, grammar, semantics, etymology, or translation fidelity.

### Source postures

- `visual_identity_confirmed` — page/region/visible structure only;
- `machine_triangulated_candidate` — independent systems agree or a bounded
  disagreement packet exists;
- `critical_edition_witness_admitted` — a published reading is anchored,
  rights-reviewed, and edition-typed;
- `language_competence_blocked` — German linguistic acceptance is still open;
- `german_competent_human_accepted` — available only after real declared
  competence and source-visible review.

### Translation consequence

A reviewed critical-edition witness can open bounded AI-only and AI+human
experimental translation lanes without pretending that OCR became accepted
German ground truth. It cannot create a genuine human-only German translation
lane for the current operator.

Human-only translation, German philological acceptance, accepted etymology,
and canonical translation remain blocked until their actual authority is
supplied. AI-only and AI+human output remain proposals even when the operator
judges their Russian form.

### Scheduling law

The thirty prepared German units are not a human backlog. Assisted review
opens only for:

- a specific source discrepancy;
- a source anchor needed by the next bounded translation experiment;
- a new source or method calibration;
- an unresolved glyph or semantic-risk case;
- periodic drift control.

One to three units are selected per distinct question. Interface regression
uses synthetic German-like fixtures and does not consume the real packet.

## Current decision

Adopt the protocol and record one bounded machine-only run for
`tos-translation-source-review-v2-001`. The assisted plan is now `in_progress`
with one selected unit, one machine-triangulated candidate, zero human debt,
zero accepted German units, zero admitted critical-edition units, and no
translation lane. This is useful source calibration without lying about
competence.

The first metadata-only critical-edition route is now frozen in
`critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json`. It binds
`tos-translation-source-review-v2-001` to the eKGWB section locator
`Za-I-Vorrede-1`, records the edition identity, stable-URL evidence, declared
BY-NC-ND rights posture, unsuccessful direct browser and shell fetches, and
exact provenance events. It remains an honest historical metadata snapshot of
that event; the later HTTP fallback and comparison do not rewrite its
observations.

The same refresh performed a separate local structural comparison. The
fixity-verified Naumann automatic-EPUB member `EPUB/page_55.html` and scan PDF
page 56 begin printed section 1; `EPUB/page_56.html` and scan PDF page 57 carry
the transition to printed section 2. This makes the first prepared unit
structure-compatible with the scope named by `Za-I-Vorrede-1`. It does not
compare the eKGWB reading, establish a sentence-level alignment, judge German
correctness, or turn automatic OCR into source truth. No eKGWB or local source
text was copied into the tracked packet.

The first protocol-native material-discovery run remains preserved in
`source-witnesses/discovery/runs/ekgwb-za-i-vorrede-1.2026-07-28.v1.json`.
It reached the GND work authority through its machine interface, but repeated
direct requests to Nietzsche Source timed out. The bounded exact-siglum
general-web fallback returned secondary or interpretive noise rather than the
originating critical-edition passage. The run therefore selected no result,
downloaded no content, and remains `incomplete`. This is negative discovery
evidence, not evidence that the critical passage is absent.

The reconciled successor run is
`source-witnesses/discovery/runs/ekgwb-za-i-vorrede-1-http-fallback.2026-07-29.v2.json`.
It records the two failed HTTPS requests, the two byte-identical official HTTP
responses, the local-only acquisition, and the reason HTTP transport cannot
close authenticity or admission. The separate
`german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.v1.json`
records only selectors, fixity, counts, one-way fingerprints, method, and gate
effects. No German passage text is tracked.

The access route remains useful after reconciliation and now points to
`access-requests/public-ledger/nietzsche-source-ekgwb.access-request.json`.
The public-safe card now asks for an authenticated or institutionally supplied
copy and clarification of exact local processing scopes rather than pretending
the HTTP candidate is inaccessible. It freezes the exact passage, research
purpose, local storage posture, permissions needing clarification, and
non-redistribution boundary. A 2026-07-28 refresh found the official ITEM
“Nietzsche et son temps” page: the team states that it publishes the improved
digital critical edition through Nietzsche Source and exposes a current public
institutional email route for its lead. The tracked card therefore records the
institutional page and channel class, not a personal recipient or private
correspondence. Its identity posture remains
`probable-needs-human-review`, because a suitable research contact is not by
itself proof that the same institution can grant every requested processing
right. The card is still only `draft-not-sent`: no message body or private
recipient identity was committed, no human send approval exists, and no
communication occurred.

This remains a candidate route, not an admitted witness. Human bibliographic
and rights review remain false. Exact normalized machine agreement between
the local eKGWB response and the DTA section is now recorded, but authenticated
transport, philological acceptance, and human passage alignment remain
unverified. No German unit was accepted and no human task or translation lane
was opened. A later specific source or translation question may use the
candidate as its rights-gated starting point; packet existence alone is not a
review trigger.

A second, deliberately different route now exists in the source owner:
Deutsches Textarchiv's four DTABf/TEI P5 transcriptions for the public
part-by-part sequence from 1883, 1884, and 1891. Their exact bytes, distinct
edition and holding-copy identities, page boundaries, and external
native-speaker-checking statements are checksum-bound locally. These are
source-structured comparison witnesses, not a critical-edition packet, one
silent whole-work text, or ToS-accepted German. They can later help isolate
OCR disagreement and return claims to printed pages, but they do not remove
the language-competence block or open a translation lane. The current CC
BY-SA evidence, stale conflicting Dublin Core fields, and separate facsimile
rights remain explicit per item.
