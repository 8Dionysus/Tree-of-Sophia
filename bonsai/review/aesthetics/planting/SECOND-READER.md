# The second reader's route

You read a packet another reader made. Your working directory is the packet.
The card `AGENTS.md` two levels up holds the laws. `K` stands for
`python3 ../../kit.py`. You write one file, `review.json`, and change no other.

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

## 3. Write review.json

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

For a new work, independently read every prepared language witness against its
original place and the same page image. Fetch each page with
`K witness-image . LANG URL --name FILE --by second`, then record the reading
with `K witness-review . LANG 'LOC' --reader ID --alignment '…'`. The witness
and packet second gates must open. The acceptance binds the exact reviewed
language files; a changed text, source, person, page or rights record requires a
new review. Search records for missing languages are evidence to inspect, not
proof of a translation that was never looked for.
