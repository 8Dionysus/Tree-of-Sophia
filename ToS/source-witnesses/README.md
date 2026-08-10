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
├── places/
│   └── <place>/place.json
├── organizations/
│   └── <historical-organization>/organization.json
├── discovery/                              # ordered query/result evidence
├── access-requests/                        # public status + ignored private correspondence
├── server-import/                          # future no-upload/import boundary
├── chronology/                             # Work-owned, facet-specific chronology claims
│   └── <responsibility-or-tradition>/
│       └── first-publication/
│           ├── work-chronology-claims.jsonl
│           └── provenance.jsonl
├── catalog/
│   ├── agents.jsonl
│   ├── places.jsonl
│   ├── organizations.jsonl
│   ├── works.jsonl
│   ├── expressions.jsonl
│   ├── editions.jsonl
│   ├── collections.jsonl
│   ├── items.jsonl
│   └── claims.jsonl                       # generated source-returnable relation index
├── relations/                             # corpus-wide identity-ladder assertions
│   ├── work-expression/
│   │   └── work-expression-claims.jsonl   # Work --has_expression--> Expression
│   ├── expression-edition/
│   │   └── expression-edition-claims.jsonl # Expression --embodied_by--> Edition
│   ├── edition-item/
│   │   └── edition-item-claims.jsonl       # Edition --exemplified_by--> Item
│   ├── expression-derivation/
│   │   └── expression-derivation-claims.jsonl # Expression --is_derivative_of--> Expression
│   └── provenance.jsonl                   # digest-bound batch materialization event
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
│           │               ├── provision-activity-claims.jsonl # grouped place/agent/date statements
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

The corpus-wide `provision-activities/` route holds digest-bound provenance
for bounded Edition-owned provision claims; a named subbranch may own one
multi-Edition research episode while every claim stays beside its exact
subject Edition. Shared discovery episodes are recorded under
`discovery/provenance.jsonl`; neither provenance surface becomes claim truth.

`<responsibility-or-tradition>` is a navigational route, not an authorship
claim. Anonymous, disputed, collective, and tradition-owned works receive
speaking routes and explicit responsibility claims in the catalog.

## Identity and path boundary

Object and claim records own stable ToS IDs. The catalog is their rebuildable
navigation projection: `claims.jsonl` makes tracked membership, responsibility,
publication, provision activity, chronology, and identity-ladder assertions
queryable while preserving exact source file, source line, canonical claim
digest, evidence, maker, provenance event, and review posture. It is not a
second claim authority and cannot promote an unreviewed relation. The tracked
projection admits only `public` or
`public_metadata_only` packets; less-visible claims require an explicitly
reviewed public-safe derivative. Filesystem paths are human navigation and may
improve through reviewed migrations. A path change never silently changes
object or claim identity.

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

The first four identity levels also close over three explicit corpus-wide
claim families under `relations/`. Work records cite their exact
`has_expression` packets through `expression_claim_refs`; Expression records
cite `embodied_by` packets through `embodiment_claim_refs`; Edition records
cite `exemplified_by` packets through `exemplar_claim_refs`. The direct record
fields retain speaking structural topology, while the claim packets add stable
claim IDs, exact record/manifest evidence, maker, provenance, visibility, and
review posture. The validator requires bidirectional exact closure, not merely
that both surfaces happen to exist. These relations remain bibliographic:
`embodied_by` never implies textual identity, critical equivalence, accepted
source text, translation quality, or semantics.

Expression derivation is a separate evidence-bearing claim family rather than
a fourth structural rung. Its directed `is_derivative_of` packets identify a
reported source Expression, derivation kind, directness, statement basis, and
collation state. They are explicitly non-transitive, asymmetric, irreflexive,
and no-equivalence: chronology, edition numbering, a shared translator label,
or a generated graph cannot fill a missing historical edge.

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

In the current bounded corpus, all seven Nietzsche Work records close over one
`authored_by` claim to the GND-backed Friedrich Nietzsche Agent. Each claim
returns to its recorded authorial-witness discovery and research route and is
digest-bound by an annotation event. All seven remain model-made,
`public_metadata_only`, and `unreviewed`; neither the author-named directory nor
catalog presence supplies authorship truth by itself.

