# Langs: translation route needed

The direction needs the PR1 translation route for the existing accepted works:
add a separately sourced witness for a specified language, cut and align each
existing original place to whole translated sentences, retain the edition,
translator dates, source, page, rights and provenance, compare against the
source, and make a separate second-reader gate possible. Support partial
coverage without presenting it as complete. A recorded exhaustive search must
remain distinguishable from an actual witness and from work not yet searched.

The current kit gate requires `entry.witnesses` to contain only the original
and exposes no translation command (`kit.py --help`; `kit.py` around lines
688–707, at ea79117d3). The line's report already lists this route for PR1.
The langs direction is preparing source-checked language files via direct HTTP
while the route lands; genuinely browser-only lookups are queued for Claude.

## Scan corrections in translation witnesses

Update 19 November: PR #432 at `ec411dc549` adds `witness-correct`,
which records before/after, reason and page digest and clears old review.
Thank you; verify the complete route when the PR lands.

PR #432 at `4a045a07` requires `witness-cut` and `witness-cut-candidate`
to find the final translated text verbatim in the fetched transcription. Some
legal transcriptions differ from the printed page we have checked. For example,
Halsted’s Poincaré has “guiding; And” in Gutenberg #39713 but the 1921 scan
reads “guiding. And” on p. 391, so the existing sentence ends after “guiding.”
Please provide an auditable page-image correction route for witnesses (record
the original cut, exact correction, justification and source page, and bind
second review to the corrected text), analogous to the original packet’s
correction route. Do not silently treat a corrected local file as unmodified
remote source. Existing candidate files record such discrepancies explicitly.

## Separately evidence anonymous publication

Several Schiller essays have an 1875 George Bell printing whose title page says
“newly translated from the German” but does not name the translator; the
identity is still under research. Please consider an explicit, reviewed
anonymous-publication route distinct from an identified person with an unknown
death year. EU Directive 2006/116/EC art. 1(3) uses 70 years from lawful public
availability for anonymous works, subject to the provision on an identity
revealed during that term; the Russian anonymous-work rule also requires its
own evidence and jurisdictional review. A missing name on one title page alone
must not establish that the work was lawfully published anonymously, that the
identity remained undisclosed in the relevant term, or that another printing
is the same text. Existing candidates stay unadmitted while those facts are
investigated. Official EU text:
https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32006L0116

## Image-only source transcription

For Burke's Spanish De la Dehesa edition (Alcalá, 1807), the lawful scan is an
image-only PDF and its IA-derived OCR is unusably sparse for many selected
sentences. A maker and separate reader can transcribe individual sentences from
identified page images, but `witness-cut` currently requires a verbatim run in
a separately fetched transcription. Please provide or identify an auditable
kit route for image-first human transcription that binds page image, exact
text, provenance and independent reading; a locally typed text must not be
misrepresented as a downloaded transcription. The PDF's image of p. 86 also
obscures its first lines, while the corresponding Google Books page image shows
them; retain the precise source for each page.

## Spinoza accepted evidence page mismatch

