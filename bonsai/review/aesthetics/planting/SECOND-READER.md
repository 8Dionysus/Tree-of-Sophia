# The second reader's route

You read a packet another reader made. Your working directory is the packet.
The card `AGENTS.md` two levels up holds the laws. `K` stands for
`python3 ../../kit.py`. Record your review through the kit; its image and
witness commands also record the evidence of your independent reading.

Your question for every conception and every link is one: **do these sentences
say this?** The maker's notes and the registers' theses are no ground. The
place is.

## 1. Read

- `K task .` — what the registers wrote about this work.
- `places.json` — the places, in the original.
- `reading.json` — the conceptions and links.

For each conception read its `formula` places in the original, then its thesis
in Russian and in English:

- **yes** — the sentences say it, in both languages, and the concepts under
  `of` are what the sentences speak of;
- **partly** — the sentences say less, more or something beside it, the two
  languages part ways, or the thesis sets side by side two things the places
  say apart: the note names the words of the place and what differs;
- **no** — the sentences do not say it: the note quotes what they do say.

For each link read the places of its `grounds`: do they say this verb between
these two concepts?

For each entry under `left` that is not marked `disputed`: is its reason true
of the work? Look for the passage the register names (`K task .` gives locus
and thesis) in the transcription (`K find . "words" --in FILE`) and in the
scan (`K find . "words"`). `holds: true` — the work does not bear the reading,
as the reason says; `holds: false` — it does: the note names the place
(division, page, opening words).

Look also at the cut of each place: whole sentences, and enough of them to say
the thing. A cut falls short where a word of it (a pronoun, a «this») cannot
be understood without the sentence before; a connective alone (Denn, Also,
Thus) does not make it short. A cut that falls short is a `partly` on the
conception that cites it.

## 2. Look at the page

For at least a third of the places (three at least), and for every place whose
`evidence.json` method is `image` (a place marked `from_scan` there was cut
from the scan's own OCR and has no second text behind it: read it on the page
letter by letter and mark by mark):

```
K image . N --by second --loc "LOC"      # N as in the place's link: …/page/nN/…
```

Open the file with your image viewer and read the place on the page, word by
word and mark by mark, against `places.json`. Where the gate names a
correction that rests on the eye alone, read its line at the size of the scan
(`K image . N --by second --loc "LOC" --zoom 41-48`, the band of the page
height in percent) and say in `review.images` what the enlarged line reads. Choose places where a slip would
matter: a correction, many differences, the sentence a thesis leans on.

## 3. Prepare and import the review

Prepare this object outside the packet and record it with
`K import . review FILE`. Change packet files only through kit commands. An
image entry is a statement that you actually opened and read that page; merely
fetching an image or importing the object does not establish a reading.

```json
{"reader": {"id": "terra-2026-11-10-hume-standard-second", "model": "gpt-6.1-terra", "date": "2026-11-10",
            "reads": "Every place of the packet in the original, each thesis and verb beside it; the pages of six places on the scan."},
 "conceptions": {"<conception id>": {"says": "yes"},
                 "<conception id>": {"says": "partly", "note": "The place says «…»; the thesis adds that …"}},
 "links": {"<link id>": {"says": "yes"}},
 "left": {"<register key>": {"holds": true},
          "<register key>": {"holds": false, "note": "Essay I, p. 46 bears it: «The first source of amusement …»"}},
 "images": [{"loc": "II p16a", "n": 43, "reads": "as the packet"},
            {"loc": "III p19", "n": 46, "reads": "differs", "note": "The page has «exact resemblance;», the packet a comma."}],
 "accept": true,
 "summary": "Two sentences: what stands, what does not."}
```

`accept` is `true` only when every conception and link is a `yes`, every page
reads as the packet and every left reading is rightly left.

## 4. The gate

```
K gate . --second
```

Settle what it prints, until `gate: open`. A `partly` or a `no` is a finished
answer: the line returns it to the maker. Close with three lines: how many yes,
partly, no; which pages you read; the one thing the operator should know.

## Language witnesses

For a newly recorded CiNii Books or Cardiff Naturalists' Society participant,
inspect the named reference and the actual edition, then record the identity
assessment after the page review with `K witness-reference-review . LANG ROLE
'Name' --reader READER --assessment '…'`. Explain the evidence connecting the
person to this edition; a matching name alone does not establish that identity.

For a source attributed to an author collective, independently check the exact
article, all named authors, the attribution of the particular translation and
its rights. After reviewing the page, record `K witness-collective-review .
LANG LOC --reader READER --assessment '…'`. Do not infer which individuals
translated it from the article's general authorship list.

For text joined from page OCR, read every page and the catchword or omitted
material. After the ordinary page reading, record `K witness-ocr-join-review .
LANG LOC --reader READER --assessment '…'`; changing the joined source or the
place requires a new assessment.

For a new work, independently read every prepared language witness against its
original place and the same page image. Fetch each page with
`K witness-image . LANG URL --name FILE --by second`, then record the reading
with `K witness-review . LANG 'LOC' --reader ID --alignment '…'`. The witness
and packet second gates must open. The acceptance binds the exact reviewed
language files; a changed text, source, person, page or rights record requires a
new review. Search records for missing languages are evidence to inspect, not
proof of a translation that was never looked for.

## Bounded deaths

A person with `died_before` carries the claims or the reference the bound
rests on. Open the Wikidata item or the fetched reference and confirm that it
bounds the death before the recorded year; a bound you cannot confirm is a
`no` on the rights.