The exact 1911 fourth-edition Antonovsky Expression closes over its own
`translated_by` claim and a separate RSL/RuNEB Item. Proposed whole-page
anchors bind the title-page responsibility, edition and publication statement,
the distinct Energiya printer line, and Antonovsky's edition-history preface
to the exact local file digest. The preface names the 1900, 1903, 1907, and
current 1911 editions and says the three previous editions were not simple
reprints of their predecessors but new reworkings. It does not explicitly
establish a 1911-to-1907 derivation. That source-visible statement makes
edition separation foundational: no shared
translator label can establish an invariant Russian text. The 62,952,283-byte
PDF remains ignored and local-only; the tracked layer admits no embedded OCR,
equivalence, redistribution permission, or future-site payload.

The provisional 1900 Antonovsky Expression now closes over one exact
institutionally identified Edition. Current RSL and RNL records reconcile one
exact route through the shared `rc\\1717107` core, matching transaction value,
and bibliography. They report Saint Petersburg, year 1900,
`тип. Б.М. Вольфа`, extent `[4], XII, 624 с.; 16`, two RSL holdings, and RNL
holding `128/318`. The current LNB record agrees on title, responsibility,
place, year, and 624-page main extent and reports holding `1.40686`, but gives
`[s.n.]` and `[3], XI`. That discrepancy remains unresolved pending
source-visible pages. All four remote holdings are outside the ToS Item ladder:
no source copy or bytes were acquired, `exemplar_claim_refs` remains empty,
and no File, anchor, publisher identity, provision activity, source text,
rights clearance, collation, equivalence, or new derivation edge is admitted.
The tiered LNB request is refocused on the discrepancy but remains public-safe,
optional, and `draft-not-sent`.

The provisional 1903 Antonovsky Expression now likewise closes over one exact
institutionally identified Edition. The Russian State Library record reports
`2-е изд., испр.`, Saint Petersburg, `тип. Альтшулера`, year 1903, extent
`[2], III, [3], 432, [2], V`, and four physical holdings. A current RNL
crosswalk resolves the same bibliographic control core to Primo
`07NLR_LMS004843722`, system number `004843722`, and two additional holdings
under `129/5943` and `38.35.6.32`. The distinct identifier wrappers are not
collapsed, the six shelfmarks remain external coordinates rather than ToS
Items, and one undefined `TEMP` row is rejected. No record-specific digital
object, bytes, or source pages were acquired.
The typography string is not inferred to be a publisher and opens no
provision-activity claim. No responsibility claim, collation, equivalence,
new derivation edge, rights conclusion, accepted text, or semantic object is
created. Separate tiered RSL and RNL copy requests are public-safe and
`draft-not-sent`; the RNL digitization route remains recommendation-only.

The provisional 1907 Antonovsky Expression now closes over one exact
institutionally identified third Edition. The current RuNEB record reports
Saint Petersburg, `тип. Ф. Вайсберга и П. Гершунина`, year 1907, the RSL as
source institution, and code `000199_000009_003693382`; the live card also
says that the edition was removed or replaced. A later exact RNL Primo pass
supersedes the current-catalogue gap: record `07NLR_LMS004843723`, system
number `004843723`, and its public item rows expose three holdings in the
Russian Book Fund under `17.145.5.1`, `17.145.5.1а`, and `17.145.5.1 Б`, each
reported `В хранении`. The catalogue code `Web заказ` is preserved literally;
official RNL guidance does not establish actual online ordering for a 1907
edition, and the record-specific scan form is recommendation-only. The remote
holdings are not ToS Items, and no source pages or bytes were acquired. The
same exact RUSMARC response exposes `001` `v19\rc\1717109` and field
`899 |a RuMoRGB |j V 106/216`. Official RUSMARC and MARC organization-code
documentation support the narrow statement that RNL reports an RSL call
number; they do not confirm the current RSL copy. `V 106/216` is therefore
tracked as `RSL shelfmark reported by RNL`, while predicted RSL record
`01003693382` stays rejected at HTTP `404` and the separate `БАН` row without
a call number stays unresolved. The refocused RSL/RuNEB request asks for
institutional confirmation before any source-copy question and remains
`draft-not-sent`. The
typography string is not inferred to be a publisher and opens no provision
claim. No Item, File, exemplar claim, responsibility claim, collation,
equivalence, changed derivation, rights conclusion, accepted text, or semantic
object is created. All three 1907 requests remain public-safe and
`draft-not-sent`.

