# The maker's route

You make one packet: one work of the Aesthetics bonsai. Your working directory
is the packet. The card `AGENTS.md` two levels up holds the laws; this page is
the order of work. `K` below stands for `python3 ../../kit.py`.

The packet may already hold work from an earlier session. Read what is there
(`places.json`, `reading.json`, `evidence.json`, `K gate .`) and go on from it.
What a second reader confirmed stays as it is.

## 1. The task

`K task .` prints the work, the register readings and links that wait, and
for every concept its key and its slug. A register reading is what a research
register wrote about a place of the work: locus, thesis, concepts. It tells
you where to look and what to test. It is no ground: the place is.

A `sources` block defaults to the original work. If the remaining obstacle is
a translation source, record `source_scope: "translation"`, the interface
`language`, and `tried` entries with an HTTPS `url` and specific `outcome`.
An incomplete search is a block, never an absent-language finding; original
OCR does not supply the translation.

## 2. The edition

Find, by fetching pages (law 7):

- the original-language text in a lawful scan or preview, with page-indexed OCR where there is one;
- a transcription of the same text from a proofreading project (Wikisource
  with its scan, Project Gutenberg, Deutsches Textarchiv, Zeno.org, Gallica).
  For English before 1800 the Text Creation Partnership (ECCO-TCP, EEBO-TCP)
  keeps keyed texts; for German, TextGrid; for Latin, the Corpus Thomisticum
  and the Latin Library. A copy on another site serves only where none of
  them gives the text, and `K why . FILE "what was looked at, why none gives
  it"` says so. A page of extracts does not excuse a reading: where the
  transcription at hand lacks a passage, find it in the scan
  (`K find . "words"`) and fetch a transcription that holds it.

Choose the two as a pair: take the scan the transcription follows. A
Wikisource index names its scan, often an Internet Archive item; the Deutsches
Textarchiv and Gallica show their own. A transcription of another edition
brings that edition's readings with it: each one the page does not bear is a
correction (step 4), and `reasoning` says which edition the transcription
follows.

```
K ia . ITEM            # add --long-s for a print that sets the long s
K fetch . transcription URL
K person . author Q37160        # the Wikidata item of each hand in the text: the kit reads name and year of death
K printed . 1757                # the year of the printing whose scan is read
K person . editor Q123 --remove # a hand entered for an edition you did not take leaves the record again
```

A work in several volumes has a scan for each volume read: fetch each with
`K ia . ITEM`. The kit keeps them all, finds every place in the scan that holds
it, and `K scan . ITEM` chooses the one that `K leaf`, `K image`, `K offset`
and `K cut . ocr` work on (`K scan .` lists them). Each scan wants its own
offset.

Prepare a complete copy of the packet's current `places.json` outside the
packet, add `entry`: `original` (language code) and one witness under that
code, then use `K import . places FILE` to record it. Packet files are changed
only through kit commands; retain existing cuts and page checks in the copy:

```json
"witnesses": {"en": {
  "edition": "Title, place, printer, year, which edition; whose copy the scan is.",
  "year": 1753,
  "url": "https://archive.org/details/ITEM",
  "sources": ["https://archive.org/details/ITEM", "https://www.gutenberg.org/ebooks/NNNN"],
  "licence": "Public domain book; scan in the Internet Archive. Project Gutenberg License for the transcription the words were taken from (public domain in the USA).",
  "attribution": "Author, Title (place: printer, year); words from …, compared with the scan …",
  "rights": {"reasoning": "Whose text; which transcription the words come from and which edition it follows; which scan they were compared with; what was normalised."}}},
"absent": []
```

`K rights .` writes the public-domain determinations supported by the dates.
For a quotation, licence or locator, `K rights . --jurisdiction russia --basis
short-quotation --law "…"` records that jurisdiction's basis and law. Repeat
for all three. A language not searched is simply unsearched; `absent` records
an actual search and its outcomes, not an omitted translation. If the work
is already in the bonsai (`K task .` says so), `places.json` has no `entry`:
read the same edition and add places beside the ones held.

## 3. The places

For each register reading, find in the transcription the sentences the reading
rests on (`K find . "some words" --in FILE` shows the surroundings), read
around them, and take the whole sentences that say the thing:

```
K cut . FILE "Opening words of the first sentence" "closing words of the last." --loc "II p16a"
```

Where no transcription holds the passage, the words come from the scan's own
OCR. This road is always open where the scan has page-indexed OCR: a missing
transcription is no ground for `blocked.json`. First say what was looked at,
on the projects' own sites and not only through a web search (a site that did
not answer is named as such):

```
K why . ITEM.ocr.txt "Wikisource, the Deutsches Textarchiv, Zeno, TextGrid and Project Gutenberg searched on their own sites for the title and for «some words of the passage»: none holds this text."
K find . "words of the passage"          # the leaf of the scan; K leaf . N prints its OCR
K cut . ocr "Opening words as the OCR has them" "closing words." --loc "II p46"
```

The kit joins words divided at line ends and writes ſ as s. Such a place has
one machine reading behind it, so the page is read in place of the second:
step 4 reads it whole on the image of every page it runs over, mends each slip
of the OCR with `K correct`, and the second reader reads it on the page again.
Take this road only for a passage no transcription holds.

