# Translation Reference Research

Status: bounded research complete; eKGWB rights/transport refreshed
2026-07-30; forthcoming Stanford translation route added; zero reference
content admitted; human bibliographic and rights review pending

Research snapshot: 2026-07-30; eKGWB rights/transport refresh: 2026-07-30

Machine-readable register:
`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/translation-reference-register.v1.json`

Register SHA-256:
`70f6c15e0a21553a4c2eca8fcbd57b6efe7be3c252888ab93de03b421a417f5f`

Contract:
`ToS/contracts/translation-reference-register.schema.json`

## Question

Which dictionaries, historical corpora, Nietzsche-specific lexical resources,
critical editions, and recognized or additional translations are defensible
reference candidates for the Zarathustra translation laboratory, and what may
ToS actually do with each one?

The answer is not a bibliography of names. Every candidate needs a dated
identity, scholarly role, intended use, access state, rights posture, citation
rule, limitations, and an explicit admission state. A source can be excellent
scholarship and still be unavailable for corpus ingestion, derivative use, or
redistribution.

This packet supplements the general standards, leading-paper, and 2026-paper
review in `RESEARCH.md`. It preserves the same order while narrowing the
question to translation reference material:

1. classical and official documentation;
2. established scholarly reference resources;
3. current access, version, and rights surfaces.

No source text, dictionary article, translation passage, or subscriber-only
content was downloaded or copied during this research.

## Governing distinctions

1. **A work is not an expression.** “The Antonovsky translation” names a
   lineage, not one invariant Russian string.
2. **An expression is not an edition or item.** The 1911 fourth edition and
   the scientifically edited 2007 version must remain separately identifiable
   until textual comparison proves a narrower relation.
3. **Scholarly authority is not ground truth.** A critical edition, dictionary,
   or recognized translation is a high-value witness whose claims and readings
   remain citable and contestable.
4. **Open consultation is not open derivation.** Permission to browse, quote,
   or cite does not automatically permit bulk ingestion, transformation,
   publication, or model training.
5. **Metadata rights are not content rights.** A library record may be openly
   reusable while the described book remains restricted.
6. **A resource license is not an item license.** Corpus records and images can
   carry different rights even inside one institutional repository.
7. **Reference research does not break blindness.** The recognized Russian
   comparator remains sealed until the independent human, AI, alternatives,
   and AI+human lanes have been frozen in the required order.

## I. Classical and official documentation

### Historical German dictionary