The exact 1913 Antonovsky Expression separately closes over one
`translated_by` claim. Its evidence is a proposed whole-page anchor on PDF
page 7, bound to the exact Item and file SHA-256 after direct source-visible
inspection. The source retains the displayed label `Ю. М. Антоновский` in
that historical claim evidence, but a later independent
identity pass resolves the Agent to `Юлий Михайлович Антоновский`: DNB GND
`123235553` joins the initials to `Julij Michajlovič`, translator occupation,
and *Tak govoril Zaratustra*, while RSL supplies the Cyrillic full-name bridge
and dates `1857-1913`. This does not establish textual equivalence among the
1911, 1913, 1981, 1996, 2007, and later Expressions. The anchor and claim are
model-made and unreviewed; neither admits OCR, source text, or translation
quality.

The 2007 Cultural Revolution Expression now closes over a separate
`translated_by` claim to the same stable Agent. Proposed whole-page anchors on
PDF pages 3 and 4 preserve both the direct translator credit and the fuller
statement that the historical translation was checked and newly edited. The
source-visible general editor, scientific editor/checker, commentary
translator, and designer remain exact deferred-role evidence because the
current predicates cannot express them without loss. No equivalence with the
1898, 1911, 1913, 1996, or later witnesses, accepted source text, translation
quality, semantics, rights clearance, or human review is inferred.

The three other translator Agents named by the 1996 Mysl boundary map follow
the same separation between a source credit and a person identity. An ordered
authority pass verifies Карен Араевич Свасьян as GND `120452367` and Николай
Николаевич Полилов as GND `1012315509`; their original initials remain
source-bound variants and their existing stable Agent refs do not change.
`В. А. Флёрова` remains provisional with no external identifier because the
official record chain stops at initials. `Вера Александровна Флёрова`, born in
1913, is explicitly rejected as a chronologically impossible match to a 1907
translator credit. Identity enrichment creates no new responsibility claim,
Expression equivalence, accepted translation, or human review.

Antonovsky also exercises the explicit identity/path boundary. The already
referenced `tos.agent.yuri-antonovsky` remains the stable object of four
responsibility claims, but the Agent file moves from the misleading human route
`agents/yuri-antonovsky/` to `agents/yuliy-antonovsky/`. `Yuri M. Antonovsky`
is rejected as a name expansion, and the surviving `yuri` token is documented
only as a legacy stable locator. The later 2007 claim reuses that verified
Agent without rehabilitating the rejected name expansion. The path migration
and claim reuse create no new Agent, text, or equivalence.

Those seven Work records also each close over one
`first_publication_chronology` claim under `chronology/`. This is a bounded
ordering facet, not a universal Work date: composition, manuscript, printing,
title-page dating, private issue, public sale, reception, and digitization
remain distinct chronologies. The staged *Zarathustra* profile preserves the
1883–1884 public parts and the private 1885 fourth part; *Götzen-Dämmerung*
keeps 1888 production separate from its 1889 public release; *Der Antichrist*
and *Ecce Homo* remain posthumous editorial publications. All seven claims are
model-made, `public_metadata_only`, and `unreviewed`.

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

