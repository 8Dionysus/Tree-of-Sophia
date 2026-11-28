# AGENTS.md

This card governs `bonsai/review/aesthetics/planting/`.

## Role

A packet is one work, `packets/<work id>/`, read by a maker and an independent
second reader. `kit.py` is the line's tool; `drive.py` starts readers.

| File | Holds | Written by |
| --- | --- | --- |
| `places.json` | edition, rights and whole-sentence original places | maker, text through `kit.py cut` |
| `reading.json` | register-based conceptions and links; `left` records omissions and reasons | maker |
| `evidence.json` | fetched files, scans and judgments of differences | `kit.py` only |
| `review.json` | independent answers to conceptions and links | second reader |
| `blocked.json` | `{ground, reason, tried}` when planting fails; ground: `rights`, `sources`, `reading` or `kit` | maker |
| `witnesses/<language>.json` | independently sourced translation and page review | `kit.py` only |
| `accepted.json` | who accepted the packet and on what basis | line, never reader |

`../plant.mjs` plants accepted packets into `bonsai/aesthetics/`.

## Laws

1. **Words come from files.** `kit.py cut` uses a fetched transcription or the
   scan's OCR. Never recall, modernise or mend words by hand; use `kit.py correct`
   for page-supported differences. Without a faithful transcription,
   `witness-transcribe-page` and `transcribe-original-page` record human
   readings and independent checks of the exact image; original transcription
   also records the reader and punctuation. These are not downloads or licences.
   `witness-new-translation` requires a sourced creator, licence, exact original
   and page, and independent linguistic and adaptation-rights review; identify
   it as new work. See [MAKER](MAKER.md) for these routes.
   In a print with the long s, ſ is s and ſs is the print's ß.
2. **The page decides.** `kit.py compare` sets every place beside the scan.
   Every difference is looked at on the page image, then judged with
   `kit.py judge`. A judgment without looking cannot be repaired afterwards.
   Where OCR does not support a changed letter, maker and second reader inspect
   the line at scan size (`kit.py image … --zoom`): on a whole page *possimus*
   and *possumus* look alike.
3. **A place is a formula:** the whole sentences in which the work says the
   thing, and no more of them than say it.
4. **Nothing is planted that no register read.** Every conception and link
   names under `from` the register readings or links it grows from (the keys
   `kit.py task` prints). A reading that the work's words do not bear goes
   under `left` with the reason.
5. **A thesis affirms.** It says what the place says, in the place's own
   terms, in Russian and in English. It hedges, negates or contrasts only
   where the words of the place do.
6. **Rights belong to each jurisdiction.** Record a basis and its legal
   statement for Russia, the USA and the EU. Public-domain dates use the
   running year and every hand in the text; unknown wartime status gets the
   longer Russian term. A protected text may be a short quotation from a
   lawful copy, with the source page and an independent second reading. A
   locator needs the edition and the searches performed. Unknown is not a
   finding of absence.
   A born-digital source may use its exact URL and structural address with
   separately captured and reviewed browser views; its capture index is not a
   printed page number.
7. **Facts are fetched.** An edition, a year, a page number, an address is
   taken from a page opened in this session, never from memory.
8. **One reader, one packet.** Write inside your packet directory only. No git
   command, no other file of the checkout, no installation; the kit keeps its
   downloads outside the checkout.

## Done

The maker finishes when `python3 ../../kit.py gate .` prints `gate: open`; the
second reader uses the same command with `--second`. The gate recomputes source
comparison and checks rights, register coverage, affirming style and planting.

An honest `blocked.json` is a finished packet. Never invent a page judgment,
date or reading. Stop at `gate: open` or `blocked.json`.

A `line:` gate finding names a checkout or corpus fault. Leave the packet
unchanged, write no `blocked.json`, report the line and stop for operator repair.

[MAKER](MAKER.md) and [SECOND-READER](SECOND-READER.md) give each reader's
route.
