# Ancient direction: bounded ancient death dates

Vitruvius (`Q47163`) has a Wikidata death claim circa 15 BCE. `entity_year`
returns `None` for its approximation qualifier, and `public_domain_by_dates`
therefore cannot establish public domain in Russia or the EU even for Rose's
1899 edition (editor Q87564, died 1916). The maker correctly refused to record
an invented exact year; the packet is currently blocked with this evidence.

The line needs a sourced upper bound or another explicit, independently
reviewable way to establish that an ancient author died before the applicable
cutoffs, without converting an approximate date into a fabricated exact year.
The evidence and rights wording must retain the uncertainty, and the gate must
verify the bound as well as every later editor and translator. This is a line
contract change; ancient owns script folds only.

## Attributed source with unidentified translator

The John of Damascus packet located a short Spanish quotation in the Vatican's
2009 audience and checked the PDF page, but that publication does not name the
translator. `witness-gate` requires a named translator, so the maker cannot
record the source faithfully without inventing an identity. The publisher,
place, page and observed absence of a translator are known; the individual
translator and its death date are not. See the packet's `blocked.json` and
its search evidence for exact links. Chinese and other searches are incomplete.

The line needs an explicit, independently reviewed representation of an
unidentified translation hand for a short quotation from a lawful, attributed
source. It must not infer public domain, identify the hand from silence, or
weaken the citation and jurisdiction-specific review requirements.

## Transcription derived from a source PDF

The Kenko packet has an exact Spanish quotation in the publisher Siruela's
PDF preview and a Russian translation in the Institute of Oriental Manuscripts'
PDF. `witness-fetch` records the original PDF bytes, but `witness-cut` reads
those bytes as text and cannot extract the passage. Substituting extracted text
under the PDF URL would misrepresent the fetched bytes. The packet's
`blocked.json` records the publications and locations; searches for other
languages remain incomplete.

The line needs a provenance-preserving way to bind extracted text and the
independently inspected page to the original PDF bytes, or an exact-edition
machine-readable transcription with its own truthful source. Such extraction
must not itself establish fidelity, translation alignment or quotation rights.

