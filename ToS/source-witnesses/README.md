# Source Witness Corpus

`ToS/source-witnesses/` is the physical and authored evidence root for works,
expressions, editions, collections, acquired items, source-near text, and
aligned witness packets.

The exact division between project evidence and host-managed caches/runtimes is
recorded in [`LOCAL_STORAGE_BOUNDARY.md`](LOCAL_STORAGE_BOUNDARY.md).

It does not own semantic canon, laboratory runtimes, or generated graph/index
stores.

## Speaking topology

```text
source-witnesses/
├── agents/
│   └── <person-or-organization>/agent.json
├── discovery/                              # ordered query/result evidence
├── access-requests/                        # public status + ignored private correspondence
├── server-import/                          # future no-upload/import boundary
├── catalog/
│   ├── agents.jsonl
│   ├── works.jsonl
│   ├── expressions.jsonl
│   ├── editions.jsonl
│   ├── collections.jsonl
│   └── items.jsonl
├── works/
│   └── <responsibility-or-tradition>/
│       └── <work>/
│           ├── work.json
│           ├── responsibility-claims.jsonl # evidence-bearing Work/child responsibility bundle
│           ├── expressions/
│           │   └── <language-and-responsibility>/
│           │       ├── expression.json
│           │       └── editions/
│           │           └── <edition>/
│           │               ├── edition.json
│           │               ├── publication-claims.jsonl # evidence-bearing, review-gated publication assertions
│           │               ├── responsibility-claims.jsonl # optional Edition-local responsibility bundle
│           │               └── items/
│           │                   └── <item>/
│           │                       ├── item.manifest.json
│           │                       ├── resource-inventory.json # tracked pages/resources, no source text
│           │                       ├── fixity.sha256
│           │                       ├── provenance.jsonl
│           │                       ├── rights.json
│           │                       ├── forensic-report.md
│           │                       ├── structure/       # text-free item-specific maps + proposed anchors
│           │                       └── payload/        # gitignored bytes
│           ├── texts/                              # reviewed source-near text
│           ├── alignments/                         # multi-expression packets
│           └── lexical-indexes/                    # source-gated plans and text-free receipts
└── collections/
    └── <responsibility-or-tradition>/
        └── <collection>/
            ├── collection.json
            ├── editions/
            ├── membership-claims.jsonl
            ├── responsibility-claims.jsonl
            └── structure/
                └── work-boundaries/       # text-free member ranges + page anchors
```

`<responsibility-or-tradition>` is a navigational route, not an authorship
claim. Anonymous, disputed, collective, and tradition-owned works receive
speaking routes and explicit responsibility claims in the catalog.

## Identity and path boundary

The catalog and object records own stable ToS IDs. Filesystem paths are human
navigation and may improve through reviewed migrations. A path change never
silently changes object identity.

The corpus follows `work -> expression -> edition -> item -> file`. Aggregate
volumes route through `collections/` and point to contained works/expressions
with evidence-bearing membership claims. A collection is not duplicated under
every contained work.

Collection work-boundary maps bind the source-visible member order to one
exact item/file digest and its text-free page inventory. They may carry
whole-page title, table-of-contents, and non-work boundary anchors plus
an optional evidence-bearing expression-responsibility claim. Translation
responsibility remains explicit where applicable and is `null`, not
manufactured, for a source-language member without a separate responsibility
claim. Such a map is bibliographic structure, not accepted text, edition
equivalence, translation quality, semantics, rights clearance, or canon.

Work, Expression, and Edition records may close over evidence-bearing
`responsibility-claims.jsonl` rows through `responsibility_claim_refs`.
The initial typed predicates are `authored_by` on a Work, `translated_by` on
an Expression, and `edited_by`, `afterword_by`, or `designed_by` on an
Edition. Every object resolves to an Agent. Every claim is referenced by its
actual subject, its file is digest-bound by the cited provenance event, and
unreferenced or cross-subject claims fail validation. A role statement remains
a versioned claim: it does not collapse author, translator, copyist, editor,
paratext author, designer, publisher, or rights holder into one generic
“creator”.

An Edition may route its `publication_claim_refs` to a sibling
`publication-claims.jsonl`. Every row remains an evidence-bearing
`tos_claim_packet_v1`: the Edition is the subject, its provenance event and
evidence resolve, and its review state remains explicit. A title-page year,
printing date, authorial receipt, sale release, publication role, print-run
extent, and later edition or issue state are separate assertions. An issue
claim must preserve unresolved textual identity and difference when no
compared witness supports either conclusion. The claims' presence does not
turn a reported chronology into an observed fact, equate editions, create a
remote Item, accept a text, or promote anything to canon.