[Wörterbuchnetz](https://woerterbuchnetz.de/) exposes the first edition of the
*Deutsches Wörterbuch* by Jacob and Wilhelm Grimm (1DWB, printed 1854–1960)
through a maintained academic surface involving the Trier Center for Digital
Humanities, BBAW, and the Göttingen academy. The BBAW
[lexical-resource inventory](https://fedora.dwds.de/de/lexik/) independently
describes 1DWB as its largest historical German dictionary resource.

Use: period-sensitive sense evidence and historical citations.

Limits:

- cite the exact article, cited source range, displayed version, URL, and
  access date;
- do not infer Nietzsche's intended sense from a dictionary entry alone;
- the landing surface did not establish a reusable license for article data,
  so ToS permits consultation and citation only until resource-specific terms
  are reviewed.

### Modern German control

The BBAW inventory describes the
[DWDS-Wörterbuch](https://fedora.dwds.de/de/lexik/) as a continuously expanded
contemporary meaning dictionary. It is useful precisely because it is not a
historical authority: it can expose where a fluent present-day reading may be
anachronistic.

Use: modern-sense control against historical witnesses.

Limits:

- record the exact article revision or access date;
- never replace 1880s evidence with a contemporary definition;
- the general BBAW repository statement says owner-produced resources are
  normally CC BY-SA unless otherwise stated, but that general statement is
  not enough to assign a license to a particular online dictionary article.

### Etymological dictionary

The same BBAW inventory documents the
[*Etymologisches Wörterbuch des Deutschen*](https://fedora.dwds.de/de/lexik/)
(EtymWB), developed under Wolfgang Pfeifer, with more than 22,000 headwords
and continued revision.

Use: externally sourced etymological evidence.

Limits:

- etymological history does not prove semantic intention in a Nietzsche
  passage;
- each claim needs the exact headword, revision or access date, and a second
  source when the history is contested;
- an LLM's recollection or paraphrase of an entry is not evidence.

### Historical corpus

The BBAW *Deutsches Textarchiv* (DTA) is a cross-disciplinary reference corpus
of German-language texts from approximately 1600–1900, with text/image views,
downloadable records, TEI P5 annotation, and item-specific rights evidence.
The full public part sequence has now been resolved as four exact DTA objects:
Schmeitzner parts [1](https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra01_1883)
and [2](https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra02_1883)
from 1883, [part 3](https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra03_1884)
from 1884, and Naumann's [first public part 4](https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra04_1891)
from 1891. DTA IDs `16840`–`16843`, exact holding copies, and four
checksum-bound local DTABf/TEI P5 items are preserved separately. Every live
header reports OCR followed by native-speaker checking under DTA's
transcription guidelines. That is stronger external source-quality evidence,
not ToS textual acceptance, whole-work equivalence, or a critical-edition
reading. The
[DTABf documentation](https://deutschestextarchiv.github.io/dtabf/einfuehrung.html)
defines a constrained TEI P5 profile and provides its own citation and CC
BY-SA 3.0 DE notice.

Use: a frozen late-nineteenth-century comparison slice for concordance,
collocation, and historical usage.

Limits:

- frequency is not sense, recurrence, or philosophical importance;
- every result must return to the exact document, source edition, and image;
- the DTABf documentation license does not license every DTA text or image;
- every selected item retains its own rights and citation record;
- the current DTA terms and all four TEI headers say CC BY-SA 4.0 for the
  annotated text, while all four live Dublin Core fields still carry the former
  CC BY-NC 3.0;
- DTA facsimile rights remain separate from the annotated text.

## II. Established scholarly resources

### Digital critical edition

The [eKGWB documentation](https://doc.nietzschesource.org/en/ekgwb) identifies
the digital edition as the Colli/Montinari reference edition of works,
posthumous fragments, and correspondence, edited digitally by Paolo D'Iorio.
It reports word-by-word collation, approximately 6,600 incorporated
philological corrections, XML-TEI encoding, whole-corpus search, and stable
sigla-based passage URLs.

The [Nietzsche Source rights page](https://doc.nietzschesource.org/en/rights)
declares CC BY-NC-ND 4.0: attribution and exact URL citation are required;
commercial reuse and derivative publication are not allowed without
permission.

Use: primary critical cross-check, exact citation, and Nietzsche-wide
recurrence search.

Limits:

- eKGWB does not replace the acquired Naumann scan or its source-visible
  review;
- search hits are observations, not semantic conclusions;
- ToS must not share a transformed eKGWB corpus under NoDerivatives.

The later
[`EKGWB_RIGHTS_AND_TRANSPORT_REFRESH.md`](EKGWB_RIGHTS_AND_TRANSPORT_REFRESH.md)
corrects the earlier, broader local-use assumption. CC BY-NC-ND 4.0 permits
non-commercial production and reproduction of private adaptations while
prohibiting their sharing. Private local indexing, embeddings, normalization,
and analysis can therefore be researched with full provenance and
source-return. This does not authorize publication, authenticate the current
HTTP response, admit a critical source, or establish German competence.

### Print critical route

The De Gruyter record for
[KSA Band 4](https://www.degruyterbrill.com/document/doi/10.1515/9783112419120-fm/html?lang=en)
and the current
[Nietzsche Online title list](https://www.degruyterbrill.com/publication/dbid/nietzsche/downloadAsset/NIETZSCHE_Nietzsche_Titelliste.pdf)
preserve the conventional print citation route for *Also sprach Zarathustra*
within the Colli/Montinari *Kritische Studienausgabe*.

Use: edition- and page-aware comparison with print scholarship.

Limits:

- publisher metadata is not access to the text or apparatus;
- exact edition and manifestation must be registered before page citation;
- content remains purchase-, library-, or subscription-controlled.

### Nietzsche-specific lexical interpretation

The publisher describes the
[*Nietzsche-Wörterbuch*](https://www.degruyterbrill.com/serial/nietzwtb-b/html)
as a four-volume project covering roughly 300 terms. Articles include
occurrence counts, uses, synonyms, senses, semantic components, contexts,
linguistic and philosophical history, research, and reception. The project is
edited by Paul van Tongeren, Gerd Schank, and Herman Siemens; new articles are
pre-published in Nietzsche Online.

Use: the strongest discovered Nietzsche-specific lexical witness after exact
German source acceptance.

Limits:

- coverage is selective, not a complete neutral word index;
- every article is an argued scholarly interpretation;
- access and reuse are restricted; the register prepares, but does not send,
  a written-access route through De Gruyter and the published feedback contact.

### Russian reference-witness lineage

The current local comparator is the 2007 volume of the complete works. The
[NEB catalog record](https://rusneb.ru/catalog/000199_000009_005395580/)
identifies volume 4, scientific editor E. V. Oznobkina, translator
Y. M. Antonovsky, 432 pages, and ISBN 978-5-250-06018-9. The local edition
states that the historical Antonovsky translation was newly edited and
checked.

The [NEB record for the 1911 fourth edition](https://rusneb.ru/catalog/000199_000009_003693383/)
provides a separately identifiable historical Antonovsky witness and exposes
full-view/download routes. Its availability makes it a high-value lineage
candidate, not an automatically cleared or equivalent text.

Use after blind drafts: compare editorial change, repeated signs, rhythm,
omission/addition, and interpretive intervention across exact expressions.

Limits:

- “Antonovsky” is not a stable-text identifier;
- the 2007 payload is local-only, in a sealed comparator lane, and has no
  redistribution authorization;
- the 1911 item has not been acquired, fixity-registered, or reviewed for
  item-level rights;
- neither version is absolute ground truth.

The local 1996 *Mysl* volume remains an additional Russian candidate. Its
[academic catalog record](https://ci.nii.ac.jp/ncid/BA35025034) corroborates
the two-volume publication and editorial responsibility, but the local volume
lists several translators. Work-specific translator attribution and textual
equivalence therefore remain explicitly blocked rather than being inferred.

### Additional English witnesses

The Oxford publisher record for Graham Parkes's
[*Thus Spoke Zarathustra*](https://www.oup.com.au/books/general-interest/humanities-social-sciences/9780199537099)
describes an annotated Oxford World's Classics translation concerned with
musicality and biblical and classical allusions.

The Cambridge publisher record for Adrian Del Caro's
[*Nietzsche: Thus Spoke Zarathustra*](https://www.cambridge.org/highereducation/books/nietzsche-thus-spoke-zarathustra/5E1FF7A96401587F59F12BDAEB9CEF93)
identifies Robert Pippin as editor, a 2006 first publication, DOI
10.1017/CBO9780511812095, and an explicit goal of preserving poetic form and
versification. Its
[front matter](https://assets.cambridge.org/97805218/41719/frontmatter/9780521841719_frontmatter.pdf)
also makes the in-copyright posture and later corrected printing visible.

Use after blind drafts: independent evidence about allusions, musicality,
versification, and translator intervention.

Limits:

- English is not the default bridge from German to Russian;
- neither publisher description proves passage-level quality;
- both translations are in copyright and not acquired;
- exact printing matters, especially where corrected reprints exist.

## III. Current access, rights, and version surfaces

### Nietzsche Online

The publisher's
[database concept](https://www.degruyterbrill.com/publication/dbid/nietzsche/downloadAsset/NIETZSCHE_Nietzsche_Konzept_Concept.pdf)
describes an integrated environment for corrected works and letters,
variants, commentaries, the *Nietzsche-Wörterbuch*, concordances, and secondary
literature. The freshest enumerated holdings surface found was the publisher's
[title list dated 2025-04-22](https://www.degruyterbrill.com/publication/dbid/nietzsche/downloadAsset/NIETZSCHE_Nietzsche_Titelliste.pdf).

This is potentially the richest integrated research route, but a 2025 title
list is not current entitlement evidence in July 2026. Before any experiment,
ToS must recheck live holdings, exact revision, institutional access, cost,
quoting scope, and export restrictions. The
[publisher portal](https://www.degruyterbrill.com/publishing/for-librarians/online-resources/database-portals?lang=en)
and `nietzsche-online@degruyter.com` form a prepared written-access route; no
request has been sent.

### Explicit word-index comparator

The [Deutsche Digitale Bibliothek record](https://www.deutsche-digitale-bibliothek.de/item/XD3K74FMFSAJPHGMVDG5PJMCTRYD7FCR)
for Karl Schlechta's 1994 *Nietzsche-Index zu den Werken in drei Bänden* was
updated on 2026-02-17. It exposes only the table of contents, identifies the
Hanser license, and binds the index to a non-KGW/KSA source edition.

That boundary is useful. The index can become an A/B comparator against
eKGWB search and Nietzsche Online only if lawful access is obtained and its
edition-specific omissions are measured. It must not silently define the
Nietzsche corpus.

### eKGWB live rights and transport refresh

The 2026-07-30 ordered refresh checked current official documentation and
law, established eKGWB scholarship, and the live host route. The resulting
model-authored rights record separates:

- private non-commercial adaptation, which CC BY-NC-ND 4.0 permits;
- sharing adapted material, which it prohibits;
- unmodified sharing that the license can permit, but which current ToS
  operator policy does not authorize for the captured payload;
- institutional preservation corroboration, now supported by an Arquivo.pt
  HTTPS replay with WARC lineage and a target block matching the current owner
  object;
- source admission, which remains blocked because neither the current
  HTTP-only owner route nor the archive's own non-validation posture
  establishes publisher-origin authentication;
- linguistic and translation acceptance, which remains competence-blocked.

The exact target block is byte-identical across the 2023 institutional
snapshot, the 2024-dated current owner object, and the 2026 owner include
(`f58a13c189bcf22db19fcbca2345e660a771656c89010c67d55eb4a00c4cc398`).
This is fixity corroboration, not source acceptance. The unsent access request
is retained only for a publisher-authenticated current copy or direct
publisher deposit and clarification of any server, quotation, or sharing
scope. It is no longer treated as necessary merely for private local
adaptation or as if no institutional archive witness existed.

### Freshness rule

At execution time, recheck all of the following even if this register remains
structurally valid:

- live access and product availability;
- resource revision and title-list date;
- license and terms pages;
- library entitlement or purchase cost;
- contact route;
- exact ISBN, DOI, printing, or database object;
- whether a downloadable item carries its own rights statement;
- whether the planned use is consultation, quotation, extraction,
  transformation, redistribution, or model input.

An early-2026 catalog update is evidence of a dated surface, not proof that a
resource remains unchanged.

### Forthcoming 2026 translation frontier

The current
[Stanford University Press record](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra)
lists *Thus Spoke Zarathustra*, Complete Works volume 7, translated with an
afterword by Paul S. Loeb and David F. Tinsley, for August 2026. The publisher
announces 424 pages, ISBNs `9780804728799` and `9781503647282`, notebook
variant cross-references, discussion of translation problems, and a key-term
glossary.

On 2026-07-30 this is metadata for a forthcoming work, not an inspectable
translation. The register therefore marks it as a deferred research
candidate. No review-copy request was sent. After publication, the exact
manifestation, actual publication date, contents, rights, independent
reception, and passage-level suitability must be checked before it enters any
comparison lane. Newness alone establishes neither recognition nor quality.

## Admission result

The register contains 15 candidates covering all nine required categories:

- historical German dictionary;
- modern German dictionary;
- etymological dictionary;
- historical corpus;
- Nietzsche critical edition;
- Nietzsche-specific lexical or word-index resource;
- recognized Russian translation candidate;
- additional Russian translation candidate;
- additional English translation candidate.

The result is deliberately **zero admitted content entries**:

- zero human bibliographic reviews;
- zero human rights reviews;
- one model-authored eKGWB rights/transport record separating private
  adaptation from shared derivatives;
- one institutional WARC-backed preservation corroboration receipt, with zero
  admitted content;
- zero permission requests sent;
- zero dictionary articles frozen;
- zero corpus slices frozen;
- zero critical-edition passages imported;
- one eKGWB section locator registered as metadata-only, with zero text
  capture and zero citation-witness admission;
- zero recognized or additional translation passages revealed;
- one forthcoming August 2026 English translation registered as metadata-only
  and deferred;
- zero accepted lexical senses or etymologies.

The laboratory remains blocked on publisher-origin authentication,
critical-source admission, and German-competent source acceptance. The
30-unit interface remains an unscheduled reusable option rather than a
standing human backlog. When a concrete source question triggers review,
reference admission still proceeds item by item: source acceptance does not
automatically clear publication rights, and a private-use rights route does
not establish a scholarly reading.

## Planned use order after source acceptance

For each accepted German unit:

1. return to the accepted scan page and diplomatic transcription;
2. compare the exact form against eKGWB by stable passage URL;
3. consult 1DWB and a frozen late-nineteenth-century DTA slice for historical
   sense and concordance;
4. consult DWDS only as a modern control;
5. source etymological claims from exact EtymWB articles plus competing
   evidence where needed;
6. search recurrence within Zarathustra and the Nietzsche corpus through
   eKGWB before using an edition-bound word index;
7. consult *Nietzsche-Wörterbuch* or Nietzsche Online only through lawful,
   versioned access;
8. freeze literal interlinear, human-only, AI-only, machine alternatives, and
   AI+human drafts;
9. reveal the exact 2007 Russian comparator and any separately admitted
   Russian or English witnesses;
10. preserve every post-reveal change, rejection, unresolved alternative, and
    source citation.

This order keeps the stable source beneath morphology, lexicography,
etymology, translation, signs, concepts, and graph projection without
pretending that the upper layers become immutable.