A place opens with a whole sentence. A Latin sentence may open in lower case,
as the editions print it. A note mark that ends the run before the opening
(`Satz. 220)`, `2)`, `[3]`, a superscript) is the boundary of the place; the
same mark inside a sentence still breaks it.

`--drop 'REGEX'` leaves out what the transcriber added inside the run (a
figure mark, a page mark). A place is understood alone: where its first
sentence has a pronoun or a «this» whose referent stands in the sentence
before, the place opens with that sentence. A connective alone (Denn, Also,
Thus, But) does not call for it. Name a place by its division and the printed page it opens
on, with a letter where a page holds several: `II p16a`, `§28 p112`,
`Letter XV p164`. Then give it by hand, in `places.json` (`p.` for one page,
`pp.` for a run of pages):

```json
"label": {"ru": "Гл. II, с. 16", "en": "Ch. II, p. 16"},
"refs": [["en", "First edition (London 1753), p. 16"]]
```

The printed page number is read from the page: `K compare .` prints it where
the Archive gives it; where it does not, the image shows it.

## 4. The page

```
K compare .
```

It lists, place by place, the leaves of the scan and every difference between
the transcription and the OCR. First fix the offset between OCR leaves and
page images once: `K image . N`, open the file with your image viewer, find
the leaf whose OCR (`K leaf . L`) is that page, then `K offset . N-minus-L`
and `K compare .` again (it now writes each place's `links`).

Then, for every leaf that carries a difference: `K image . N` (N = leaf +
offset), look at the image, and judge:

```
K judge . "II p16c" all transcription          # the page reads as the transcription at every difference
K judge . "III p19" 2 print "page reads 'ac-count' broken over the line"   # one difference where the page reads otherwise
K correct . "III.14 p193" "a qualify so" "a quality so" "scan p. 193 (Internet Archive ITEM, leaf n220)"
```

A correction that changes letters while the OCR of the page gives it no
support — the place was read from the image, the OCR reads otherwise there, or
the print sets the long s and the change is between s and f — rests on your eye
alone, and the whole page is too small for one letter. The gate names each such
correction. Read its line at the size of the scan:

```
K image . 170 --loc "III def3 p132" --zoom 41-48    # the band of the page height, in percent, that holds the line
```

Keep the correction where the enlarged line bears it letter by letter. Where
the line reads as the transcription, put the transcription's word back with
`K cut` and take the correction out.

After a correction, compare again. In a print with the long s, ſs is the
print's ß and the transcription's ß stays; ſſ is ss. Where the OCR does not hold a place at all
(`compare` says so: a Fraktur print, a poor scan), fetch every page the place
runs over with `K image . N --loc "LOC"`, read the place word by word on the
images, and record `K judge . "LOC" whole transcription "what you read where"`.

List under `checked_on_the_page_image` in `places.json` what you looked at, as
`"p. 18 (head of the page)"`.

## 5. The reading

One conception is one thing the work says at its places:

```json
{"id": "william-hogarth.the-analysis-of-beauty:composed-variety",
 "of": ["A:C120", "A:C001", "A:C030"],
 "areas": ["AE01", "AE03"],
 "work": "william-hogarth.the-analysis-of-beauty",
 "locus": "Ch. II",
 "formula": [{"place": "II p16a"}, {"place": "II p16b"}],
 "thesis": {"ru": "Разнообразие имеет великую долю в создании красоты: все чувства находят в нём удовольствие и равно отвращаются от однообразия.",
            "en": "Variety has a great share in producing beauty: all the senses delight in it and are equally averse to sameness."},
 "status": "text", "verdict": "confirmed",
 "from": ["A:E011"]}
```

- `of` and `areas` are keys of the task. `from` is the register readings this
  conception grows from; two readings of one place make one conception.
- A conception says one thing. Two things the work says in two places are two
  conceptions; a thesis that sets them side by side is two theses.
- `verdict`: `confirmed` — the register's thesis stands on these sentences and
  all its concepts are under `of` here or in a sibling conception grown from
  the same reading; `revise` — the sentences say it otherwise than the
  register (a concept dropped, the thesis narrower or other): `note` says
  what changed; `reconstruction` (with `status: "reconstruction"`) — the one
  thing that neither sentence says alone and the two say together: `note`
  names them.
- The thesis is one to three sentences in the work's own terms, Russian and
  English saying the same. It affirms (law 5). Written as «Красота — это не
  просто симметрия, а …» it fails; written as «Симметрия нравится постольку,
  поскольку даёт идею пригодности» it stands. Where the place itself negates,
  the thesis may; if the gate still objects, add `"negation": "<the negating
  words of the place, as they stand in it>"`.
- `distinction` (optional, ru/en) keeps a register's distinction that the
  place bears.

A link joins two concepts by a verb the grounds say:

```json
{"id": "symmetry:pleases-as-it-gives-the-idea-of:fitness",
 "from_concept": "A:C104", "to_concept": "A:C122",
 "verb": {"ru": "нравится лишь постольку, поскольку даёт идею того, что есть", "en": "pleases only as it serves to give the idea of"},
 "grounds": ["william-hogarth.the-analysis-of-beauty:symmetry-serves-fitness"],
 "status": "text", "verdict": "revise",
 "note": "Register A: «оценивается с учётом». The verb follows the last sentence of chapter III.",
 "from": ["A:R011"]}
```

The id is `<slug of the from concept>:<the English verb as a slug>:<slug of
the to concept>`. A link is made only from a register link of the task, and
only where a conception of this packet grounds it.

Every waiting reading and link ends in a `from` or under `left`:

```json
"left": [{"key": "A:E214", "reason": "The register reads this from a handbook; chapter IV, which it names, speaks of simplicity and does not say that …"}]
```

Fill `reader.reads` with one sentence saying what was read (the language, the
edition, beside the registers' readings). Prepare the complete reading object
outside the packet and record it with `K import . reading FILE`.

## 6. The gate

```
K gate .
```

Settle every line it prints and run it again, until it prints `gate: open`.
Close with five lines: the state, the counts (places, conceptions, links,
left), what was left and why, what you corrected against the page, what the
second reader should look at first.

## A separate translation

An accepted work can take a partial translation as a separate file. Supply a
JSON metadata object with `edition`, `year`, `url`, `licence`, `attribution`,
`maker`, `persons` (all authors and translators with death years and source
URLs), and `rights` with `{basis, law}` for `russia`, `usa`, and `eu`.
Russian metadata also gives `normalization`. The commands write the packet:

```sh
K witness-open . ru /path/to/metadata.json
K witness-person . ru author Q123
K witness-person . ru translator Q456
K witness-fetch . ru https://example.org/transcription --name text.txt
K witness-image . ru https://example.org/page --name page.jpg
K witness-cut . ru text.txt "opening" "closing." --loc "§1 p1" --page "p. 3" --image page.jpg --alignment "How these sentences correspond to the original."
K witness-image . ru https://example.org/page --name second.jpg --by second
K witness-review . ru "§1 p1" --reader another-reader --alignment "How the second reader verified the correspondence."
K witness-gate . ru --second --accept
```

When any jurisdiction uses `short-quotation`, separately assess Mexico's
LFDA article 148(I) for this exact witness and its selected places. Record
why the quotation is not a substantial substitute, how normal exploitation
is affected, and how attribution, source and unchanged wording are preserved:

```sh
K mexico-quotation . --language ru --non-substantial '…' --normal-exploitation '…' --source-integrity '…'
K mexico-quotation-review . --language ru --outcome approved --assessment 'Independent assessment of these exact places and the law.'
K witness-gate . ru --second --accept
```

The independent reader chooses `approved`, `rejected` or `pending` according
to the evidence. Only `approved` opens the second gate; a rejected or pending
assessment is retained as such and can be reviewed again after correction.
An older acceptance without an explicit outcome owes a new assessment.

Mexico may require a quotation assessment even when the recorded Russian, US
and EU bases are all public domain. Record the Mexican assessment directly; it
does not change those three bases and it requires the same independent review.


The first command fetches the official Cámara de Diputados law PDF, retains
its digest and binds the assessment to the exact witness and places. The second
reader must already be recorded through `witness-review`. For an original
witness omit `--language`; its existing packet second reader reviews the
assessment. An extension records its own assessment on its evidence and review,
scoped to that extension's places. A later change to the witness or its places requires a new
assessment and independent review. Missing assessments on older accepted
packets remain explicit `owes`, while new quotations require this assessment.

The reader opens each fetched image and checks the whole translated sentences
against it and against the original; recording a review is not itself a page
reading. A quotation also records `--lines`, and an exceptional longer place
records `--justification` on `witness-cut`. The acceptance binds the exact file
by SHA-256. Changes require a new independent reading and acceptance. Local
source files and images stay in the kit cache outside Git. A partial witness
leaves the other places without a translation.

For a place that runs across printed pages, fetch each additional page as
maker and bind its words to the place. Open every image and read the
complete passage; the second reader independently fetches and opens the same
images before recording one review of the whole place:

```
K witness-image . en https://example.org/page38 --name page38.jpg
K witness-add-page . en 'I p39a' --page 'p. 38' --image page38.jpg --first-span 'words on the first page' --span 'words on the second page'
K witness-image . en https://example.org/page38 --name second38.jpg --by second
K witness-review . en 'I p39a' --reader another-reader --alignment 'Both pages read against the original place.'
```

When one source image shows a spread, use the same image for both distinct
printed page numbers. For a correction on a shared image, add `--page` to
select the printed page whose span contains it.

The spans cover the whole place in order; each records the words read on that
page. The review binds the URL, printed page and digest of every image. A page
range needs an image for every page. For a correction on an additional page, pass
`--image page38.jpg` to `witness-correct`; the selected page span is updated.
The citation and packet provenance preserve the additional page references.

When no usable transcription contains the passage, open and read the maker's
page image, then record the actual words as a human page transcription:

```
K witness-transcribe-page . es --loc '§1 p1' --page 'p. 86' --image page.jpg --text 'The actual complete sentences read on this page.' --reason 'The image-only source has no usable transcription of this passage.' --alignment 'How these sentences correspond to the original.'
```

Use the exact image URL for each page; another page source requires its own
image. The packet identifies this as a human transcription, not a downloaded
text. A different reader must open the same image and independently review the
words and alignment with `witness-image --by second` and `witness-review`.
Recording a check is a statement that the reader actually opened and read it.

A prepared JSON object can be brought into a new, unaccepted packet with
`K import . places FILE` or `K import . reading FILE`; the second reader can
use `K import . review FILE`. `K import . blocked FILE` records a block and
`K unblock .` clears it for another attempt. Import does not validate or
accept the packet. The gate still checks the source files, page evidence and
reading. Accepted packets cannot be replaced by these commands.

For an accepted packet whose Archive page-number metadata disagrees with the
visible printed page, the line owner and a different reader open that exact
leaf. The owner can record the two observations without replacing acceptance:

```
K page-number-correct . --loc 'III affects2 p184' --loc 'III affects3 p184' --leaf 218 --old 183 --new 184 --url 'https://archive.org/download/benedictidespin01spingoog/page/n218.jpg' --by maker-id --second second-id --second-model model --assessment 'What the second reader actually saw on this image.'
```

The source page image is kept in the local cache; its URL and digest, the
previous number, both readers and the assessment are recorded in evidence.
The original acceptance stays in place and binds the correction review. A new
comparison retains the correction only while its original leaf and recorded
predecessor still match.

For an accepted original text whose print differs from its transcription,
two independent readers may record the exact page-bound correction without
replacing the original acceptance:

```
K accepted-text-correct . 'LOC' 'BEFORE' 'AFTER' --n 289 --url 'https://archive.org/download/ITEM/page/n289.jpg' --by maker-id --second reader-id --second-model model --assessment 'What the second reader saw on this page.'
K compare .
K gate . --second
```

The page must already be recorded for this place; the source image and digest,
both readings and the separate assessment remain bound to the acceptance.

## Continue an accepted reading

If every required source has actually been searched and no translation of any
current place was found, record the search on the parent packet, without inventing a
translation witness or book locator. The input JSON has `maker`, a detailed
`reason`, and `tried` entries with `site`, `url`, and `outcome` for each
required source. The command records the current original-place scope.

```sh
K language-search . es search.json
K language-search-review . es --reader SECOND --model MODEL --assessment 'What the second reader checked and concluded.'
K language-search-accept . es
node ../../../plant.mjs
```

The second reader must differ from the maker; acceptance binds their decision
to the exact search. Adding an original place requires renewed search and
review. `K language-search-withdraw . es` removes the acceptance while keeping
the evidence, so the search can be replaced. A subsequently accepted witness
takes precedence over the search in the projection.

For a legacy packet, `drive.py` sends a maker-ready unaccepted witness or
complete language search to a fresh independent reader before the packet's
second reading. It accepts that language only after its own second gate opens;
a research candidate or failed review remains a debt. The line preserves a
snapshot of the maker evidence and stops on an unexpected change to it.

Leave the accepted packet and its review intact. To work on specific waiting
register keys, make an independent extension:

```sh
K extension-open . ae25 --keys D:KEY1,D:KEY2
python3 ../../drive.py run WORK/extensions/ae25
```

The actual path printed by `extension-open` is
`packets/WORK/extensions/ae25`; pass `WORK/extensions/ae25` to `drive.py run`.
Inside the extension, K is `python3 ../../../../kit.py`. The extension records
its own maker, source evidence and second reader; its gate checks the named
scope without reopening unrelated disputed keys. Accepted extensions join the
parent's places and readings at planting.

A direction's prepared research JSON can be kept without admitting it with
`K witness-candidate . ru FILE`. After fetching the actual transcription and
page image, `K witness-cut-candidate . ru text.txt --loc '…' --page '…'
--image page.jpg --alignment '…'` cuts the candidate's complete text out of
that transcription. The original research note remains a candidate; the cut,
page evidence, rights and independent review still go through the witness
gate before acceptance.

For one maker page without a usable transcription, `K witness-ocr . LANG
PAGEFILE --model-dir DIR --model rus` runs locally installed Tesseract and
records the raw OCR, exact page and model hashes, engine versions and default
parameters. The URL remains the image's address; the text is explicitly
derived, not downloaded. For a passage across pages, OCR each separately and
prepare a JSON object `{"segments":[{"source":"FIRST.txt","open":"…",
"close":"…"},{"source":"SECOND.txt","open":"…","close":"…"}],
"reason":"…"}`. The reason explains the join and any omitted catchword;
`K witness-ocr-join . LANG FILE` records exact raw character ranges, not a
claimed downloaded transcription. Cut from the resulting file, add each page
with `witness-add-page`, and correct every error against its own page. The
second reader checks every page, the join and omissions separately.

Use `witness-info` with a JSON object to update edition, URL, maker or
normalization. `witness-rights . ru russia short-quotation 'legal basis …'`
sets one jurisdiction at a time. For a person without a usable Wikidata item,
`witness-person-source . ru translator 'Name' https://… --born YEAR --died YEAR`
fetches a trusted reference and records its digest. A missing death year stays
unknown and cannot establish public domain by dates.

CiNii Books and Cardiff Naturalists' Society are accepted reference sources.
For each newly recorded person from either, the second reader must explicitly
assess the match between the reference, the named person and this edition via
`witness-reference-review`. An obituary alone does not establish that its
subject is the translator named in an edition, and a catalogue without dates
does not establish a death year or public-domain status.

When a source explicitly calls a rendering its authors' collective translation
without naming individual translators, record the exact article with
`witness-fetch`, its place and page as usual, then `witness-collective . LANG
LOC FILE`. FILE is a JSON object with `members` (all named source authors),
`source` (the fetched filename), `claim` (the source's exact attribution) and
`assessment` (how it attributes this passage to the collective). The witness
attribution must name the whole collective without asserting each person's
individual role. Record each place separately. Unknown contributors do not
establish public domain by dates: assess the actual licence or quotation basis
in every territory, and obtain independent page, licence and collective review.
Use `witness-collective-clear` with a reason to withdraw an incorrect claim.

If a place was tied to an external transcription although its maker page is
from a registered Archive scan with indexed OCR, use `scan-rebind . LOC ITEM
--image FILE --reason '…'`. The image must identify that Archive page and the
OCR and scan offset must locate the place there. The former binding and review
are retained, not silently rewritten. Run `compare` again and obtain a fresh
independent reading; locating the place is not itself a textual comparison.

When a Wikidata person has no death claim but its linked historical source
establishes an ancient upper bound, keep both identities and source bytes:

```sh
K person . author Q123 --name 'Name in the linked source' --source https://en.wikipedia.org/wiki/Example --died-before 1223 --bound-note 'What the source establishes and how the conservative bound was derived.'
```

The source must be a trusted sitelink of that exact entity; its name, fetched
bytes and entity bytes are retained. This route never invents a death year. A
separate reader reviews the reference and the bound before packet acceptance.

An ancient or medieval hand whose death is known within bounds is entered with
`K person . author QID --bounded` (and `witness-person . LANG translator QID
--bounded` for a witness). The kit reads the Wikidata date of death with its
precision (year, decade, century, millennium) and its latest date; a date of
death marked `circa` counts as alive about then. Without a date of death, a floruit or the end of a working period shows
the person alive then, and the bound is that year plus the longest attested
human life (122 years), allowing an extra calendar year for the partial year of
that lifespan. The kit records a strictly exclusive `died_before` with the
claims and calendar it rests on: a latest possible Gregorian death in 1351 is
recorded as before 1352. A bound before 1800 establishes the
public domain by dates in every jurisdiction. For a hand without a usable
Wikidata item, `K person . author --died-before YEAR --name 'Name' --source
https://… --bound-note 'What the reference says that bounds the death.'`
fetches the trusted reference, checks that the name stands in it and records
its digest; `--anonymous` marks the unnamed hand of a text (a scholiast, a
compiler) whose dating the reference gives. Every metadata change
requires a fresh independent reading before acceptance.
For a legacy unsupported participant stored as a string, remove only the exact
string with `witness-person-remove-legacy . en 'Name' --reason '…'`. The
command preserves the removed value and reason, leaves sourced participants in
place and requires a fresh second reading; duplicate identical strings must be
resolved explicitly rather than guessed.

For an ancient original author recorded in a translation witness, a trusted
reference may instead bound the death without inventing a year:

```sh
K witness-person-source . en author 'Name in reference' https://example.org/reference --died-before 1200 --bound-note 'What the source says that establishes this historical bound.' --anonymous
```

For an author already recorded with a Wikidata QID, first use `witness-person`,
then attach a reference that is a sitelink of that exact entity:

```sh
K witness-person-bound . en author Q123 --name 'Name in reference' --source https://en.wikipedia.org/wiki/Example --died-before 1200 --bound-note 'What the source says that establishes this historical bound.'
```

Omit `--anonymous` for an identified author. The named translator and editors
still need their own dates and rights. For an anonymous original, the bound
alone does not establish the Russian or EU public-domain basis: first lawful
publication and posthumous rights need separate evidence. Use a separately
assessed quotation or licence until that evidence has its own reviewed route.
The second reader verifies the reference and the bound.

For an unaccepted witness, `witness-person-remove . ru author Q123 --reason
'Reason for removing this mistaken identity.'` preserves the removed record
and clears the second reading; verify the replacement before reviewing again.

If a second reader finds an error after acceptance but before the work was
planted, `K reopen . --reader ID --finding 'The specific error seen on the
page …'` withdraws the acceptance, retains the complete predecessor and
clears the second reading. Correct the reading through the kit and obtain a
new independent review. This route refuses already planted works and packets
with accepted translation witnesses or correction receipts.

After `reopen`, read every numbered finding in `reopen-history.json` and
address it in the actual reading or explain with source evidence why it does
not hold. The packet maker records each response and its exact reading scope:

```sh
K reopen-response . 1 --maker MAKER_ID --assessment 'Specific response to the finding and the source words that support it.'
```

Changing the reading or places invalidates this response. A different second
reader must assess the finding after their complete packet review.

For a work already in `fragments.json` that predates the line and has no
packet, run `K witness-parent WORK` first. It creates a witness-only parent;
it does not manufacture a reading or an acceptance. Open and review the
language in that parent with the same witness commands above.

When the scanned page differs from the transcription, first cut the unmodified
source run, fetch and read the maker's page image, then use
`K witness-correct . en 'LOC' 'FROM' 'TO' 'What the page reads and where.'`.
The witness retains the original and corrected readings, the reason and the
page-image digest. The correction clears the prior review; the second reader
must read the corrected place on the same page before acceptance. Give context
around a deletion so both sides of the correction are nonempty and unique.

A source with no printed pagination can identify its exact place structurally:
`K address . 'LOC' section 'III.2' https://example.org/work#section-2` records
the kind, value and source URL. A translation cut can instead use
`--address-kind section --address-value 'III.2'` in place of `--page`.
Keep the whole sentences, source bytes, page or screen image and independent
reading even where the edition supplies a section rather than a page number.

## Interface languages of a new work

For each of ru, en, es, de and zh-Hans other than the original, search the
lawful source routes named by the operator. Add each available translation as
an independently sourced language witness before finishing the maker gate.
A missing-language entry is reserved for an actual search: it records a reason,
the book's edition and lawful locator, and `tried` entries with the site, URL
and outcome of each required search. An unsearched language is not absent.
The maker prepares source and image evidence; the separate second reader reviews
each witness. The packet acceptance binds those reviewed language files.
Existing accepted packets and continuations retain gaps as `owes`.


### PDF with an extractable text layer

Fetch the PDF bytes as `other` (or `witness-fetch` for a translation), then
extract the text and render each physical PDF page used by the place:

```sh
K fetch . other https://example.org/book.pdf --name book.pdf
K pdf-text . book.pdf
K pdf-page . book.pdf 12 --loc 'I p10'       # physical PDF page, starting at 1
K pdf-page . book.pdf 12 --loc 'I p10' --by second
```

For a translation, use `witness-fetch` and add `--language en` to both
commands; `pdf-page` then records an image usable by `witness-cut` and
`witness-review`. The commands retain the PDF digest, extracted text or image
digest, tool version and operation. The printed page in the edition may differ
from the physical PDF page. The second-reader command independently fetches the PDF and refuses a digest
that differs from the maker's source; older accepted records without this fetch
remain visible as a debt. Both readers must open and read the rendered image;
rendering or matching hashes does not constitute a page reading. Licences and
rights of the precise edition still require independent assessment.

### Licensed editions still under copyright

Use `licensed` for a protected edition covered by an open licence; it does not
assert public domain. Fetch the exact publisher or repository edition as a
source (`fetch` for the original, `witness-fetch` for a translation), then run
`licence PACKET SOURCE_FILE LICENCE_URL EXPLANATION` (add `--language LANG`
for a translation). The fetched source must contain that licence URL; the
record binds its digest and the precise edition. Set `licensed` separately for
each jurisdiction covered by that licence. The second reader must verify the
rightsholder, the precise version, applicable licence terms and attribution,
then record `licence-review PACKET ASSESSMENT` (with `--language LANG` for a
translation) under their existing second-reader identity. A conflicting or
unspecified version remains unresolved until checked. The licence evidence
checks do not replace source-page and semantic review.

For a new work, every original place needs each interface-language witness or
a documented search for that particular missing place. For a partial witness,
`witness-missing PACKET LANG LOC SEARCH.json` records the same exhaustive
search, edition and book locator required for a missing language; a later cut
of that location replaces the gap. Accepted older works may still add a
partial witness and retain their existing coverage debt.

An extension may add disjoint translated places to an already accepted language
from the same edition or a different edition. Record the actual edition, each
contributor, source and territorial rights for that extension's witness; each
extension is independently reviewed and signed. Planting assigns each place to
its own witness. An overlapping place is refused before acceptance; a different
edition does not inherit the original witness's rights or review.

If the work is already planted and no new original reading is needed, open a
witness-only extension from the parent packet:

```sh
K witness-extension-open . additional-ru
cd extensions/additional-ru
K witness-open . ru metadata.json
```

The path creates no reading or original acceptance. Use the normal witness
commands and the online independent `witness-gate --second --accept`; only
that exact accepted witness is planted. Its places must not overlap another
accepted witness of the same language, even if its edition differs.

An accepted witness is immutable. To recover a missing cached source or image
without changing it, use `K witness-restore-evidence . ru source.txt`: the
command restores only bytes matching the recorded SHA-256. For a browser capture
or other derivative, provide the exact saved bytes with `--file`; the parent
URL is not presented as the origin of those derivative bytes. An existing
different file is preserved for inspection.

### A new translation

Only when the original edition permits adaptation, a new translation may be
recorded as newly authored work. Open a separate witness whose edition says
`New translation`, attribution names the creator, and URL points to the
creator's exact licence declaration. Fetch that declaration, record `licensed`
and its law for each jurisdiction, and bind the declaration with `licence`.
Record the named translator with the normal person-source route, or use
`K witness-creator . es 'Translator Name'` to bind their identity to the exact
declaration itself. A different reader must verify the creator's authority;
the declaration alone does not establish it. Fetch and inspect the original
page as the witness maker, then:

```sh
K witness-new-translation . es 'LOC' --creator 'Translator Name' --text 'Una oración completa.' --image original-page.jpg --page '12' --alignment 'How these sentences correspond to the original.' --source-assessment 'Which original edition and each territorial basis permit creating this adaptation.' --image-assessment 'How this page image identifies the source edition and the original words.'
```

The kit records locally authored words as such, binds the exact original text,
edition and page, and does not claim that the translation was downloaded.
Source reuse, the translator's authority to license this new work, the licence,
and the linguistic alignment need independent assessment. A new translation
does not establish that no earlier translation exists.

### An unidentified translator

An attributed source can leave its individual translator unidentified without
establishing anonymous publication. For that distinct case, fetch the exact
source, document searches in two catalogues or publisher resources, and set
each territorial basis to a separately justified short quotation. Prepare:

```json
{"finding":"The exact publication and documented searches do not name its individual translator.","sources":["edition.pdf"],"searches":[{"url":"https://example.org/catalogue","outcome":"The catalogue does not identify a translator."},{"url":"https://example.org/publisher","outcome":"The publisher does not identify a translator."}]}
```

`K witness-unidentified-translator . es identity.json` records the exact
source hashes. It neither names a translator nor asserts public domain or
anonymous publication. If later identified, use `witness-unidentified-clear`
with a reason and record the actual person; both routes require renewed review.

### An anonymously published translation

An unnamed translator is not necessarily an anonymously published translation.
First fetch at least two sources with `witness-fetch` and research attribution
across at least three distinct catalogues or sources. Prepare a JSON object:

```json
{
  "first_publication_year": 1875,
  "lawful_publication": "Evidence for the first lawful public availability of this translation.",
  "identity_research": "Searches for disclosure of the translator's identity during the applicable period.",
  "same_version": "Evidence that the first publication is the translation in this exact edition.",
  "searches": [{"url": "https://example.org/catalogue", "outcome": "What was searched and what the catalogue established."}],
  "sources": ["edition.html", "catalogue.html"]
}
```

The example is a shape, not sufficient evidence: each explanation needs at
least 60 characters, three distinct searches and two fetched sources are
required. Record it with `witness-anonymous PACKET LANG evidence.json`, then
have the second reader verify the edition, publication and identity research
and run `witness-anonymous-review PACKET LANG ASSESSMENT` after their page
review. The review binds to the exact evidence record. The identified original
author and every identified editor still require their own sourced dates.

For the EU, Directive 2006/116/EC Article 1(3) and Article 8 permit the
anonymous-publication term only after the evidenced first lawful availability
and subject to identity disclosure; the dynamic gate uses year − 71. Russia is
not automatically classified public domain from this EU route: assess a
separate lawful quotation or licence, or establish the translator's identity.
If a translator is identified, use `witness-anonymous-clear PACKET LANG`,
record the person and obtain a new independent review.

To retire a superseded, unused transcription from an unaccepted witness, use
`K witness-source-remove . ru filename.txt --reason 'Why this source is no longer used.'`.
The recorded source and reason remain in the history, cached bytes are retained,
and the independent review is cleared. Any remaining reference blocks removal.

For context-sensitive Russian modernization, use `witness-correct` on the
exact page image, explaining the historic form and its modern spelling. The
original page transcription and correction remain recorded for the next reader;
the command does not silently rewrite the source.

For an erroneous source-backed participant with no QID, remove only its exact
role and English catalogue name with `K witness-person-remove-source . de author
'Pope, Alexander' --reason 'Why the participant record needs replacement.'`.
The entire source-backed record and reason are retained in correction history;
record the replacement identity and obtain a fresh independent review.

For a defective place in an unaccepted witness, use `K witness-place-withdraw
. ru 'LOC' --reason 'Why this edition cannot support this place as recorded.'`.
The exact place, prior review and reason are retained in history; the existing
source and page images remain. This is not a search finding: document the gap
through `witness-missing` or provide a verified replacement, then obtain a fresh
independent reading. Accepted witnesses need a separately reviewed correction.

For an undated translator already recorded without a QID, the exact AST publisher
card may expand an initialled name. Fetch the edition card as a witness source,
include its ISBN in `witness.edition`, then use:

```sh
K witness-person-publisher . ru 'Е. Парахневич' --full-name 'Парахневич Елена Валерьевна' --source ast.html --isbn '978-5-17-179033-2' --reason 'The publisher card for this exact edition expands the translator’s initials.'
```

This route records an identity, not dates or rights. The former participant is
retained in correction history; the independent reader must inspect the card.

When the only evidence for an existing provisional participant is a printed
page, first register its exact maker image with `witness-image`. Record only
the name and death year actually visible, with a transcription and assessment:

```sh
K witness-person-image . ru editor 'M. Epstein' --name 'М. А. Эпштейн' --died 1901 --image contributor.jpg --page 501 --wording 'М. А. Эпштейнъ († 1901)' --assessment 'The printed page names the contributor and gives the year of death.' --reason 'Replace the provisional record with the initials and year actually printed.'
```

The previous record remains in correction history. This does not establish an
unprinted full name, authorship beyond the stated contribution, or rights. The
second reader must fetch and assess the same page independently.

When printed italics carry meaning, preserve the exact words and their page
without adding markup to the transcription:

```sh
K witness-emphasis . en 'III p319b' --text 'real experience' --page 401 --reason 'The printed italics distinguish actual experience from imagined events.'
```

The words must occur exactly once and lie on the selected maker page. Each
emphasis record requires independent review. A mistaken record may be removed
with `witness-emphasis-withdraw`, which retains the former record and review.

### Original read directly from a page

When no faithful machine transcription of this edition is available, a maker
may record the exact original characters directly from a lawful image:

```sh
K transcribe-original-page . 'Dream p57' --url https://example.org/page.png --n 57 --reader MAKER_ID --text '…' --label-ru 'Сон, с. 57' --label-en 'Dream, p. 57' --why 'Which available transcriptions were searched, and why none gives this exact printing.' --punctuation 'Which punctuation is printed and which is editorial.'
```

`--file` may supply locally obtained image bytes from that URL. The text is
recorded as a human transcription, not a downloaded transcription; its maker,
source image, exact bytes and punctuation account are bound together. Run the
usual comparison and whole-page judgement, and obtain a second reading of the
same image. A page transcription establishes no rights or acceptance by itself.

When a printed quotation closes before an attribution in parentheses and the
sentence's punctuation follows that attribution, retain the quoted text as it
stands. After cutting it from the exact source and recording the page image:

```sh
K witness-sentence-boundary . es 'I p24b' --suffix ' (traducción del autor).' --reason 'The printed stop follows the translator attribution outside the closing quotation mark.'
```

The suffix is source context, not part of the translation. The command checks
that it immediately follows the quote in the fetched transcription and retains
the source and page hashes. A distinct page reader must assess the boundary.

### A lawful electronic edition without page images

Fetch the exact HTML chapter with `witness-fetch`. After personally opening that
same URL in the in-app Codex browser and saving a screenshot of the relevant
passage, record the capture with its actual time and reader:

```sh
K witness-browser-capture . zh-Hans chapter.html --file /absolute/path/to/maker.png --by maker --reader MAKER_ID --captured-at '2026-11-28T05:00:00+00:00'
```

The capture is a human observation bound to the fetched HTML and URL, not image
bytes downloaded from that URL. Use the resulting image in `witness-cut` with
an exact structural address. The reader must inspect the page, attribution,
translation and territorial rights; neither the screenshot nor its hash proves
them. A second reader independently opens and captures the same chapter.

For a born-digital original, fetch its exact HTML as a transcription and cut the
original place normally. After inspecting the page in the in-app browser, save
the screenshot and record the actual section and capture time:

```sh
K original-browser-capture . 'Discussion' article.html --file /absolute/path/to/maker.png --by maker --reader MAKER_ID --captured-at '2026-11-28T05:00:00+00:00' --n 1 --address-kind section --address-value 'Discussion'
K compare .
K judge . 'Discussion' whole transcription 'The visible section reads as this transcription.'
```

`--n` is only a capture index for the review workflow, not a printed page.
The exact HTML, structural address, image and reader are bound; existing page
review for the place is cleared. The independent reader must capture and read
the same section, and the usual source, rights and language duties still apply.

### Correcting a witness address or withdrawing an unplanted acceptance

When a single printed page's old label mixes a page and a line range, keep the
page separate from the lines. After inspecting the recorded maker image:

```sh
K witness-address-correct . en 'XXX.1 p953a' --section XXX.1 --page 953a --line-start 10 --line-end 14 --reason 'The inspected Bekker locator identifies lines on this single printed page.'
```

The complete predecessor and previous review are retained, including text
corrections; the page image is checked and the independent reading must be
renewed. A real page range still needs its separate page evidence.

If the existing independent reader finds a problem in an accepted witness
that has not been planted, `witness-withdraw . ru --reader READER_ID --finding
'Specific finding of at least forty characters.'` preserves the exact witness
and acceptance under `witness-history`, clears the review and reopens it. Add
`--replace` to preserve the old files and open a different edition instead.
The command refuses planted language records and changed acceptance bytes.
The replacement needs its own source, rights and independent review.
For an unaccepted, unplanted witness, its maker can instead use
`witness-retire . ru --maker MAKER_ID --reason 'Explain the need for a different edition in detail.'`.
This keeps the complete prior file and review under `retired-witnesses`;
the replacement is opened and reviewed separately. Both history routes
verify the saved predecessor's digest at the gate.

When the independent reader retrieves a differently rendered image of the
same page, retain it with `witness-image --by second` and use
`witness-page-identity` with the printed page, that image filename, the
reader's own archival reader URL, retrieval method and a detailed comparison
of the page and edition. The assessment is bound to both actual images, their
URLs and hashes, the page label and edition. It is a reader's judgment, not an
automatic inference from matching URLs or page numbers. The reader must still
review the passage and the territorial rights.

If a PDF render was incorrectly registered as a directly retrieved external
image, use `page-withdraw . LOC FILENAME --reason 'Explain the mistaken source and the needed correction.'`.
The recorded image, bytes, prior place and review remain in history. Register
the real PDF using `fetch` and `pdf-page`, then obtain a fresh page reading.
When its physical PDF page differs from the old index, give `--replacement-page N`.
The gate requires the replacement; the correction alone establishes no rights.
