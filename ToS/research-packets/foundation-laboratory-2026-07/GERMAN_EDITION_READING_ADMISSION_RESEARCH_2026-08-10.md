# German Edition-Reading Admission for Solo+AI

Status: ordered method research and contract recommendation; no source-reading
admission, German-language competence, translation acceptance, sign promotion,
semantic truth, graph truth, canon effect, or publication authority

Research snapshot: 2026-08-10

## Question

Can Tree of Sophia admit what one exact scholarly edition or witness
transcription reads when its solo operator cannot independently judge German
orthography, grammar, semantics, etymology, or translation fidelity?

Yes, but only if ToS stops compressing two different claims into “accepted
German”:

1. **edition-reading evidence** — this identified edition or transcription
   contains this source-returnable reading under its declared editorial method;
2. **German-language judgment** — the reading is linguistically correct, its
   morphology or meaning is established, or a translation is faithful.

The first can be admitted from edition-typed scholarly evidence without
inventing competence in the operator. The second still requires evidence fit
for the exact linguistic or interpretive claim. Neither claim establishes one
universal, author-final, or canonically preferred text.

## 1. Current standards and official source documentation

### TEI P5

TEI keeps the electronic resource, its edition, the source from which it was
derived, editorial responsibility, corrections, normalization, certainty, and
revision history separately representable. In particular, `sourceDesc`
identifies the source of a digital text, `editionStmt` identifies a fixed state
of an electronic edition, and responsibility statements identify editors or
institutions. This supports an edition-local source assertion without silently
promoting it to a universal language or Work-level truth.

Sources:

