# German Assisted Source Review for Solo+AI

Status: focused method research and protocol decision; no German source
acceptance, transcription, translation, or human-language competence claim

Research snapshot: 2026-07-28

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

Adopt the protocol as a prepared, not-run solo+AI route. It improves what can
be learned without lying about competence, but it does not by itself accept
one German unit or authorize one translation.

The first metadata-only critical-edition route is now frozen in
`critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json`. It binds
`tos-translation-source-review-v2-001` to the eKGWB section locator
`Za-I-Vorrede-1`, records the edition identity, stable-URL evidence, declared
BY-NC-ND rights posture, unsuccessful direct browser and shell fetches, and
exact provenance events. A 2026-07-28 refresh found the official eKGWB and
rights pages still represented in current search indexes and found independent
scholarly uses of the exact siglum, but neither current network path could
fetch the critical passage directly.

The same refresh performed a separate local structural comparison. The
fixity-verified Naumann automatic-EPUB member `EPUB/page_55.html` and scan PDF
page 56 begin printed section 1; `EPUB/page_56.html` and scan PDF page 57 carry
the transition to printed section 2. This makes the first prepared unit
structure-compatible with the scope named by `Za-I-Vorrede-1`. It does not
compare the eKGWB reading, establish a sentence-level alignment, judge German
correctness, or turn automatic OCR into source truth. No eKGWB or local source
text was copied into the tracked packet.

This is a prepared admission packet, not an admitted witness. Human
bibliographic and rights review remain false, exact critical-text comparison
and passage alignment remain unverified, no German unit was accepted, and no
human task or translation lane was opened. A later specific source or
translation question may use this packet as its rights-gated starting point;
packet existence alone is not a review trigger.