## Payload boundary

Only an item's `payload/` content is ignored by Git. Everything required to
identify, verify, understand, authorize, and reacquire that payload remains
tracked:

- item and file identity;
- original basename and media type;
- size and cryptographic digest;
- deterministic page, container-resource, or TEI-structure inventory without
  copied source text;
- acquisition/source record;
- forensic inspection;
- rights and visibility posture;
- preservation and transformation events;
- references from accepted source-near text and anchors.

Payload files are immutable after intake. Changed bytes create a new file
record and, when materially distinct, a new item or item version.

`resource-inventory.json` is a tracked mechanical companion generated from the
exact payload digest. It enumerates PDF page geometry and image counts, bundled
DjVu page order and geometry, EPUB member/spine order and member fixity, TEI
page-break/division structure, or provider DjVu/ABBYY OCR page geometry and
counts. Text-bearing EPUB, TEI, and OCR resources may carry only one-way
normalized fingerprints and character or word counts. The inventory cannot
accept a reading, settle an edition, clear rights, or expose source text.

Large working derivatives, model caches, OCR scratch, page renders, and
benchmark outputs belong to the `abyss-stack` laboratory or host-managed cache,
not beside the source payload. Reviewed text or annotation small enough to be
authored may return through `texts/`, `alignments/`, candidate intake, or canon
according to its authority layer and rights posture.

The first whole-work lexical observation index is a deliberately split
exception with no runtime authority. Its source-bearing SQLite/FTS5 database
stays inside the foundation pilot's ignored `local-content/`; its tracked
companion contains only form hashes, counts, and references to existing TEI
page/division resources. The form hashes are dictionary-recoverable
navigational fingerprints, not secrecy. The route remains `local_only` and
blocked from the future site until rights review, and it does not accept
German or create a lexeme, lemma, phrase assertion, sign, or semantic claim.

## Current seed

The first physical seed is *Thus Spoke Zarathustra* and one Nietzsche
collection containing it. The seed deliberately includes:

- a vector-outline Russian PDF requiring raster/OCR recovery;
- an image-plus-ABBYY Russian collection PDF with an imperfect text layer;
- an automatically generated German OCR EPUB with weak metadata and no usable
  navigation;
- the exact 529-page image-container PDF exposed by that EPUB's
  checksum-verified Internet Archive parent item;
- a checksum-verified 402-page scan of Yu. M. Antonovsky's 1913 Russian
  translation from Wikimedia Commons, kept distinct from both the later
  Antonovsky expression and the separately licensed Wikisource transcription;
- checksum-verified DTABf/TEI P5 transcriptions of all four public part
  witnesses from Deutsches Textarchiv: Schmeitzner parts 1 and 2 from 1883,
  Schmeitzner part 3 from 1884, and Naumann's first public edition of part 4
  from 1891. Each remains grounded in its exact SBB-PK or SUB Göttingen
  holding copy and distinct from DTA's facsimile layer and from a critical
  edition;
- an exact Wikimedia Commons revision of the UNC/Internet Archive scan of
  Nietzsche's 1892 second Naumann *Zur Genealogie der Moral*, with visible
  edition statement, exact file fixity, and a text-free 208-page resource
  inventory;
- an exact Wikimedia Commons/Stanford DjVu of the 1906 Naumann
  *Nietzsche's Werke*, Band VIII aggregate, with a text-free 523-page bundled
  DjVu inventory and an explicitly partial proposed boundary for *Der
  Antichrist*;
- an exact official MDZ PDF of the Bamberg Staatsbibliothek `30.1972` copy of
  the standalone 1888 Naumann *Der Fall Wagner*, with 74 ordered IIIF
  canvases, a one-page provider-cover offset, local fixity, and a text-free
  75-page inventory;
- an exact official MDZ PDF of the Bayerische Staatsbibliothek
  `Ph.pr. 881 kc` copy of the standalone 1889 Naumann
  *Götzen-Dämmerung*, with 164 ordered IIIF canvases, the same explicit
  provider-cover offset, local fixity, and a text-free 165-page inventory;