- [TEI P5, The TEI Header](https://tei-c.org/release/doc/tei-p5-doc/en/html/HD.html)
- [TEI P5, Certainty, Precision, and Responsibility](https://tei-c.org/release/doc/tei-p5-doc/en/html/CE.html)
- [TEI P5, Representation of Primary Sources](https://tei-c.org/release/doc/tei-p5-doc/en/html/PH.html)

### Deutsches Textarchiv

The current DTABf documentation describes a constrained TEI P5 profile for
coherent historical full-text preparation. The DTA transcription rules preserve
the historical language state, minimize silent modernization, and distinguish
source-oriented XML from a reader-facing HTML presentation.

The DTA's owner guidelines make the exact epistemic scope unusually clear:

- the usual acquisition routes are double keying with comparison, or OCR with
  manual correction;
- either route is followed by manual rechecking;
- the acquisition method is recorded per Item;
- the full text aims to be a reliable transcription of the selected physical
  source;
- it is not offered as a comprehensive critical edition or commentary.

That is sufficient evidence for “the DTA transcription of this exact source
reads X” when the exact Item, header, selector, bytes, and editorial policy are
bound. It is not evidence for “X is the only correct German text”, “X is
author-final”, or “a translation of X is faithful”.

Sources:

- [DTA owner guidelines](https://www.deutschestextarchiv.de/doku/leitlinien)
- [DTABf transcription rules](https://deutschestextarchiv.github.io/dtabf/transkription.html)
- [DTABf introduction](https://deutschestextarchiv.github.io/dtabf/einfuehrung.html)
- [DTABf development guidelines](https://deutschestextarchiv.github.io/dtabf/leitlinien.html)

### Nietzsche Source eKGWB

Nietzsche Source describes eKGWB as the digital Colli/Montinari German
reference edition, proofread against the print edition, enriched with about
6,600 separately visible philological corrections, TEI-encoded, and addressed
by stable work/section locators. This makes it a strong edition-typed critical
witness. It does not collapse into the 1883 DTA Item or prove that the two
editions are universally identical.

Source:

- [eKGWB official description](https://doc.nietzschesource.org/en/ekgwb)

## 2. Established top methodological work

The IDE/RIDE criteria define a scholarly digital edition as a critical
representation governed by an explicit editorial method and scholarly quality
requirements. They require bibliographic identity, editor and institution,
source selection, transcription perspective, responsibility, persistent
identification, access to basic data, rights, and documentation to be judged
separately. They also say that a review may legitimately cover only the
methodological or only the content side if that limit is declared.

This supports scope-bounded admission: ToS may rely on a documented edition for
an edition-local reading while explicitly withholding linguistic, critical,
and interpretive judgments it is not qualified to make.

Sahle's established account likewise treats a scholarly digital edition as an
edition with an explicit scholarly method rather than as an untyped digital
text. The editorial layer and the documents or texts it represents therefore
remain identifiable objects, not one flattened “correct text”.

Sources:

- [IDE/RIDE Criteria for Reviewing Scholarly Digital Editions, v1.1](https://www.i-d-e.de/publikationen/weitereschriften/criteria-version-1-1/)
- [Patrick Sahle, What Is a Scholarly Digital Edition?](https://books.openedition.org/obp/3397)

## 3. Fresh and current work

Lazzerini and Di Franco's May 2026 case study models textual variants,
corrective hands, manuscript families, and witness hierarchies in TEI and
makes the `constitutio textus` process visible. Its relevance to ToS is the
separation of witnesses and editorial decisions: a source reading can be
represented and compared before a preferred text is constituted.

Fischer and Monella's July 2026 Oxford Handbook chapter describes current
digital scholarly editing through transcription, textual criticism, structured
data modeling, annotation, TEI, IIIF, and transmedial publication. Berti and
Crane's companion chapter separately names edition structure, linguistic
annotation, translation alignment, and treebanks. These current surveys do not
support one all-or-nothing “accepted German” gate; they support typed layers
whose claims and responsibilities remain visible.

The 2025 evaluative work on digital-edition interfaces also preserves the
sequence source analysis -> data modeling -> transcription -> encoding ->
display. Presentation is downstream of the source and editorial method, not a
substitute for them.

Sources:

- [Lazzerini and Di Franco, Creating a digital critical edition of a classical text with XML/TEI](https://doi.org/10.60923/issn.2532-8816/23486)
- [Fischer and Monella, Digital Philology and Editing Texts](https://doi.org/10.1093/9780197835210.003.0040)
- [Berti and Crane, Philology and Digital Texts](https://doi.org/10.1093/9780197835210.003.0002)
- [User interfaces of digital scholarly editions: an evaluative framework](https://doi.org/10.1007/s42803-025-00102-y)

Freshness does not overturn the established source-first model. It strengthens
the need to expose witness hierarchy, editorial intervention, source images,
structured identifiers, and downstream alignment as distinct layers.

## 4. Exact Tree of Sophia evidence

The exact local DTA Part I TEI is already fixity-bound as
`tos.file.sha256.d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b`.
Its TEI header identifies:

- the DTA/BBAW publisher and named editors;
- a complete digitized edition published in its captured state on
  2025-02-28;
- the exact 1883 Schmeitzner source and SBB-PK shelfmark
  `19 ZZ 10200-1/3`;
- OCR acquisition followed by rechecking by native speakers under the DTA
  transcription rules;
- TEI P5/DTABf encoding and the current annotated-text licence statement.

The existing text-free triangulation then proves that the exact DTA section
`TEI/text[1]/body[1]/div[1]/div[1]` and the observed eKGWB locator
`Za-I-Vorrede-1` have the same twelve paragraph sequences and all 261
alphabetic tokens after the declared DTA printed-hyphen normalization. A third
automatic Naumann witness preserves one OCR replacement instead of being
silently corrected into agreement.

This is enough to admit an **edition-attested reading** for the exact DTA
section, with eKGWB as an independent critical-edition corroboration. It is not
enough to claim:

- publisher-authenticated eKGWB transport;
- identity between the 1883 and critical Editions;
- author-final or universal German text;
- German orthographic, grammatical, semantic, or etymological correctness as
  independently judged by the operator;
- accepted morphology, lemma, lexeme, translation, sign, concept, relation,
  graph, or canon state;
- permission to publish any local payload.

## 5. Admission ladder

ToS should use these distinct postures:

1. `source_identity_verified` — exact Work/Expression/Edition/Item identity,
   fixity, locator, and custody are bound.
2. `edition_reading_attested` — an identified scholarly edition or documented
   witness transcription supplies a source-returnable reading under a stated
   editorial method.
3. `linguistic_analysis_proposed` — morphology, lemma, meaning, etymology, or
   translation correspondence is a typed proposal with maker and evidence.
4. `linguistic_claim_reviewed` — the exact linguistic claim has competence-
   appropriate review; this is claim-specific, not a blanket source upgrade.
5. `sign_candidate` — recurrence, context, translation evidence, alternatives,
   and source returns support a candidate, not a sign.
6. `human_sign_decided` — a real human accepts, rejects, or defers the concrete
   candidate within declared competence and after alternatives are visible.

The first two postures concern source evidence. The next two concern language
and translation. The last two concern semantic promotion. No later posture is
inferred from an earlier one.

## 6. Contract consequences

The current semantic-ladder source gate is too coarse because it requires
language-competence evidence before even source-observational stages can
materialize. A corrected v4 contract should:

- replace `accepted_source_sha256` with an edition-local admitted-reading
  binding and retain the exact local source digest separately;
- add `source_reading_status: blocked | edition-attested`;
- keep `language_competence_status: blocked | evidence-attested` independent;
- allow exact form, source-return, frequency, context, and explicitly
  machine-proposed morphology/lemma/translation/sign-candidate stages from an
  edition-attested reading;
- forbid every `reviewed`, accepted-sign, interpretive, relation, concept,
  claim, graph, and canon effect while the relevant competence and real-human
  decision evidence is absent;
- keep the source payload local-only and the tracked packet text-free unless a
  separate publication route is cleared;
- schedule no routine human backlog merely because a source reading becomes
  available.

For the present exact section, ToS may create one edition-reading admission
packet and move only the observational semantic stages from `blocked` to a
source-grounded, model-made state. It must leave accepted German, accepted
translation, sign promotion, interpretation, graph truth, and publication at
zero.

## Decision

Adopt `edition_reading_attested` as a source-evidence posture distinct from
German-language acceptance.

The operator's missing German competence is not a reason to discard or hide
the human scholarship already embodied in an exact documented edition. It is
a reason to type the downstream claims honestly. ToS may say “this exact DTA
edition reads X and eKGWB independently attests the same normalized sequence.”
It may not say “X is linguistically or philosophically settled” without the
additional evidence required for that exact claim.

## Authority boundary

This research changes no source status by itself. It is a model-authored method
decision recommending a narrower and more useful evidence ladder. An actual
admission packet, contract update, and source-return verification must be
reviewed and validated separately. No source payload was added to Git or
authorized for publication by this research.
