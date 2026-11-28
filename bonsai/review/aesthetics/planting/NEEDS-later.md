# Needs from the later direction

The earlier line (`kit.py` at 6c78ee512) admitted only an original public-domain
witness, rejected additional languages and accepted publication through 1929
and deaths through 1942 only. The owner's 12 November brief calls for:

- jurisdiction-specific public-domain and short-quotation bases with evidence
  for each participant, printing and lawful source;
- original plus the reader's language (ru, en, es, de, zh-Hans), including
  source-bound translated places and provenance for modernized Russian;
- a way to record actual, bounded searches without treating an unsearched
  translation as absent, and to admit lawfully sourced whole-sentence quotes.

PR #432 landed these mechanisms on 19 November. Complete source research,
territorial evidence and independent review remain necessary for each packet;
the legacy `owes` result described below is not evidence of that completion.

## Concrete evidence for the implementation

- `akane-ohashi.beyond-the-baby-schema`, DOI 10.1371/journal.pone.0340903:
  publisher JATS carries CC BY 4.0 and the actual target is Abstract / Discussion.
  Model coauthors individually and bind each place to this exact licensed version.
- `hiroshi-nittono.the-power-of-kawaii`, DOI 10.1371/journal.pone.0046362:
  publisher JATS says CC Attribution without a version; Crossref gives BY 4.0
  for an unspecified content version. Preserve the discrepancy for review.
- `howard-phillips-lovecraft.supernatural-horror-in-literature`: the 1927
  *Recluse* pp. 24–25 and Suhrkamp's licensed-access 2017 sample p. 12 provide
  corresponding source places. Its German translation needs short-quotation
  assessment and a page-image transcription, not a claimed open licence.
- `charles-sanders-peirce.what-is-a-sign`: MS 404 (1894) includes sections
  published at different times; the author death year and manuscript date
  alone cannot establish a single US rights basis. Navarra's Rivas translation
  is an institutional web text with section numbers but no printed pages;
  allow explicit structured locators where the source has no pagination.
- `george-santayana.the-sense-of-beauty`: local source E027/E028 point at §11
  in the 1896 edition, pp. 49–50; EU, Russian and Mexican transition rules
  need evidence-bound handling for each version rather than one global basis.

The detailed candidates and remaining searches are in
`bonsai/review/aesthetics/availability/later.json`; `research-pending` is an
unsearched state, never an assertion that a text does not exist.

- `heinrich-woelfflin.kunstgeschichtliche-grundbegriffe`: on 19 November the
  existing maker actually stopped at the 1942 death cutoff (Wölfflin died in
  1945), without preparing places. The owner assigns this work as available;
  assess the actual Russian, EU, US and Mexican rules and participants instead
  of treating the earlier first-wave cutoff as an exhaustive rights judgment.
  Heidelberg's 1915 edition and p. 57 are available at
  https://doi.org/10.11588/diglit.27250#0075; the translation search remains
  incomplete. Its blocked packet is a work-in-progress, not an absence record.

## Legacy packets after #432

On 19 November the new offline gate reports `gate: open, owes 7` for the
unaccepted Pope and Knight packets: older single-basis rights, undocumented
Russian absence/searches and missing edition locator, plus no es/de/zh-Hans
search record. The owner's direction requires real investigation and original
plus each reader language, so these `owes` are not evidence of readiness.
Please provide a maker route to complete legacy-packet rights and witnesses
before second-reader acceptance; `drive.py line --retake` otherwise advances
an already open gate directly to the second reader. Full search evidence is
in the later availability reports; no absence should be inferred.

## Licensed work with a coauthor lacking Wikidata

`winfried-menninghaus.towards-a-psychological-construct-of-being-moved`, DOI
10.1371/journal.pone.0128451, is a publisher version under CC BY 4.0. The
maker documented that Milena Kuehnast is named by both PLOS and PubMed but no
matching Wikidata person was found; `kit.py person` requires a QID or integer
`--died`, while the gate asks for every coauthor. Please support a
source-backed named participant with an explicitly unknown death date for a
licensed or short-quotation witness. Do not infer death or reuse a wrong QID.
The maker's `blocked.json` preserves the exact attempted sources; other
language searches and independent review remain outstanding.