An Edition may separately close over sibling `provision-activity-claims.jsonl`
rows through `provision_activity_claim_refs`. A provision claim groups one
typed publication, production, distribution, or manufacture statement with
its literal wording, role-specific places and agents, bounded temporal
assertion, evidence, provenance, and review posture. Literal transcription or
report and normalized identity remain separate: an authority match cannot
overwrite the statement, and the statement alone cannot manufacture a Place
or Organization identity. The original two-case slice resolves one
provisional Leipzig Place and two provisional historical publisher
Organizations for the 1889 *Götzen-Dämmerung* and 1908 *Ecce Homo* Editions.
It explicitly rejects the C. G. Naumann printing company and modern Berlin
Insel successor as publisher shortcuts. A later exact-Edition extension adds
the 1883 first part of *Also sprach Zarathustra*, a provisional Chemnitz Place,
and the historical `Ernst Schmeitzner, Verlagsbuchhandlung` Organization. It
keeps that Corporate Body distinct from the Person Ernst Schmeitzner. A
following pass adds parts II and III only from their own DTA source
descriptions and separate Edition identities; repeated labels, the part-I
claim, and the shared holding shelfmark are not evidence for propagation. A
fourth-part pass adds only the exact DTA-reported `Naumann; Leipzig; 1891`
statement. It reuses the provisional Leipzig Place and C. G. Naumann Verlag
Organization while rejecting the printer and founder Person as substitutions;
the 1885 private print, 1890 printing, planned 1891 delivery, and actual March
1892 delivery remain distinct historical stages. The Antonovsky 1913 pass then
adds two source-visible claims with a different shape: publication joins the
page-5 cover imprint to the page-7 place/year, while manufacture keeps the
page-8 brothers-Linnik printer line separate. Both reuse one provisional Saint
Petersburg Place but resolve to distinct provisional Organizations. All eight
claims remain model-made, `public_metadata_only`, and `unreviewed`. A final
consolidated-Edition pass adds two more claims from the exact 1893 Naumann title
page: its one combined `Druck und Verlag` literal is shared by separate
publication and manufacture roles. The provisional GND-backed publisher and
printer Organizations remain distinct normalization targets without a legal-
identity or `same_as` assertion. All ten Edition claims remain model-made,
`public_metadata_only`, and `unreviewed`.

The exact 1886 *Jenseits von Gut und Böse* Item adds a second shared-literal
case without copying either 1893 claim. Its page-3 `Druck und Verlag` wording
supports separate publication and manufacture claims through the same
role-specific provisional publisher and printer Organizations. A separate
page-4 anchor preserves `Alle Rechte vorbehalten.` only as historical rights
evidence. All twelve Edition provision claims remain model-made,
`public_metadata_only`, and `unreviewed`; no Edition statement, exact release,
textual equivalence, rights conclusion from the provision claim, or publication
authority is created. A later independent layer-specific assessment reviews
current rights evidence without changing any provision claim.

The 1892 second *Zur Genealogie der Moral* Item adds two more claims without
copying either shared-literal case. Page 5 anchors the publisher and Edition
year; page 204 anchors a separate, undated printer line. The title-page year is
retained as an explicitly bounded Edition statement year rather than an exact
manufacture or release date. All fourteen Edition provision claims remain
model-made, `public_metadata_only`, and `unreviewed`.

Provision chronology is also facet-specific. The 1889 title-page statement is
not the reported printing completion, authorial receipt, or public-sale date;
the bracketed 1908 DNB year is not an exact release date; and the part-IV 1891
DTA year is not its private-print, printing, planned-delivery, or
actual-delivery date. The generated graph
may connect a reified claim to normalized Place and Organization nodes, but it
must never emit a direct Edition-to-identity fact edge. The frozen two-case
A/B/C inspection and its negative controls are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/PROVISION_ACTIVITY_ABC_EXPERIMENT.md`.
The post-experiment first-part identity admission is separately recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PART1_PROVISION_IDENTITY_RESEARCH.md`.
The independently researched part-IV boundary is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PART4_PROVISION_IDENTITY_RESEARCH.md`.
The source-visible Antonovsky publication/manufacture split is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1913_PROVISION_IDENTITY_RESEARCH.md`.
The exact consolidated-Edition statement, shared title-page provision literal,
role-specific Naumann authorities, and historical translation-right boundary
are recorded in
`ToS/research-packets/foundation-laboratory-2026-07/NAUMANN_1893_PROVISION_IDENTITY_RESEARCH.md`.
The independent 1886 *Jenseits* title-page and historical-rights pass is
recorded in
`ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_PROVISION_IDENTITY_RESEARCH.md`.
The distributed 1892 *Genealogie* publisher/printer pass is recorded in
`ToS/research-packets/foundation-laboratory-2026-07/GENEALOGIE_1892_PROVISION_IDENTITY_RESEARCH.md`.

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