In the accepted `baruch-spinoza.ethics` packet, `evidence.json` records
`printed: ["183"]` for both `III affects2 p184` and `III affects3 p184`, leaf
218. The actual [Paulus scan leaf 218](https://archive.org/download/benedictidespin01spingoog/page/n218.jpg)
is numbered 184 at its top and contains both definitions; the packet places
and labels also say 184. A separate research reader and the langs maker both
visually checked that leaf on 19 November. Please correct the accepted
packet's page evidence through the owner route and preserve/review the
acceptance; langs has not changed the packet or its acceptance.

## Translation place across two printed pages

At PR #432 `c15d0eec96`, `witness-cut` accepts one `--image`, a place stores
one `image_sha256`, and the second-reader gate binds that one image. Whole
translated sentences can cross printed pages: Bergson's `I p39a` crosses
English pp. 37–38 (IA n48–49), and `II p89a` crosses pp. 87–88 (IA n98–99).
Both pages are needed to inspect the complete sentence. Please provide an
owner-reviewed way to bind every page image and each independent second-reader
check to a place, while retaining exact page addresses and digests. A single
image must not imply that the unseen portion was checked. Until supported,
these places remain research candidates; no stitched substitute or silent
truncation is proposed.

## Semantic contrast carried by printed emphasis

In the 1902 Dole English Schiller, *Aesthetical Letters*, p. 56,
[XV p172a](https://archive.org/download/aestheticalphilo05schiuoft/page/n97.jpg)
prints two clauses with the same words but distinguishes them by italics:
first `play`, then `shall only play` and `beauty`. The existing normalized
candidate drops emphasis and therefore presents the clauses as identical.
Please advise the owner-approved representation or witness choice when printed
emphasis is semantically necessary; a plain-text duplicate must not be silently
presented as preserving the contrast. The translation identity and full
alignment also remain under research, so this is not an admission request.

## Alberti accepted Latin spelling to verify

A langs research reader inspected 1541 fol. 136v, n289 of
`leon-battista-alberti.de-re-aedificatoria`, and reports that the print reads
`autoritatem` where the accepted `places.json` has `auctoritatem`. Please
check the [page](https://archive.org/download/dereaedificatori00albe/page/n289.jpg)
and determine through the owner route whether this is an intended documented
normalization or a correction required in the accepted packet. Langs has not
altered that packet or its acceptance.

## Entire language searched without a translated place

At PR #432 `c32662da89`, `witness-missing` records per-place search results,
but `witness_problems` unconditionally adds `no translated places` when the
witness has none (around line 1661). For an already planted work, the langs
objective permits an actual translation **or an exhaustively recorded search**
for each language. Please identify or provide the reviewed route for recording
an entirely searched but unavailable language on an existing work, without
inventing a translator, textual witness or a false locator. Partial witness
coverage is a separate case. No present research lead is claimed exhaustive.

## Mexico quotation assessment in witness route

The shared brief names Mexico LFDA art. 148 I among the required
short-quotation bases. PR #432 `c32662da89` defines `JURISDICTIONS` as
`russia`, `usa`, `eu`; `witness-rights` accepts only those three. Please
identify the owner-approved way to record and review the Mexico quotation
assessment for a witness before public display, or extend the route if needed.
This note does not assume the duration or outcome of Mexican copyright law.

## Anonymous translator and publication-based term

A Bell 1875 English translation of Schiller's *Über die tragische Kunst*
contains a closer reading of III p163 than Weiss 1845; Clark's 1918 anthology
[labels the translation anonymous](https://archive.org/download/europeantheories00claruoft/page/n332.jpg).
The translator's identity, lawful publication and any later disclosure remain
under research. The current witness gate requires a translator and, for
public-domain claims in Russia and the EU, death years. Please identify the
owner-reviewed route for a translation genuinely published anonymously, when
its identity and applicable publication-based terms are actually established.
This request does not claim the legal conditions are already satisfied and does
not propose inventing a person, date of death or licence.

## Add places to an accepted partial translation

On current main, Bergson's accepted French packet has 13 places and no waiting
register readings or links (`kit task` reports zero). The independently reviewed
English witness for 11 places is in PR #518; its two remaining existing original
places, `I p39a` and `II p89a`, cross printed pages and remain unfilled. Once
the witness is accepted, `witness-open` refuses to replace it. The documented
extension route requires waiting register keys in `extension-open`; there are
none for these already planted places. Please provide or identify the reviewed
route for adding non-overlapping translations of existing original places to
an accepted partial witness, preserving the accepted witness and binding new
page evidence and an independent review. This does not request changes to the
original places or assert that the remaining translations have been reviewed.

## Wikidata multilingual labels

`witness-person` selected `Q18910535` (Joseph Ward Swain) with correct dates,
but emitted the QID as the participant name: the entity has its name under
Wikidata's `mul` label, without an `en` or `ru` label. A source-based named
record against the same Wikidata EntityData allowed this draft to proceed.
Please consider recognizing the `mul` label in the person route, without
changing death-date or independent-review checks. The Swain draft was not
accepted for a separate semantic reason.

## Meaningful emphasis in translated quotations

In Alix Strachey's 1925 translation of Freud's *Das Unheimliche*, the
independently inspected [p. 401](https://archive.org/download/freud-1925-cp-5/page/n404.jpg)
prints `real experience` in italics for original place `III p319b`, and
[p. 403](https://archive.org/download/freud-1925-cp-5/page/n406.jpg) prints
`belief` in italics for `III p320`; the latter corresponds to emphasized
`Glaubens` in the German original. The independent reader found the words and
alignment otherwise acceptable but withheld approval because this emphasis is
meaningful. The research stand currently renders fragment text through
`textContent`, so the contrast is not displayed. Please advise an
owner-reviewed representation and display of semantic emphasis, or an explicit
review route for when a particular typographic normalization preserves the
meaning and integrity of a quotation. This is a request to assess the concrete
case, not a legal conclusion that all italic text must be retained. Neither
place has been accepted; the draft is preserved in
`ae-langs-freud-uncanny-emphasis-pending-20261127`.

## Authoritative person sources outside the fixed domain list

The witness person route currently requires a Wikidata QID or a reference on a
fixed set of domains. Some translator identities are documented by strong
sources outside that list:

- [CiNii Books](https://ci.nii.ac.jp/author/DA06887074), the Japanese academic
  library catalogue, names Patrick Edward Charvet and identifies him as the
  translator of Baudelaire's *Selected Writings on Art and Artists* (1981).
  His death date is not established; this is not a public-domain claim.
- A [peer-reviewed study](https://www.tandfonline.com/doi/abs/10.1080/0013838X.2023.2275455)
  (notes 81–82) identifies translator W. B. Rönnfeldt with William Rönnfeldt
  (1837–1919), citing the [Cardiff Naturalists' Society obituary](https://cardiffnaturalists.org.uk/htmfiles/150th-23.htm).
  The dates and identity rest on that chain; the middle name is not established.
  The inspected Goethe printing is undated, so its US publication status and
  any short-quotation route also require their own review.

Please identify or provide an owner-reviewed route to bind such a named person
and exact source evidence, preserving uncertainty and independent review. This
is not a request to treat arbitrary URLs as authoritative or invent Wikidata
records or dates. The source readings and rights of the proposed translations
remain separate work.

## Different translated editions for different original places

The accepted Cooper English witness for Buffon's *Discours sur le style* has
`Discours p5a`; a separate translation of `p5b` in Cooper's 1930 printing was
not accepted after an independent reader found that it changes `plus fixe`
into “more absolute” and makes the writer develop ideas which in the French
arise. A different published scholarly rendering in Joanna Stalnaker,
*The Unfinished Enlightenment* (Cornell University Press),
[p. 66](https://books.google.com/books?id=q_ytDwAAQBAJ&pg=PA66), preserves
both the fixed order and the arising of the ideas. The maker inspected the
publisher-authorized page and the 1829 French p. 5; the translator attribution,
quotation rights, independent reading and formal witness are still to be
completed. The 1930 Cooper witness is accepted only locally, not yet merged.

The documented translated-place extension requires the same edition,
participants, rights and source metadata as the accepted witness and refuses a
different edition. Please provide or identify a reviewed per-place route for
non-overlapping translations from a second edition, preserving each edition's
attribution, distinct rights and page evidence and each independent review.
Do not merge texts from editions or assume one translation's legal status
covers another. This is separate from the existing request to extend a partial
witness with places from the same edition.

## Translation attributed to a named author collective

For Schiller's *Über den Grund des Vergnügens an tragischen Gegenständen*,
`III p150`, [Menninghaus, Wagner, Hanich, Wassiliwizky, Kuehnast and
Jacobsen's 2015 research article](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0128451)
prints a complete English rendering on pp. 1–2 and calls it “our translation”.
The article names six authors and lists writing contributions for four, but
neither identifies the individual translator nor states that all six personally
translated it. An independent reader has compared the original and both
published pages: the sense matches; the printed closing quote precedes the
source citation and final period, so a freestanding final period requires an
explicit normalization. The article's own page links CC BY 4.0 and identifies
the rendering as the authors' translation; any use must credit the source and
record changes. Source-bound PDF-page evidence is separately needed.

Please identify or provide a reviewed way to represent an explicitly collective
attribution, naming the article's six authors as the source collective without
asserting that each was personally a translator or inventing a single person.
Preserve exact edition, licence and page evidence and independent review. The
candidate is not accepted and this request does not assert a particular
individual's contribution or waive the source-page gate.

## Wikidata default-language labels in witness participants

For the accepted English witness for Alberti's *De pictura* prepared on branch
`claude/aesthetics-langs-w2-buffon-schlegel` at `249dfc8182`,
`witness-person ... author Q182046` recorded the Russian label as both the
English and Russian name. The Wikidata record identifies the same person and
has the default-language (`mul`) label “Leon Battista Alberti”; the command
currently chooses `labels.en`, then `labels.ru`, without checking `labels.mul`.
The projected English participant therefore displays in Cyrillic even though
the edition and attribution use the Latin name. The identity and dates are not
in doubt, and the already accepted witness was not altered behind its receipt.

Please consider a source-backed default-language fallback and a reviewed
correction route for an already accepted witness's participant label. Preserve
the exact participant identity and source, prior acceptance, independent
reading and regenerated projection; do not silently rewrite acceptance hashes.