- the exact permanent Wikimedia Commons revision of the Getty/Internet
  Archive scan of the posthumous 1908 Insel *Ecce homo*, copy 965, with local
  fixity, a text-free 166-page inventory, a source-visible Nietzsche/Richter
  responsibility boundary, and the DNB 155-page versus IA/Open Library
  154-page catalog conflict preserved.

The EPUB and image-container PDF are distinct acquired items descended from
the same scan family. The PDF supplies source-visible pages for independent
OCR; the EPUB text remains a sealed reference witness until variant outputs
are frozen. Internet Archive's Public Domain Mark is retained as a source
statement, while ToS keeps the bytes `local_only` and
`copyright_undetermined` pending human jurisdiction and terms review.

The 1913 scan likewise preserves the source provider's public-domain
declaration as positive rights evidence rather than flattening it into either
“authorized” or “closed.” Its local copy remains off the future public site by
operator policy. The Wikisource transcription is an independent CC BY-SA
candidate layer and remains deferred: its index reports incomplete
proofreading, so open licensing does not make it accepted text or gold.

The four DTA TEIs add an institutionally corrected, source-structured German
sequence without inventing whole-work textual unity. DTA reports OCR followed
by native-speaker checking for every part, but ToS records that as external
quality evidence, not as its own linguistic acceptance. The current DTA terms
and TEI headers report CC BY-SA 4.0 while every live official Dublin Core field
still reports the former CC BY-NC 3.0; facsimile-image rights are separate.
The exact TEIs therefore remain local, gitignored, and metadata-only for
future server routing while the positive and conflicting evidence stays
visible.

The transfer seed now also includes the exact 274-page Google/Harvard scan
package of Nietzsche's 1886 Naumann *Jenseits von Gut und Böse*: the Internet
Archive PDF, its DjVu XML word-coordinate companion, and its compressed ABBYY
XML coordinate companion. All three provider files are checksum-reconciled,
gitignored, and local-only. The OCR companions are not independent textual
witnesses and are not accepted German; they exist to make structure experiments
reproducible without hiding machine navigation behind the PDF's embedded text.

The second transfer-work source tree advances *Zur Genealogie der Moral*
beyond a provisional Russian collection-member expression. Its acquired file
is the exact 2020 Commons revision descended from the UNC/Internet Archive
scan of the 1892 second Naumann edition; the current Internet Archive PDF is
recorded as a different byte revision rather than silently equated. Commons'
public-domain declaration is strong publication-route evidence, but the
operator-held PDF remains local, the embedded OCR remains unaccepted, and no
German text, translation relation, semantic claim, or server payload is
admitted.

The third transfer-work route advances *Der Antichrist* beyond its
provisional Russian collection expression without flattening the newly found
aggregate. The source-visible 1906 Naumann volume, Stanford holding marks,
exact Commons fixity, and scan pages 228-329 support one proposed member
boundary; pages 1-227 and 330-523 remain explicitly unrepresented rather than
being mislabeled as non-work material. Internet Archive's contaminated
description is preserved as a conflict, while the 1906 archive-edition text
is kept distinct from Nietzsche's 1888 manuscript, the 1895 first printing,
a critical edition, accepted German, and provider OCR. The local DjVu is not a
future-site upload source.

The fourth route advances *Der Fall Wagner* beyond its provisional Russian
collection expression through an exact standalone 1888 Naumann item. DNB fixes
the work identity; MDZ IIIF, B3Kat, the Bamberg holding furniture, and the
visible title page jointly fix the copy. HAdW's historical-critical commentary
reports one 1000-copy run whose latter half was nominally marked
`Zweite Auflage`; because this title page carries no such label, ToS preserves
the exact unlabelled issue and the distinction instead of flattening copies.
The MDZ `NoC-NC 1.0` statement supports bounded local non-commercial research,
not payload redistribution, accepted German, critical-text equivalence, or
server publication.

The fifth route advances *Götzen-Dämmerung* through the exact standalone 1889
Naumann BSB item. Its title and catalog records carry no edition statement,
while the scholarly publication history places Köselitz's textually altered
second edition in 1893. ToS therefore retains the 1889 object as a
first-publication witness without equating it with Nietzsche's manuscript or
a critical text. MDZ declares the exact digital object `CC BY-NC-SA 4.0`; that
positive evidence remains model-assessed and unreviewed, the operator-held PDF
stays local, and any future public route still requires current conditions,
attribution, ShareAlike, jurisdiction, and explicit operator review.

