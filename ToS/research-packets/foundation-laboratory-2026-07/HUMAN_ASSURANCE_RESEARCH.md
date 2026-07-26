# Human Review Assurance Research

Status: method research for the next manual-gold contract; not a human review
receipt and not source acceptance

Research snapshot: 2026-07-26

## Question

How should Tree of Sophia describe manual source review when the real operating
model is one human working with AI, exact transcription is expensive, German
language competence is not available to that human, and later A/B/C cost
comparisons must not be confounded by display position?

This packet follows the project research order:

1. current standards and official guidance;
2. established methodological work;
3. recent work checked at the snapshot date.

## 1. Standards and official guidance

### OCR-D Ground Truth Guidelines

The current OCR-D Ground Truth site says that ground truth exists both to train
OCR and to evaluate recognition results. Its transcription law preserves the
historical language level, minimizes unavoidable typographic interpretation,
does not silently correct printing errors, and permits explicitly declared
transcription depths. Its current pages identify the underlying OCR-D
specification as version 3.4.0, so this is authoritative guidance with an old
technical-version dependency rather than evidence that every component is
new.

The same guidance separates transcription from layout and reading order.
Ground truth therefore is not one undifferentiated text box: textual form,
page regions, and reading order need distinct evidence.

Sources:

- [OCR-D Ground Truth Guidelines](https://ocr-d.de/en/gt-guidelines/trans/)
- [OCR-D transcription guidance](https://ocr-d.de/en/gt-guidelines/trans/transkription.html)
- [OCR-D transcription fundamentals](https://ocr-d.de/en/gt-guidelines/trans/trGrundsaetze.html)
- [OCR-D layout and structure guidance](https://ocr-d.de/en/gt-guidelines/trans/layout.html)
- [OCR-D reading-order guidance](https://ocr-d.de/en/gt-guidelines/trans/lyLeserichtung.html)

### TEI P5 primary-source representation

TEI P5 4.11.0, updated 2026-02-18, treats transcription as an explicit scholarly
representation. It provides separate forms for source text, correction,
normalization, omission, uncertainty, responsibility, and evidence rather than
silently replacing one form with another. This supports keeping diplomatic
text, presentation typography, uncertainty, and editorial intervention as
separate fields.

Source:

- [TEI P5 4.11.0, Representation of Primary Sources](https://tei-c.org/release/doc/tei-p5-doc/en/html/PH.html)

### NIST experimental-design guidance

The NIST/SEMATECH e-Handbook describes Latin-square designs as a way to
separate a treatment effect from two blocking factors. For the OCR review
workbench, the hidden OCR method is the treatment; source page and display
position are blocking factors. A purely independent shuffle does not guarantee
that every method occupies every position equally often.

Source:

- [NIST Latin square and related designs](https://www.itl.nist.gov/div898/handbook/pri/section3/pri3321.htm)

## 2. Established methodological work

Artstein and Poesio's 2008 survey distinguishes agreement measures and their
assumptions for computational-linguistic annotation. It supports a strict
terminological boundary: agreement among different coders is not the same
measurement as one coder repeating a judgment.

Source:

- [Artstein and Poesio, Inter-Coder Agreement for Computational Linguistics](https://aclanthology.org/J08-4004/)

The durable conclusion for ToS is not that every source transcription requires
two people. It is that the number and identity of reviewers, their instructions,
their exposure, and the kind of agreement being claimed must be reported
instead of compressed into the word “gold”.

## 3. Recent and snapshot-current work

### Same-human repetition measures stability

Abercrombie et al. (2025) explicitly distinguish inter-annotator reliability
from intra-annotator temporal stability. Their experiments found inconsistent
repeat responses on more than 25 percent of items across four NLP tasks. They
also report that prior studies used intervals ranging from minutes to a month;
the literature does not establish one universal delay that turns a repeated
judgment into independence.

Source:

- [Consistency is Key: Disentangling Label Variation with Intra-Annotator Agreement](https://aclanthology.org/2025.nlperspectives-1.6/)

### AI assistance changes the evidence-generating process

Schroeder, Roy, and Kabbara (ACL 2025) ran a preregistered experiment with 350
annotators and 7,000 annotations. LLM suggestions did not make annotators
faster, increased confidence, and significantly shifted labels toward model
suggestions. A human-approved model suggestion is therefore not equivalent to
an anchoring-independent human reference.

Gu et al. (ACL 2026) found AI useful as an expert assistant in a particular
event-annotation workflow, including a measured time reduction, while still
finding it unsuitable as an independent expert replacement. These results are
task-specific, but together they support separate `human-only`, `AI-only`, and
`AI+human` lanes rather than pretending assistance has no effect.

Sources:

- [Just Put a Human in the Loop?](https://aclanthology.org/2025.findings-acl.1323/)
- [Large Language Models Are Effective Human Annotation Assistants, But Not Good Independent Annotators](https://aclanthology.org/2026.findings-acl.4/)

### Correction can help selection without becoming source truth

DocOCR-Eval (preprint dated 2026-05-05) uses multi-model correction discrepancy
to approximate OCR-tool ordering without human ground truth. Its reported
rankings do not always match annotation-based rankings. It is therefore a
useful current lead for cheap method selection, not a replacement for the
small independent reference needed for exact CER/WER or accepted source text.

Bubula et al. (2025) likewise study scalable OCR assessment by combining a
limited ground-truth sample with confidence signals over a much larger
historical corpus. This supports a small representative reference plus broader
diagnostics instead of full manual transcription of every page.

Sources:

- [DocOCR-Eval](https://arxiv.org/abs/2607.16203)
- [How Scalable is Quality Assessment of Text Recognition?](https://doi.org/10.63744/GR59c1iXu6Wj)

### Current annotation reporting still omits critical context

Kunilovskaya et al. (preprint dated 2026-06-01) audit 2,667 annotation tasks
reported in ACL-venue papers from 2018–2025. They identify language proficiency,
training, adjudication, and agreement as frequently missing information needed
to judge validity. This directly supports making language competence and
assurance posture required fields.

Source:

- [Who Annotates in NLP?](https://arxiv.org/abs/2606.02255)

## Contract consequences

1. **Criteria review is the default.** Reading the source, judging fidelity,
   completeness, structure, and errors, and optionally correcting a visible
   candidate is the ordinary human route.
2. **Independent transcription is small and rare.** It is used only where an
   anchoring-independent reference is needed for exact metrics or calibration.
3. **One human may recheck their own reference.** Pass 2 must be a separate,
   pass-1-hidden session. It measures intra-annotator stability and may support
   calibration metrics with disclosure; it never proves inter-reviewer
   agreement.
4. **The local delay floor is operational, not universal science.** The next
   ToS contract uses 24 hours to prevent a same-sitting recheck and records the
   actual interval. It does not claim that 24 hours makes the pass independent.
5. **AI exposure is typed.** AI may assist ordinary work, but its output stays
   hidden from an independent-reference pass. AI-assisted review remains a
   separate lane and carries anchoring risk.
6. **Language competence bounds every claim.** A visual-only German review may
   judge page identity, legibility, and visible structure. It cannot establish
   German orthography, grammar, semantics, or translation quality.
7. **“Gold” becomes an assurance ladder.** Single-human, solo-rechecked, and
   independent-multi-human evidence remain distinguishable and have different
   allowed uses.
8. **Cost-bearing A/B/C review is block-balanced.** The packet predeclares a
   Latin-square-like assignment over method and display position. When exact
   balance is impossible, the maximum position-count difference must be one
   and the remaining imbalance must be reported.

## Stop line

No source text, reviewer identity, language competence, or gold status changes
because this research packet exists. The findings become operative only
through reviewed ToS contracts and actual source-visible human receipts.