Question-scoped exact context remains a second private source-bearing layer,
not a tracked export. Its first control preserves a complete page-bounded KWIC
census under ignored mode-0600 `local-content/usage-context/`; only a
string-free plan, fixity/count/selector receipt, and provenance remain tracked
beside the lexical plan. The tracked records authorize neither source reuse nor
publication, and a context row is not a sentence, sense, lexeme, sign, or
semantic assertion.

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
- an exact 62,952,283-byte RSL/RuNEB PDF of Antonovsky's 1911 fourth edition,
  with 153 scanned spreads, separate Prometey publication and Energiya
  manufacture claims, and source-visible evidence that the earlier translation
  editions were revisions rather than simple reprints;
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
- a separate exact Internet Archive automatic DjVuXML derivative from the
  Google/Stanford scan lineage, with provider SHA-1 plus local SHA-256 closure,
  a text-free 525-page/5,434-paragraph/16,467-line/105,803-word inventory, and
  local-only navigation posture; contaminated descriptive metadata, OCR
  correctness, container identity, accepted German, and future-site upload
  remain explicitly unestablished;
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
154-page catalog conflict preserved. Its eleven-layer rights record separates
public-domain Nietzsche and Richter strata from van de Velde's German design
term through 2027-12-31, the exact mixed Commons object, Getty furniture,
licensed Commons metadata surfaces, and unresolved Internet Archive package.

The EPUB and image-container PDF are distinct acquired items descended from
the same scan family. The PDF supplies source-visible pages for independent
OCR; the EPUB text remains a sealed reference witness until variant outputs
are frozen. Their current layer-specific rights results deliberately diverge.
For the exact image-container PDF, the historical Work, Peter Gast preface,
1893 Edition presentation, Nietzsche portrait, Overbeck letter facsimile, and
faithful mass scan are `public_domain_reviewed` for DE/US; the separately
reported catalog metadata is not folded into that result. The automatic EPUB
retains positive historical, OCR, and extracted-image evidence but also
contains a later 2023 Internet Archive notice with no reuse license, plus
unresolved package, navigation, style, and metadata layers, so its aggregate
status is `conflicting_evidence`. Both operator-held files remain `local_only`,
redistribution `not_authorized`, derivatives `local_research_only`, and
unapproved for server transfer. A future public PDF candidate must be
independently reacquired and reviewed; a future EPUB must be newly built from
an accepted rights-positive source rather than obtained by stripping or
republishing this package.

The 1913 scan likewise preserves the source provider's public-domain
declaration as positive rights evidence rather than flattening it into either
“authorized” or “closed.” Its local copy remains off the future public site by
operator policy. The Wikisource transcription is an independent CC BY-SA
candidate layer and remains deferred: its index reports incomplete
proofreading, so open licensing does not make it accepted text or gold.

The 1911 RuNEB witness now preserves a deliberately mixed layered assessment.
Nietzsche's Work, Antonovsky's independently assessed 1911 translation and
preface, and the historical presentation are `public_domain_reviewed` for
RU/US; the United States result uses the foreign-work URAA source-country
condition rather than blindly applying the domestic pre-1931 rule. The exact
RSL/RuNEB scan and embedded text remain `copyright_undetermined`: current
institutional terms support bounded local research but expose no
resource-specific redistribution license, and the digital production and
correction history is absent. The aggregate Item stays `local_only`,
redistribution `not_authorized`, derivatives `local_research_only`, and
blocked from the future site. Only public-safe metadata, fixity, provenance,
text-free resource geometry, and proposed source-return anchors are tracked.

The Nani 1899 parallel witness now exercises the required per-layer rights
model. The exact record is freely downloadable and the current RuNEB FAQ says
that only public-domain works may be downloaded, so the underlying Nietzsche
work plus the published Nani translation and preface retain a positive
`public_domain_reviewed` assessment for Russia. That conclusion does not clear
the complete PDF: edition presentation, exact scan production, embedded text,
and handwritten annotations remain unresolved, no reusable-file license was
found, and no non-Russian serving jurisdiction was reviewed. The aggregate
Item/File therefore stays `copyright_undetermined`, `local_only`, and blocked
from the future site. Its `layer_assessments` are positive and negative
evidence, not a mechanism for lifting the most restrictive content-bearing
gate.

The four DTA TEIs add an institutionally corrected, source-structured German
sequence without inventing whole-work textual unity. DTA reports OCR followed
by native-speaker checking for every part, but ToS records that as external
quality evidence, not as its own linguistic acceptance. The current DTA terms
and TEI headers report CC BY-SA 4.0. Object-specific records now classify the
exact annotated TEIs as `licensed`, while every live official Dublin Core
field still reports the former CC BY-NC 3.0 and the distinct plain-text and
facsimile routes remain separate. The exact operator-held TEIs therefore stay
local, gitignored, and metadata-only for future server routing even though a
fresh, independently acquired upstream object has a positive licensed route.

