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