`heinrich-woelfflin.kunstgeschichtliche-grundbegriffe` was technically
accepted with Russian marked as lacking source text and image. A publisher
preview https://books.yandex.ru/books/zlx1UDWc/read-online actually contains
the exact corresponding complete sentence in the 2023 revised edition of
Adrian Frankovsky's translation. Its page image, editor and territorial
rights still need checking; it is not proof of the 1930 edition's wording.
The locally accepted packet is withheld from planting pending correction,
complete search evidence for the other languages and independent review.
Please support reopening a technically accepted but not yet planted packet
for a material witness discovered after review, without silently accepting
the older missing-language claim.

The current witness workflow cuts an independently published translation
from a lawful source and requires page evidence. For a source under an open
licence or in the public domain where no suitable existing translation is
found, the direction also needs a route for a new translation with explicit
author, licence, source sentence, source image and independent linguistic
review. Such a translation must not be represented as a historical edition;
its use does not turn an incomplete search for existing editions into an
absence finding.

The accepted but unplanted `de-witt-henry-parker.the-principles-of-aesthetics`
packet records 124 language search outcomes generated from site-restricted web
searches. A read-only audit found these do not establish internal catalogue or
full-text searches, and its Russian, Spanish and German `book_url` fields
point to the English original rather than editions in those languages. The
Chinese 1965 Zhang Jin translation is catalogued but its target pages have not
been obtained. The gate returns `open`; the direction is withholding the
packet until the owner's required search and locator evidence is met.

`alison-hills.aesthetic-testimony-understanding-and-virtue`, DOI
10.1111/nous.12344, has an Oxford-hosted version-of-record PDF at
https://ora.ox.ac.uk/objects/uuid%3A111a83ff-75f2-4e0c-8cb7-46119b5efd71/files/rnk322f032
and Crossref identifies CC BY 4.0 for that version. The maker and independent
research have the actual PDF and target printed pages 23, 26 and 30, but the
line cannot turn a fetched PDF into a source-bound transcription and page
images; the publisher HTML/XML endpoints returned 403. Please provide a
versioned, digest-bound route to extract text and render selected PDF pages,
with the original PDF retained and both readers checking the page. The maker's
blocked packet documents attempts; translation searches remain incomplete.

The independent Parker audit also found the 124 `outcome` entries written in
one direct `json.dump` operation in the maker log, rather than through the
owner-mandated kit commands. Please make the kit-only packet-writing route
and its instructions consistent for new work; retain the evidence and repeat
review after correcting the packet. Audit: `audit-parker-31-20261119.md` in
the direction's external availability reports.

Follow-up on the PDF case: `kit.py page --file` does accept a locally rendered
PNG with a lawful PDF URL and records the image digest, so a direct image URL
is not technically required. This has only been verified from the command
implementation and image validation, not through a complete packet/gate. The
record does not by itself bind the rendered image digest to the source PDF's
digest and rendering operation. Please clarify or implement that provenance;
`fetch transcription --file` describes bytes fetched from its URL and must
not silently label locally extracted text as the original PDF bytes.

## Source-backed witness participant correction — 27 November

In the unplanted Pope packet, the 1733 German witness records the original
writer from DNB as `Pope, Alexander`, with a fetched source and dates. The
original packet records `Alexander Pope` / `Александр Поуп` with Q164047.
`witness-gate --second` does not match the inverted catalogue name to either
original name. `witness-person-remove` only accepts a QID, while the sourced
person has none; `witness-person-source` replaces only an exactly identical
name object. Please provide a history-preserving kit route to correct or remove
a source-backed person without a QID, then require renewed independent review.
The two e-rara page images and initial independent reading are retained in the
local witness; no acceptance or planting is claimed.

## Complete quotation whose final period follows attribution — 27 November

Lovecraft's Spanish candidate `I p24b` comes from Andrés Torres-Scott's
translation in footnote 9, printed p. 44 of the UNAM journal article
https://www.redalyc.org/journal/640/64069682002/64069682002.pdf#page=9.
The quoted words form the complete original sentence, but the printed period
follows the parenthetical `(traducción del autor)` outside the closing quotation
mark. The kit's `witness-gate` requires the stored translation itself to end in
sentence punctuation. Adding a period to the cited wording would invent a mark;
including the attribution parenthesis inside the translation would misattribute
it to Lovecraft. Please provide an evidence-bound representation of this case,
so both sentence completeness and exact source punctuation can be reviewed.
The parent work, this partial Spanish witness, the Mexico quotation assessment,
and the remaining language searches are not yet accepted.