The sixth route gives *Ecce homo* its first distinct exact historical German
tree. It admits the 1908 Insel physical-copy witness without pretending that
the posthumous Richter/Köselitz expression is Nietzsche's lost author-final
state or a modern critical reconstruction. The visible book itself separates
Nietzsche's text through printed page 130 from Richter's separately titled
afterword beginning at printed page 133. Commons' `PD-US-expired` statement is
positive but US-scoped evidence; van de Velde's binding and ornaments and the
other object layers remain unreviewed. The local PDF is not a future-site
upload source.

The same route now exercises multi-role responsibility claims without
flattening the book into one creator. The Work points to Nietzsche through
`authored_by`; the exact 1908 Edition separately points to Raoul Richter
through `edited_by` and `afterword_by`, and to Henry van de Velde through
`designed_by`. All three Agent identities resolve to current GND identifiers,
and the four claims are model-made, unreviewed, subject-closed, and
digest-bound to evidence. This is bibliographic routing, not an author-final
text, a measurement of Richter's intervention, a rights conclusion, graph
truth, or canon.

The metadata-only authorial follow-up gives that publication witness an exact
upstream and critical comparison route without pretending ToS holds the
remote objects. Current GSA ORES resolves Nietzsche's D 25 print manuscript
as `GSA 71/32`, the Köselitz/Peter Gast D 25a copy as `GSA 71/33`, the
surviving replacement-section copy as `GSA 102/734`, the undigitized
deletions/change file as `GSA 102/735`, and four relevant notebook/loose-leaf
complexes. DFGA supplies matching book identities; eKGWB supplies stable
critical section addresses. Since no payload was acquired, no new
Expression/Edition/Item branch is manufactured. The route admits only remote
identity and provenance evidence, not author-final reconstruction, accepted
German, rights clearance, or publication.

The *Zarathustra* golden kernel now has the same responsibility-aware
upstream discipline. Current GSA ORES and DFGA records separate mixed
compositional notebooks from the surviving authorial part-IV print manuscript
D 17 (`GSA 71/25`), the 1885 private print, and the separately corrected Peter
Gast ensemble (`GSA 71/25a`). The current critical route reports that print
manuscripts for parts I-III are not preserved. Whole-notebook work membership
is therefore prohibited: an exact region and sourced attribution claim are
required. The eKGWB route supplies 181 stable critical comparison addresses,
not an admitted text or manuscript map. No remote manuscript or print becomes
a local Item without custody, fixity, rights evidence, and an acquisition
event.

The first transfer work, *Jenseits von Gut und Böse*, now has a matching
metadata-only route. W I 3-W I 8 and the Mp XV/Mp XVI complexes remain mixed,
region-addressed material; D 18 (`GSA 71/26`) is a reassembled print
manuscript rather than an immutable original; HAAB C 4615 and C 4619 are
distinct proof/correction witnesses; and the existing 1886 local Item remains
separate from the 302-address eKGWB critical surface. ORES/Kalliope and DFGA
leaf, canvas, sheet, image, and logical-ID counts are representation-specific.
The deferred §22 route may open only for a concrete source-visible question
with rights and German-competence gates; it does not create a whole-notebook
membership claim, source text, semantic edge, duplicate Item, or human queue.

The second transfer work, *Zur Genealogie der Moral*, now has its own
metadata-only documentary route. It keeps region-only W II 1 evidence, D 20a
and D 20b, partial K 11/C 4616 correction sheets, E 40, unresolved later
copies, the existing 1892 second-edition Item, and critical addresses
distinct. The December 2025 Basel edition is registered as a remote scholarly
representation with source-surface navigation, not another historical witness
or a local Item. No open viewer, IIIF placeholder, or digital-edition route
admits its source body, collapses responsibility, creates transfer A/B/C, or
opens a human queue.

The third transfer work, *Der Antichrist*, now also has a metadata-only
documentary route above the existing 1906 aggregate Item. It keeps exact
W II regions, D 22 (`GSA 71/29`), the independent Overbeck A 311 copy, the
Koegel/Naumann 1895 first print, the later archive-edition Item, and critical
addresses distinct. D 22's leaf order and mixed reverse-side evidence may not
be silently normalized. GSA 71/32 fol. 47 is recorded separately as a
contested associated adjunct: its physical D 25 container, proposed
*Antichrist* association, and later editorial placement are different claims,
and it is not `AC 63`. Open IIIF and Public Domain Mark evidence creates no
admitted source body, accepted German, author-final reconstruction,
publication permission, transfer unit, semantic object, or human queue.