`died_before` is exclusive. For example, a Julian year whose last possible
Gregorian year is 1351 needs `died_before: 1352`. Older computed Wikidata
bounds without the explicit exclusive semantics and retained entity evidence
need correction and a fresh independent review; do not silently reinterpret
their existing acceptance.

For a Wikidata- or source-bounded person, open the exact evidence and record
its independent assessment after preparing the packet review:

```sh
K person-bound-review . author Q123 --reader READER_ID --assessment 'What the source says and why it supports the stated conservative upper bound.'
```

The assessment is bound to the exact person, source and bound.

## Reopened findings

For every numbered finding in `reopen-history.json`, read the source passage,
the maker's recorded response and the current packet. After importing the
complete review, record your independent assessment:

```sh
K reopen-review . 1 --reader READER_ID --resolution resolved --assessment 'What the source says, and whether the present reading addresses the finding.'
```

Use `--resolution resolved` only when the finding is actually resolved;
otherwise record `--resolution unresolved` and set `review.accept` to false
so the line returns the finding to the maker. The assessment binds to the exact maker response, reading,
places and review answers. The line may stamp model and effort without changing
those answers. A substantive change requires review again; a recorded assessment is not a
substitute for reading the source page.

For a source-bounded participant in a language witness, independently open the
reference and assess its bound after recording the witness page review:

```sh
K witness-bound-review . en author 'Name in reference' --reader READER_ID --assessment 'What the source says and how it supports this upper bound on death.'
```

The assessment binds the exact participant, source digest and bound.

For an identity established from the AST card for the exact witness edition,
inspect the publisher card and ISBN after reading the witness pages, then record:

```sh
K witness-publisher-review . ru 'Парахневич Елена Валерьевна' --reader READER_ID --assessment 'The publisher names this translator on the card with the same edition ISBN.'
```

This records no death year, rights determination or equivalence beyond the
identified edition. Any identity or source change invalidates the assessment.

For each original transcribed directly from a page, fetch and inspect the same
page as second reader, record it under `review.images`, then assess the exact
characters and any editorial punctuation:

```sh
K page . 'Dream p57' 57 https://example.org/page.png --by second
K original-page-review . 'Dream p57' --reader READER_ID --assessment 'The printed characters and the stated editorial punctuation were compared independently with this page.'
```

The assessment is tied to the exact source and current place. A differing
reading remains a negative review and cannot be accepted until corrected.

For a quotation whose terminal punctuation follows a parenthetical attribution,
inspect the exact source and page after the usual witness page review:

```sh
K witness-boundary-review . es 'I p24b' --reader READER_ID --assessment 'The page closes the quotation before the attribution and puts its terminal mark after that attribution.'
```

The attribution remains source context, not words of the quoted author; the
assessment is tied to the exact place and must be renewed after a change.

For a contributor identified from a printed page, independently fetch that
same image with `witness-image --by second`, inspect the wording and the role,
and after the ordinary witness page review record your assessment:

```sh
K witness-image-identity-review . ru 'М. А. Эпштейн' --reader READER_ID --assessment 'The same page names this contributor, describes her additions and gives the year of death as printed.'
```

The assessment is bound to the exact participant record and image; changing
either requires renewed review. Do not infer an unprinted full name or rights.

When a translation records printed emphasis, inspect the selected page and,
after the normal witness page review, assess the exact current place:

```sh
K witness-emphasis-review . en 'III p319b' --reader READER_ID --assessment 'The same printed page sets these exact words in italics and the emphasis is relevant to the contrast.'
```

The review is tied to the place and its emphasis; an edited place needs a new
assessment. Text remains text; the reader displays and copies the emphasis.

For an unidentified translator on an attributed short quotation, independently
inspect the exact edition and documented identity searches after the page
review, and record your assessment:

```sh
K witness-unidentified-review . es --reader READER_ID --assessment 'The exact publisher source and documented searches do not identify the individual translator.'
```

The assessment does not establish anonymity, public domain or rights: review
each territorial quotation basis, the page and the attribution separately.

For a new translation, inspect the original page independently, review the
translation and alignment through `witness-review`, and review the creator's
licence through `licence-review`. Then bind your linguistic and original-reuse
assessment to the exact place:

```sh
K witness-new-translation-review . es 'LOC' --reader READER_ID --assessment 'The exact original page and new translation correspond; the original edition permits this adaptation.'
```

Assess the creator's authority, licence conditions and any third-party material
separately. If any of them remains uncertain, leave the witness unaccepted.

For a browser-captured electronic edition, independently open the same URL in
the in-app Codex browser and save your own screenshot of the passage. Record its
actual time and your reader ID, then perform the ordinary page and alignment
review:

```sh
K witness-browser-capture . zh-Hans chapter.html --file /absolute/path/to/second.png --by second --reader READER_ID --captured-at '2026-11-28T05:30:00+00:00'
K witness-review . zh-Hans 'LOC' --reader READER_ID --model MODEL --alignment 'How the translation corresponds to the original.'
```

The command independently fetches the HTML and refuses a changed source hash;
your capture may differ in pixels from the maker's. Inspecting the actual page
and assessing its authority and rights remain separate duties.

For a born-digital original, independently open the same HTML section and
record your own capture under the maker's capture index and exact address:

```sh
K original-browser-capture . 'Discussion' article.html --file /absolute/path/to/second.png --by second --reader READER_ID --captured-at '2026-11-28T05:30:00+00:00' --n 1 --address-kind section --address-value 'Discussion'
```

Record the independent reading in `review.images` as usual; `n` identifies
the capture and does not assert a printed page. The HTML digest, address and
reader are checked separately from the screenshot bytes and page judgement.