Independent image review has the same gap even when the PDF already has a
searchable text layer: in `bhatta-nayaka.hrdayadarpana`, the second reader
found that the previously recorded page image for Sheldon Pollock's
[university-hosted article](https://www.columbia.edu/cu/mesaas/faculty/directory/pollock_pub/pollock_What%20was%20Bhatta%20Nayaka%20Saying.pdf),
p. 163, now returns HTTP 410, and `witness-image` cannot register the PDF as
a page image. Please allow each reader to generate and register the selected
page image from independently fetched, digest-verified PDF bytes, with the
page number, render method, resulting image digest and separate reader
identity preserved. A successful render is evidence of an image, not an
independent reading, textual acceptance or permission to quote.

## Anonymous ancient author

The author of *On the Sublime* is unknown, according to the Cambridge
manuscript catalogue linked in the Longinus packet's `blocked.json`; the
1907 editor's death date is recorded separately. An author entity without an
integer death year cannot pass the current public-domain-by-dates route. This
needs an explicit, sourced and jurisdiction-specific route for an anonymous
ancient work where a reliable historical witness establishes an appropriate
upper bound. It must retain the author's anonymity, account for later textual
contributors and avoid inventing a year or presuming all editions are free.

## Mahima Bhatta identity and unknown death

The Vyaktiviveka maker located a possible author entity Q15140044, but the
layer's `mahima-bhatta` identity remains unresolved; its English label differs
from the layer label, and the kit cannot supply alternative names for an item
without supplying a fabricated exact death year. Open Library's author record
https://openlibrary.org/authors/OL13268938A identifies Mahimabhatta as active
in the eleventh century and lists *Vyaktiviveka*; it is evidence for review,
not by itself proof that this is Wikidata Q15140044. The packet's
`blocked.json` has the searches and distinctions. Please resolve the identity
through the line's source route and preserve uncertain dates. The recent 1964
edition has an editor whose independent contribution and rights also need
separate assessment; ancient authorship alone does not free that edition.

## Legacy Arabic encoding and PDF licence evidence

For al-Farabi's *Kitab al-shiʿr*, the publisher-served Özpilavcı (2026)
critical edition is a PDF whose first page visibly states CC BY-NC 4.0; the
publisher's article API also returns that licence URL. The kit's licence
check searches raw PDF bytes for the URL and does not establish the visible
statement. The edition is not public domain; attribution, noncommercial scope,
third-party material and any jurisdiction-specific quotation basis remain
subject to independent review. The exact source, PDF digest, first page and
printed p. 48 are recorded in the packet's `blocked.json`.

A separate Arabic quotation at https://www.ahewar.org/debat/show.art.asp?aid=266761
is served as Windows-1256, while the transcription path assumes UTF-8 and
inserts replacement characters. It is a separate edition whose rights and
alignment still require review. The line needs source-byte-preserving handling
of declared encodings and verifiable page or publisher-API licence evidence;
converting bytes and claiming they were fetched unchanged is not a solution.

## A Latin sentence beginning in lowercase

In Mayhoff's 1897 Pliny XXXV.145, a period on the printed page precedes the
lowercase opening `illud`; the maker and a second reader observed the page
https://archive.org/details/cplinisecundina19plingoog/page/n299/mode/1up .
The kit and the shared whole-sentence helper reject it solely on initial case.
Another edition has an uppercase opening and is being evaluated separately.

Please support such a source-verified lowercase Latin sentence in a way that
uses the actual language and preserves the normal capital-initial check for
modern translations; the source text and independent page reading still need to
establish the boundary. The call from `plant.mjs` is owned by the line. A
blanket exemption for every Latin-script translation would weaken the check.

## A printed footnote after a full stop

In Kirchmann's German *On the Soul* (1871), printed p. 149,
https://archive.org/details/aristotelesdrei00arisgoog/page/n161/mode/1up ,
a full stop is followed by the superscript footnote marker `220)` and then a
new sentence. The maker's two OCR witnesses preserve or corrupt that marker;
`witness-cut` rejects the next sentence as opening inside the previous one.
The operator also inspected the actual page. The source-page review is still
needed for the whole translation and its alignment.

Please let the transcription boundary check recognize a clearly identified
trailing footnote marker after sentence punctuation without treating arbitrary
words as footnotes or weakening the whole-sentence rule. Preserve the original
source bytes and the independent page-reading requirement. Other language
searches are incomplete; this is not a missing-translation finding.

## Repairing provisional witness participants

In the Pliny packet, the English witness was opened with three preliminary
string-valued participant names without evidence; `witness-person` then added
the same author and translators with QIDs and fetched evidence, leaving the
original unsupported entries in place. The gate correctly rejects those
entries, while `witness-info` protects persons and the witness commands offer
no removal or replacement. See the packet's `blocked.json` and `witnesses/en`
for the exact records. The prepared English text, page images and proper person
evidence should be retained.

Please provide a supported, audited way to remove or replace an erroneous
provisional witness participant, invalidating the second review as needed.
Do not silently equate names or suppress unsupported participants; the maker's
error and every identity still require review.

## Preserve existing reader logs when resuming a line

On 27 November the `aristotle.de-anima` output directory already contained
`01-maker.jsonl` and `01-maker.md`, while its `state.json` had no recorded
sessions. `drive.py` chooses the next filename from `len(sessions) + 1` and
opens the JSONL with mode `w`. A retake consequently overwrote the earlier
`01-maker.jsonl`; the old final message and the directed continuation's
JSONL and message remain. The current session is writing to the same name,
so the previous event log cannot be inferred from it.

Please make allocation of reader log and final-message names collision-safe
against files already present, even when state is missing, stale, or an
interrupted reader has not yet been recorded. Preserve existing evidence and
concurrent readers; report a failure instead of overwriting. A new output
directory is the interim workaround, not a recovery of the lost log.

## Pliny witness: repair unsupported legacy participants

On fresh main `d5471497`, the Pliny retake confirmed that
`witness-person-remove` requires a QID. The three original English witness
participant records are strings without QIDs, while independently sourced
structured participants were subsequently added alongside them. The supported
command cannot select the unsupported records, so the witness gate still
fails. Please provide an auditable correction for these exact pre-existing
records without deleting the evidenced participants or changing the source
witness. The packet also reports a separate imprecise-death-date mismatch and
an English two-page image binding to resolve; Spanish and other translations
remain unfinished.

## A translation-source block and original-language OCR

The `aristotle.de-anima` retake verified that the footnote boundary is fixed:
the English and German witness maker gates now pass. Russian, Spanish and
Chinese witness searches remain incomplete. A `sources` block explaining
that those translation pages have not been established was rejected because
the packet has page-indexed OCR of its Greek original. The check in
`gate` applies to any OCR scan without considering which language the
blocked source must supply. Original Greek OCR cannot provide a translated
witness. The maker recorded a `kit` block with the exact reproduction; the
blocked state does not claim that any translation is absent.

Please distinguish a missing original transcription, where the OCR fallback
is appropriate, from an identified translation-source obstacle. Preserve the
requirement for an actual documented search and do not turn incomplete search
into a finding of absence.

## Sourced ancient author bounds in translation witnesses

For Pseudo-Longinus, the fresh retake confirmed that `person --died-before`
can represent an anonymous original author with a sourced bound. Translation
witnesses also record the original author, but `witness-person-source` only
accepts an exact `--died` year, and `witness-person` cannot recover a usable
bound from the anonymous Wikidata identity. Please support the same sourced,
uncertainty-preserving bound and anonymity in the translation-witness route,
with independently reviewable evidence and all translator/editor rights kept
separate. The available alternative of quotation or licence still needs
assessment for the actual passages; this request asserts neither public
domain for an edition nor absence of other sources.

## Original text transcribed directly from a printed page

For the butterfly dream in the Zhuangzi packet, a dated 1445 printing of
Lin Xiyi's *Nanhua zhenjing kouyi* is available as page 57 of the copy
recorded at https://commons.wikimedia.org/wiki/Special:Redirect/page/201074579.
The print reads 翔翔 where a separately sourced Wikisource transcription
reads 栩栩, and the scanned edition also varies 胡蝶/蝴蝶. The maker has
visually located the passage, but has not completed a character-by-character
reading or established all contributors' rights. The separately acquired
transcription cannot be claimed as this printing without an explicit collation.

The current original `cut` route requires a fetched machine-readable source or
page-indexed OCR, and `witness-transcribe-page` records a translation witness
with translator requirements. Please provide an auditable original-language
page transcription route, recording the exact page image, the transcriber's
identity, any editorial punctuation, and an independent second reader;
retain the source and edition distinction. Alternatively, a documented way to
collate an existing transcription against a different printing must preserve
both sources and each correction. A transcription or scan alone establishes
neither a verified reading nor the contributors' rights.

## Independent image of a lawful web edition

The Sophocles *Antigone* packet has identified an authorized electronic
edition at https://yunqi.qq.com/read/25912987/5; its
https://yunqi.qq.com/detail/25912987 catalogue names the translator, publisher
and the digital rights provider's authorization. Both short prologue passages
are visible in the HTML chapter. The edition does not show print pagination,
but chapter 5 is an exact structural locator; the maker fetched the HTML through
the kit and has not claimed to inspect a printed page.

`witness-cut` still requires a registered page image. `witness-image` can
record an image URL and optional supplied image bytes, but does not represent
a browser screenshot as a derivative of an independently visited HTML URL:
using the HTML URL as if it delivered those image bytes would obscure their
origin. Please provide a verifiable route for recording a browser-rendered
page screenshot or other independently inspectable visual evidence of an
unpaginated lawful electronic edition, with source URL, capture time and
method, image digest and separate maker/second-reader evidence. This should
not turn a screenshot into proof of authority, text accuracy, equivalence,
or permission to quote; the reader must still inspect the page and assess
rights in each jurisdiction.

## Exclusive death-year bound from a Julian Wikidata claim

For the English witness of Kenkō's *Tsurezuregusa*, Wikidata Q372772 gives
P570 in Julian 1350 with year precision. `death_bound` correctly widens the
latest possible Gregorian calendar year to 1351, but `witness-person
--bounded` stores that inclusive latest year as `died_before: 1351`. The
second-reader instructions require evidence that death happened **before**
the recorded year; a death in January 1351 is not before 1351. The online and
offline gates nevertheless report open after the review. The same field is
used by original-person handling, and Vitruvius has also been represented
through a computed bound; please audit both routes and existing readers.

Please make the semantics explicit and consistent: either record the strict
exclusive bound 1352, or use an accurately named inclusive-latest-year
representation everywhere. Preserve the Julian calendar, precision and
original claim evidence; update review and acceptance invalidation as needed.
The distinction does not change the public-domain conclusion for this ancient
hand, but a gate must not accept a statement stronger than its evidence. This
request is separate from the modern translator's own quotation-rights review.