The exact 1888 *Der Fall Wagner* Item now also has a metadata-only
documentary route. W II 6 (`GSA 71/162`) and W II 7 (`GSA 71/163`) remain
mixed notebooks whose work relation requires exact regions and inscription
layers. `D 21 / GSA 71/28` is explicitly *Götzen-Dämmerung*, not
*Der Fall Wagner*. The June print manuscript and July fair copy are lost;
`NL 200 : II, 1` is the sole surviving one-leaf correction/addition fragment,
not either complete manuscript. Nietzsche's changes and later
Köselitz/Lauterbach notes remain distinct. The unlabelled existing Item and
nominal `Zweite Auflage` half of the 1000-copy run have no presumed textual
identity or difference. Open archive routes create no admitted source body,
accepted German, author-final reconstruction, publication permission,
transfer unit, semantic object, or human queue.

The 1888 Edition now exercises `publication_claim_refs` through six sibling,
unreviewed claim packets. The title-page year is observed; author-supervised
publication role, September printing completion, 22 September publication,
the single 1000-copy run, and the nominal later issue state remain scholarly
reports. The issue-state object explicitly keeps both textual identity and
textual difference `unresolved`, records that the current Item is unlabelled,
and creates no separate Item. The claims are digest-bound to provenance and
evidence; they are not human acceptance, collation, critical equivalence,
publication authority, a canonical graph, or a signal to bulk-populate every
Edition.

The exact 1889-dated *Götzen-Dämmerung* Item now also has a metadata-only
documentary route. W II preparatory notebooks remain mixed surfaces whose
work relation requires exact regions and inscription layers. D 21
(`GSA 71/28`) is the positive autograph print manuscript for E 42, but it
does not erase the additions sent after 7 September, proof-stage insertions,
or the late migration of *Was ich den Alten verdanke*. Köselitz's title
suggestion and Nietzsche's adoption remain distinct responsibility events.
November 1888 printing and authorial receipt, the 1889 title-page date, and
late-January 1889 sale remain distinct chronology claims. The
Köselitz-edited, textually changed, non-author-approved 1893 second edition
and the modern *Magnum in parvo* editorial reconstruction remain separate
downstream routes. Open archive records create no admitted source body,
accepted German, synthetic author-final text, publication permission,
transfer unit, semantic object, or human queue.

The 1889 *Götzen-Dämmerung* Edition also exercises the existing
`publication_claim_refs`
contract through six sibling, unreviewed claim packets. They keep the
title-page year, printing completion, authorial receipt, public-sale release,
first-publication role, and the separate changed 1893 editorial state
queryable without collapsing them into one date or one immutable text. The
claims are digest-bound to provenance and evidence; they are not human
acceptance, a remote 1893 Item, critical equivalence, publication authority,
or a canonical graph.

These items are laboratory witnesses, not assumed critical editions. Their
catalog, rights, and forensic records must remain honest about what is known,
claimed, inferred, and unresolved.

## Source-near movement

```text
payload + receipt
  -> forensic inspection
    -> raw extraction or OCR proposal
      -> anchored manual correction
        -> reviewed text / alignment
          -> candidate observation or claim
```

Do not skip from acquired file to semantic canon.

## Rebuild local resource inventories

The payloads may be absent from a public clone, so the inventory builder is a
focused local operation rather than a release-gate download. Its authoritative
local invocation and explicit payload-root requirement live in
[`scripts/AGENTS.md`](../../scripts/AGENTS.md).

Omit `--check` only when intentionally regenerating tracked inventories from
the same fixity-verified local payloads. The source-foundation validator checks
their schema, manifest closure, counts, and digest-bound provenance without
claiming content correctness.

## Witness structure correspondence

The tracked Naumann 1893 ↔ DTA-parts map under
`works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/`
uses named primary TEI division starts and exact EPUB/PDF resource locators.
Its generator transiently reads the local payloads but writes only resource
IDs, paths inside the containers, one-way fingerprints, and match metrics.

The companion `structure-anchor-set.json` and `structure-anchors.jsonl`
materialize three stable proposed addresses for every correspondence: the DTA
TEI structural path, the exact Naumann EPUB member, and the whole Naumann scan
page. These are reusable IDs for later source review; they contain no source
text and remain `proposed`. A binding between the three addresses means only
that the structure-map method selected them as one candidate route. It does
not turn a page or container member into an exact passage boundary.

