# Forensic Intake — Der Fall Wagner, Naumann 1888

Status: AI-assisted technical, bibliographic, and source-visible inspection; no
human textual or legal review

Inspected: 2026-07-30

Payload visibility: local only

## Source identity

- MDZ digital object: `bsb11827837`.
- URN: `urn:nbn:de:bvb:12-bsb11827837-1`.
- B3Kat record: `BV020338451`; OCLC: `217629023`.
- Holding copy: Bamberg, Staatsbibliothek, call number `30.1972`.
- Source-visible title page: *Der Fall Wagner. Ein Musikanten-Problem. Von
  Friedrich Nietzsche.* Leipzig, Verlag von C. G. Naumann, 1888.

The DNB/GND authority independently fixes the work identity, author, and 1888
publication date. MDZ's IIIF manifest, B3Kat MARC record, holding furniture, and
title page jointly identify the physical-copy surrogate.

The title page has no `Zweite Auflage` statement. Sommer's historical-critical
commentary reports one 1000-copy print run whose second half was designated
`Zweite Auflage`. ToS therefore records the exact unlabelled copy and the
issue-level risk rather than flattening both halves.

## Exact acquisition and fixity

- Official service: MDZ immediate PDF after acknowledgement of the object's
  NoC-NC rights statement.
- Provider-generated basename: `17854266678888bsb11827837.pdf`.
- Local basename: `bsb11827837.pdf`.
- Size: 31,518,299 bytes.
- SHA-256:
  `37fc9eb2d26886be936efe06c7fbeaf9f6dab231b3a81bc2cb5e824e98f984ed`.
- SHA-1: `6bf0ac120082468b7c22dabbf7748c6af4a51910`.
- MD5: `4327af405bdcaabf59d59f1e7a34b441`.
- Acquired and fixity-checked on 2026-07-30.

The PDF is held only in the canonical gitignored source tree. Its dynamic
generation URL is not treated as a durable identifier; the IIIF manifest, URN,
BSB/B3Kat records, holding identity, and local digest provide the durable chain.

## PDF and IIIF structure

Poppler 26.01.0 reports:

- PDF 1.4, 75 pages, A4 media box, unencrypted, untagged, and without
  JavaScript;
- 76 image rows: one MDZ logo, one soft mask, and 74 scan JPEGs;
- two cover-page fonts;
- no embedded book OCR.

The 221 extracted text bytes belong only to the MDZ-generated cover. PDF page 1
is that cover. PDF pages 2–75 map one-to-one and in order to the 74 IIIF
canvases. This offset is part of the item identity and must be respected by
future anchors.

## Source-visible review

The model inspected the front, title, opening, terminal, colophon, and rear
regions:

- IIIF canvas 1 is the red binding and canvas 2 carries the holding call number;
- canvas 5 is the 1888 historical title page;
- canvases 7–10 contain the preface;
- canvas 11 is the internal work title and canvas 13 begins printed page 1;
- canvases 61–69 cover the terminal printed sequence through page 57;
- canvas 69 carries the terminal paragraph and holding stamp;
- canvas 70 is the printer leaf; canvases 71–74 are rear furniture and binding.

Contact sheets remain machine-local evidence. This is a model review, not a
human transcription or content acceptance event.

## Scholarly production history

Sommer's official HAdW commentary records that Nietzsche sent the printing
manuscript to Naumann, continued adding material, corrected galley proofs, and
officially published the work on 22 September 1888. That evidence makes this
1888 witness materially stronger for author-proximate work than the later 1906
collected edition. It does not prove that every character in this scan equals a
critical text.

## Rights and publication boundary

The exact MDZ manifest reports No Copyright — Non-Commercial Use Only
(`NoC-NC 1.0`). MDZ documents immediate low-resolution PDF and IIIF routes and
requires holding, call-number, page/leaf, URN, and applicable digitization
credit in permitted reuse.

ToS admits local non-commercial research and tracked public-safe metadata only.
The operator-held payload is not a future-site upload. No page image, OCR,
transcription, translation, snippet, embedding, or content-bearing derivative
is authorized for server publication by this intake.

## Authority ceiling

This intake establishes an exact physical-copy surrogate, publication
identity, fixity, page topology, rights evidence, and an author-proximate
historical route. It accepts no German text, critical text, OCR, translation,
semantic claim, sign, graph relation, canon status, or payload publication.
