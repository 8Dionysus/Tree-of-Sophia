# AGENTS.md

This card applies to `bonsai/review/aesthetics/planting/`: the line that plants
the Aesthetics bonsai one work at a time.

## Role

A packet is one work, `packets/<work id>/`, made by one reader (the maker) and
read again by another (the second reader). `kit.py` is the line's one tool;
`drive.py` starts readers and is the operator's.

| File | Holds | Written by |
| --- | --- | --- |
| `places.json` | the edition read, its rights, the places: whole sentences of the original | maker, the texts through `kit.py cut` |
| `reading.json` | conceptions and links grown from the register readings; under `left` every reading not grown, with the reason | maker |
| `evidence.json` | fetched files, every place beside the scan, the judgment of every difference | `kit.py` only |
| `review.json` | the second reader's answer to every conception and link | second reader |
| `blocked.json` | `{ground, reason, tried}` in place of the rest, when the work cannot be planted by this card; `ground` is `rights`, `sources`, `reading` or `kit` | maker |
| `accepted.json` | who let the packet onto the map and on what basis | the line, never a reader |

`../plant.mjs` plants accepted packets into `bonsai/aesthetics/`.

## Laws

1. **Words come from files.** The text of a place is written by `kit.py cut`
   out of a transcription fetched by `kit.py fetch`, or, where no
   transcription holds the passage, out of the scan's own OCR. It is never typed,
   recalled, translated, modernised or mended by hand. Where the printed page
   reads otherwise, `kit.py correct` changes the text and records the change.
   In a print with the long s, ſ is s and ſs is the print's ß.
2. **The page decides.** `kit.py compare` sets every place beside the scan.
   Every difference is looked at on the page image, then judged with
   `kit.py judge`. A judgment given without looking is the one fault this line
   cannot repair afterwards.
3. **A place is a formula:** the whole sentences in which the work says the
   thing, and no more of them than say it.
4. **Nothing is planted that no register read.** Every conception and link
   names under `from` the register readings or links it grows from (the keys
   `kit.py task` prints). A reading that the work's words do not bear goes
   under `left` with the reason.
5. **A thesis affirms.** It says what the place says, in the place's own
   terms, in Russian and in English. It hedges, negates or contrasts only
   where the words of the place do.
6. **Rights are dates.** First wave: every hand in the text (author, editor,
   translator) died in 1942 or earlier, and the printing read is from 1929 or
   earlier. An author dead in 1746 is inside the wave; one dead in 1952 is
   outside it. The
   kit reads each year of death from the person's Wikidata item and writes the
   rights sentences itself. A work outside this is a `blocked.json`.
7. **Facts are fetched.** An edition, a year, a page number, an address is
   taken from a page opened in this session, never from memory.
8. **One reader, one packet.** Write inside your packet directory only. No git
   command, no other file of the checkout, no installation; the kit keeps its
   downloads outside the checkout.

## Done

The maker is done when `python3 ../../kit.py gate .` prints `gate: open`; the
second reader when `python3 ../../kit.py gate . --second` does. The gate
recomputes the comparison from the fetched files and checks rights, coverage
of the register readings, the affirming style and the planting itself; each
line it prints is one thing to settle.

An honest `blocked.json` is a finished packet. A packet pushed through the
gate by a judgment nobody looked at, a date nobody fetched or a reading bent
to fit is worse than none. Stop at `gate: open` or at `blocked.json`, nowhere
between.

A gate line that begins `line:` names a fault of the line itself: the checkout
or the corpus the planting reads. The packet stands as it is. Write no
`blocked.json`, change nothing, say the line in the last message and stop; the
operator mends the checkout and the packet goes on from where it stands.

[MAKER](MAKER.md) and [SECOND-READER](SECOND-READER.md) give each reader's
route.
