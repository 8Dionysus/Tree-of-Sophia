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