The transfer seed now also includes the exact 274-page Google/Harvard scan
package of Nietzsche's 1886 Naumann *Jenseits von Gut und Böse*: the Internet
Archive PDF, its DjVu XML word-coordinate companion, and its compressed ABBYY
XML coordinate companion. All three provider files are checksum-reconciled,
gitignored, and local-only. The OCR companions are not independent textual
witnesses and are not accepted German; they exist to make structure experiments
reproducible without hiding machine navigation behind the PDF's embedded text.
The historical Work and Edition, faithful historical page scan, and automatic
historical OCR text are `public_domain_reviewed` for DE/US, but the generated
Google cover, Harvard holding furniture, coordinate XML, package structure, and
metadata remain unresolved. The exact three-file aggregate is therefore
`conflicting_evidence`, redistribution `not_authorized`, derivatives
`local_research_only`, and blocked from future-site transfer. A separate
Harvard/Hathi 1886 volume found through the current Institutional Books route
is not this physical Item and creates no identity or rights edge.

The second transfer-work source tree advances *Zur Genealogie der Moral*
beyond a provisional Russian collection-member expression. Its acquired file
is the exact 2020 Commons revision descended from the UNC/Internet Archive
scan of the 1892 second Naumann edition; the current Internet Archive PDF is
recorded as a different byte revision rather than silently equated. Current
DE/US research classifies the historical Work and Edition presentation, the
faithful historical page scan, and automatic historical OCR text as
`public_domain_reviewed`. Commons' structured file metadata is separately
`licensed` under CC0 1.0 and its unstructured page prose under CC BY-SA 4.0.
Those positive results do not flatten the mixed PDF: the physical binding and
marbled covers, UNC holding furniture, OCR layout, PDF package, and Internet
Archive metadata remain unresolved. The exact aggregate is therefore
`conflicting_evidence`, redistribution `not_authorized`, derivatives
`local_research_only`, and blocked from transfer. The provider page's current
`PD-US-expired` expression lacks an explicit German source-country tag; ToS
preserves that incompleteness alongside its independent bounded German term
analysis. The operator-held PDF stays local, its embedded OCR remains
unaccepted, and no German text, translation relation, semantic claim, identity
bridge, or server payload is admitted.

The third transfer-work route advances *Der Antichrist* beyond its
provisional Russian collection expression without flattening the newly found
aggregate. The source-visible 1906 Naumann volume, Stanford holding marks,
exact Commons fixity, and scan pages 228-329 support one proposed member
boundary; pages 1-227 and 330-523 remain explicitly unrepresented rather than
being mislabeled as non-work material. Internet Archive's contaminated
description is preserved as a conflict, while the 1906 archive-edition text
is kept distinct from Nietzsche's 1888 manuscript, the 1895 first printing,
a critical edition, accepted German, and provider OCR. Current DE/US research
classifies the represented historical Work, 1906 presentation, faithful
historical page scan, and automatic historical OCR text as
`public_domain_reviewed`; Commons structured metadata is separately CC0 1.0
and its unstructured prose CC BY-SA 4.0. The archive editorial layer remains
`conflicting_evidence`, while Stanford binding and holding furniture, OCR
layout, the DjVu package, and contaminated Internet Archive metadata remain
unresolved. The exact eleven-layer aggregate is therefore
`conflicting_evidence`, redistribution `not_authorized`, derivatives
`local_research_only`, and blocked from transfer. The 490 `TXTz` chunks were
counted but not decoded or accepted. The local DjVu is not a future-site
upload source, and the rights result does not accept the proposed member map.

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
positive but US-scoped evidence. Current DE/US research now assesses
Nietzsche's Work and Richter's editing and afterword as public domain, while
the source-visible title, binding, lettering, and ornaments attributed to van
de Velde remain in copyright in Germany through 2027-12-31. Commons carries no
German source-country tag, Getty exposes no exact CC0 record for this scanned
book, and the exact aggregate is therefore `in_copyright`, `local_only`, and
`not_authorized`. The local PDF is never a future-site upload source; a
post-2027 candidate must be independently reacquired and pass fresh rights,
fixity, quality, human-review, and operator gates.

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

