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