Every result remains a `mechanical_candidate_only` locator. A unique normalized
heading, context score, monotonic route, or matching page number does not prove
textual identity, edition equivalence, correct German, translation
correspondence, semantic correspondence, or canon fitness. The local rebuild
and tracked validation routes live in [`scripts/AGENTS.md`](../../scripts/AGENTS.md).

The Jenseits Naumann 1886 ↔ Polilov/Mysl 1996 map under
`works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/`
uses the more general parallel-PDF contract. It binds eleven source-visible
division starts—preface, nine numbered main divisions, and aftersong—to exact
pages in both fixity-bound witnesses. The nine main divisions cover the
contiguous integer ranges 1–296 and preserve the source supplemental units
`65a`, `73a`, and `237a`.

The source item now carries a separate, text-free
`structure/numbered-unit-page-map.json`. An A/B/C comparison rejected embedded
PDF text as a complete structure source, retained DjVu XML as a secondary
coordinate layer, and selected ABBYY XML for ordered candidates. Bounded
source-visible review closed the remaining numeral gaps and exposed a repeated
printed `237.` on PDF page 189, retained locally as `237a`. The result is 299
monotonic proposed start-page candidates for §§1–296 plus 65a, 73a, and 237a,
with 299 whole-page proposed source anchors.

This does not turn the OCR into text or a page into an exact line boundary.
The parallel German ↔ Russian map still stops at division granularity and
materializes zero exact target-unit starts. In the Mysl witness, the prose
corresponding to source `237a` follows the Seven Sayings without a repeated
237/237a label; that visible asymmetry is preserved instead of being forced
into an equivalence.

The Polilov/Mysl expression now also carries its own target-only
`structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json`.
Its A/B/C route rejects plain embedded text as a complete map source, rejects
unordered bbox numerals without an order constraint, and selects ordered bbox
candidates plus bounded source-visible gap review. It materializes 298
monotonic proposed target-label starts for 1–296, `65a`, and `73a`, with the
absent target label `237a` recorded only as a source-map asymmetry. This layer
does not pair any target unit to a source unit. Shared order, numbering, or
separate proposed page addresses do not establish textual identity,
critical-edition equivalence, translation alignment, translation quality,
semantics, or acceptance.

A release-safe companion under the existing parallel-structure route now
intersects only the number-label keys already materialized independently by
those two maps. It yields 298 proposed shared-label pairing candidates and
keeps source-only `237a` unpaired. The builder reads tracked maps and rights
records, not either payload or text. A pairing means only “both witnesses
materialize this structural label”; it is not an exact passage alignment or a
translation claim.

The Zarathustra foundation pilot also carries one deliberately narrower
German source-triangulation packet. A deterministic local builder compares an
ignored eKGWB HTTP response, the exact DTA part-I TEI, and two exact members of
the Naumann automatic EPUB, then emits only fixity, selectors, aggregate
counts, and one-way fingerprints. It preserves the failed HTTPS route,
unencrypted-transport risk, a source-aware dehyphenation control, and the OCR
residual. Machine agreement is reusable evidence, but it does not admit the
critical edition, accept German, authorize publication, open translation, or
create semantic authority.

A later tracked discovery receipt adds a distinct evidence layer: Arquivo.pt
preserves a 2023 snapshot of the same owner URL with capture timestamp,
collection, archive digest, WARC file, and offset. Its exact target block
matches the current owner object and include. This is independent historical
fixity corroboration; because the archive explicitly does not validate
archived source or content, it is not publisher-origin authentication, source
admission, German acceptance, or publication authority.

One derived sentence from that route is now preserved below the pilot's
ignored `local-content/translation/research-inputs/` branch. Its tracked
companion records the DTA selector, transformations, fixity, token count,
three-witness corroboration, rights posture, and closed authority gates
without copying the sentence into that packet. A matching string and authored
translations already exist in the older canon node and public compatibility
mirror; the calibration packet binds and seals those surfaces instead of
claiming global text absence. This makes the exact local string eligible for
blind `abyss-stack` model/prompt/runtime calibration only. It does not make
the string philologically accepted, revalidate the older canon role, open any
lane in the frozen translation plan, reveal a translation surface, authorize
redistribution, or create semantic, graph, or canon evidence.