Its current layered-rights pass now separates Nietzsche's public-domain Work,
the public-domain 1888 presentation, faithful historical printed-page capture,
the exact MDZ/Google digital object, and BSB/BDR bibliographic metadata. The
digital object is `permission_granted` under `NoC-NC 1.0`, which permits
non-commercial Item reuse but is a Rights Statement rather than a standard
license; the metadata is separately CC0. The PDF contains no embedded book
OCR. These positive results do not change local custody: the operator-held
file is never a future-site upload source, and the server plan remains
metadata-only, blocked, and operator-unapproved.

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

Its current layered-rights pass uses the same five-layer topology without
copying the *Fall Wagner* result. The historical Work, 1889 presentation, and
faithful printed-page capture are public-domain-reviewed for DE/US; the exact
MDZ digital object is `licensed` under CC BY-NC-SA 4.0; and BSB/BDR
bibliographic metadata is separately CC0. The PDF contains no embedded book
OCR. Attribution, non-commercial use, source/license and modification notices,
and ShareAlike for shared adapted material remain exact-object duties. The
operator-held file still stays off the future site, and its metadata-only plan
remains blocked and operator-unapproved until a separately reacquired current
object passes the full publication gates.

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

The same exact Mysl Item supplies two additional target structure routes
without exposing its prose. *Zur Genealogie der Moral* preserves 78 proposed
starts across four independently resetting series—preface and three essays—
while *Der Antichrist* preserves 62 starts in one series. Their series-qualified
identities prevent repeated numerals from collapsing into a false flat unit
namespace. The original target-only crosswalks narrow the twelve frozen pages
to twenty possible target routes and remain unchanged as historical inputs.

Separate German source maps now materialize the same 140 structural labels.
The 1892 *Genealogie* Item supplies 78 proposed PDF starts. The 1906
*Antichrist* route preserves a stronger boundary: the 523-page Commons DjVu is
the address witness, while a separately registered 525-page Internet Archive
DjVuXML Item is navigation evidence under a source-visible, bounded two-page
offset. This relation is not full-container or textual identity. A payload-free
intersection pairs only identical `series:unit` keys, and a second composition
gives all twelve frozen pages twenty possible German structural routes. These
source routes create no German source passage end, accepted German or Russian,
source-to-target passage alignment, translation relation, eligible transfer
unit, target gold, semantics, rights clearance, canon effect, or human work.

The frozen target side now has a separate exact-layer boundary preparation
step over the ignored Mysl PDF. It expands the twenty page candidates into
thirty-five numbered-unit passage candidates, using ordered PDF word geometry
from one visible unit label to the next same-series label. Thirty-two candidates
intersect their frozen page; three conservative spill routes (`Jenseits` 284 on
page 399, `Jenseits` 46 on page 279, and `Antichrist` 35 on page 661) are kept
as explicit rejected nonintersections instead of being silently discarded.
Private strings remain mode 0600 below ignored `local-content/`; Git receives
only selectors, geometry, counts, digests, proposed/rejected anchors, and
provenance. These are layer-exact target candidates, not diplomatic or accepted
Russian, exact German passages, passage alignments, translation evidence,
eligible units, target gold, semantic claims, canon effects, publication
authority, or new human work.

The German side now has a matching but deliberately partial exact-layer pass.
It keeps all thirty-five conservative routes and materializes thirty-two
private mode-0600 source candidates. Twenty-eight resolve inside one ABBYY,
DjVuXML, or PDF-bbox layer; four *Antichrist* routes use three exact visible
section markers in the same Internet Archive JP2 package, bound through its
scandata leaves to the first following DjVuXML line. Three routes remain
explicit unresolved-boundary records: *Jenseits* 32 and 201 and *Genealogie*
`essay-1:10`. Tracked artifacts remain text-free and retain content/address
witness separation; the *Antichrist* two-page navigation offset still asserts
no textual identity. The JP2 ZIP and scandata remain ignored local source
payloads. These candidates are not diplomatic or accepted German,
source-to-target passage alignments, translation evidence, eligible units,
gold, human tasks, semantic claims, publication objects, or canon effects.

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