## Recover a source scan binding after external page registration — 28 November

In the local `george-santayana.the-sense-of-beauty` packet the maker compared
both places against page-indexed Archive OCR, then used `kit.py page --file`
with an image from that very Archive scan. This assigns the synthetic
`external` scan to each place; `scan` changes only the default, `image` uses
`setdefault`, `cut` retains the old binding, and `scan-withdraw external`
refuses while places reference it. `compare` consequently does not use the
available Archive OCR even when `find` locates the same text in the named
scan. Please provide a source-evidenced, history-preserving kit command to
rebind a place to an already registered scan, retaining its page images and
requiring recomparison and a fresh second reading. Do not treat a supported
rebind as proof of textual agreement. The local packet remains blocked; its
translations and independent review are incomplete.

## Spanish opening question mark in complete quotations — 28 November

The local Croce Spanish witness now has a complete question at `I p11b`,
from Sánchez-Rojas 1912 printed p. 55 (PDF p. 55): the printed sentence starts
`¿Cómo ...?`. The independent second reader checked the page and the Italian
alignment, but `witness-gate --second --accept` rejects it as lacking an
uppercase or uncased initial. `sentences` strips `OPENING` before testing the
first letter, and `OPENING` does not include the Spanish inverted question
mark `¿` (the corresponding `¡` also needs consideration). Please support
faithful complete Spanish punctuation in the owner's sentence validator, with
appropriate focused coverage. Do not drop `¿` or alter the witness to pass.
The witness remains unaccepted until the owner fix and renewed gate; the
parent packet and other languages remain incomplete.

## Kendall L. Walton: conflicting register life date

`registry/B/agents.jsonl` row C-A013 says `1939–2013`, while his University of Michigan curriculum vitae lists Stanford visiting professorships in 2015–2019 and an emeritus position from 2012 onwards: https://sites.lsa.umich.edu/kendallwalton/wp-content/uploads/sites/225/2025/10/Kendall-Walton-CV.pdf. The CV was opened and read; it contradicts death in 2013, but does not itself establish a current death date. Register C separately says born 1939. The registry preserves workbook rows as imported, so its historical text should not be silently rewritten. Please resolve the conflicting author date through the owning source/correction mechanism and refresh any affected derived chronology or rights classification. Until resolved, do not treat 2013 as an established death year or infer public-domain status from it.

## Mexico quotation where the three recorded bases are public domain

Kandinsky's 1912 German text is recorded as public domain in Russia, the USA and the EU; that does not establish Mexican public-domain status. The maker needs to assess a short quotation under Mexico's LFDA article 148(I) for the exact selected places. `kit.py mexico-quotation` currently rejects the assessment with `record a short-quotation basis before the Mexican quotation assessment` unless at least one of those other three jurisdictions is marked `short-quotation`. Please allow a separate Mexican assessment and independent review without misclassifying a public-domain basis elsewhere. Until then the Mexican assessment remains open; the source and the other jurisdictional bases must not be falsified to open the gate.

## Publisher-authored HTML summary as the original source

The local Lara Harb `arabic-poetics` packet draws C:A-I102 from the author's
chapter 5 Summary published at https://www.cambridge.org/core/books/abs/arabic-poetics/nazm-wonder-and-the-inimitability-of-the-quran/B980231F9936B751EF11029D69CF4554.
The source is the publisher's HTML summary itself, with the structural locator
`Chapter 5 Summary`, paragraph 2; it is not a scan or a claim that book pages
203–251 were read. The current gate reports `no scan is named`, requires a
scan and a transcription as witness sources, and requires a maker-fetched
raster page image digest for quotation evidence. Please assess an owner route
for a genuine born-digital source that binds exact fetched HTML bytes, source
URL and structural location to both independent readers and preserves checks
of text, context, lawful quotation and currentness. A screenshot alone should
not pretend to be a printed edition or serve as proof of rights. The packet is
not accepted: language searches and an independent assessment of the Mexican
quotation, including the effect of two sentences out of the seven-sentence
summary, remain open. The current gate did not itself emit a `line:` prefix.
