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

Then fill `entry` in `places.json`: `original` (language code) and one witness
under that code:

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
edition, beside the registers' readings).

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

The reader opens each fetched image and checks the whole translated sentences
against it and against the original; recording a review is not itself a page
reading. A quotation also records `--lines`, and an exceptional longer place
records `--justification` on `witness-cut`. The acceptance binds the exact file
by SHA-256. Changes require a new independent reading and acceptance. Local
source files and images stay in the kit cache outside Git. A partial witness
leaves the other places without a translation.

A prepared JSON object can be brought into a new, unaccepted packet with
`K import . places FILE` or `K import . reading FILE`; the second reader can
use `K import . review FILE`. `K import . blocked FILE` records a block and
`K unblock .` clears it for another attempt. Import does not validate or
accept the packet. The gate still checks the source files, page evidence and
reading. Accepted packets cannot be replaced by these commands.

## Continue an accepted reading

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

Use `witness-info` with a JSON object to update edition, URL, maker or
normalization. `witness-rights . ru russia short-quotation 'legal basis …'`
sets one jurisdiction at a time. For a person without a usable Wikidata item,
`witness-person-source . ru translator 'Name' https://… --born YEAR --died YEAR`
fetches a trusted reference and records its digest. A missing death year stays
unknown and cannot establish public domain by dates. Every metadata change
requires a fresh independent reading before acceptance.

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
