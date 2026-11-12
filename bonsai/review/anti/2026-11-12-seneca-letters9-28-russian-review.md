# Seneca, letters 9 and 28: Russian journal witnesses

The first December 1884 issue of *Вѣра и Разумъ*, philosophy section
pp. 483–489 (PDF leaves 102–108), prints selected letter LII, explicitly
identified in its note as Seneca's letter 9. The first June issue, pp. 611–612
(PDF leaves 128–129), prints selected letter XXX and identifies it as letter
28. The scans, independently transcribed here, are:

- [December, first issue](https://azbyka.ru/otechnik/assets/uploads/vera-i-razum/%D0%92%D0%B5%D1%80%D0%B0%20%D0%B8%20%D1%80%D0%B0%D0%B7%D1%83%D0%BC.%201884.%20%D0%94%D0%B5%D0%BA%D0%B0%D0%B1%D1%80%D1%8C.%20%D0%9A%D0%BD.%201.pdf),
  SHA-256 `7f48fd3be4da4fb8d3711745a81855470a6212194385e42c70a3884e71925926`.
- [June, first issue](https://azbyka.ru/otechnik/assets/uploads/vera-i-razum/%D0%92%D0%B5%D1%80%D0%B0%20%D0%B8%20%D1%80%D0%B0%D0%B7%D1%83%D0%BC.%201884.%20%D0%98%D1%8E%D0%BD%D1%8C.%20%D0%9A%D0%BD.%201.pdf),
  SHA-256 `08c7755f70b92147d93ebd2d3dfb14beb8d7dce38ab06e43d82eb2fc8c7712cd`.

The journal does not name the translator. The anonymous 1884 printing, rather
than a later reprint or unverified attribution, is the source; the rights basis
is documented with the preceding letter 17 witness. Each passage was read
against its scanned pages and compared with the existing Latin and English
paragraphs. The 9.13 quotation includes the sentence immediately before the
paragraph break to preserve the stated proposition. The 9.19 translation
expands the list of goods after “all mine is with me”; that expansion belongs
to the printed Russian witness and has not been silently removed. Line wraps
and page breaks are joined, pre-reform spelling retained, and spaces around
punctuation normalized. Footnote marks are omitted. This review covers the six
quoted places, not a complete collation of either letter.

## Letter 1

The [first March issue](https://azbyka.ru/otechnik/assets/uploads/vera-i-razum/%D0%92%D0%B5%D1%80%D0%B0%20%D0%B8%20%D1%80%D0%B0%D0%B7%D1%83%D0%BC.%201884.%20%D0%9C%D0%B0%D1%80%D1%82.%20%D0%9A%D0%BD.%201.pdf),
philosophy section pp. 278–279 (PDF leaves 119–120), prints the text as
selected letter XIII and identifies it in a note as Seneca's letter 1. The
file SHA-256 is `e7f3cb682451cb4995f411a683b2edd271ef99283ccf2924023d53ef6d833303`.
The first three numbered Latin places were independently transcribed and
compared with the Latin/English passages. The translator expands or interprets
several phrases, including the final sentence of 1.2; the Russian printing's
own wording is retained rather than silently replacing it with another edition.
The quotation boundaries are assessed below against the corresponding Latin
and complete printed sentences.

## Reproducible boundary alignment

[`align-seneca-letters1-9-28.mjs`](align-seneca-letters1-9-28.mjs) runs
`bonsai/lib/align.mjs` against continuous Latin and independently transcribed
Russian windows, with neighbouring sentences on each side where available.
It saves the complete inputs, sentence spans and automatic candidates in
[`seneca-letters1-9-28-alignment.json`](seneca-letters1-9-28-alignment.json).
The exact printed source links and scan leaves are above; neighbour sentences
are context for this check, not additional stand quotations. The original
Latin comes from the existing Perseus witness. Sentence-unit runs retain
unmatched groups instead of hiding them.

The broader automatic windows for 1.1–3, 9.8 and 9.19 contain shifted or
unmatched sentence pairs and are not accepted as exact sentence alignment.
The focused 1.1 run and the sentence-unit `1.2-start`, `1.2-end`, `1.3-start`
and `1.3-end` runs identify the adjacent boundaries; their middle sentences
were compared with the print and Latin. For 9.8, `9.8-start` and `9.8-end`
identify the neighbouring statements, while the broader run has a shifted
pair; the corresponding whole sentences in between were inspected. For 9.19,
`9.19-start` and the sentence-unit `9.19-end` distinguish the adjacent Latin
and Russian statements; the translator's expansion and an omitted automatic
pair in the middle remain visible in the record. The 9.1, 9.10 and 9.13
whole-window candidates and the sentence-unit 28.1 run identify the required
boundaries. The 28.1 paragraph-level run joins its closing sentence with the
next section, so that pair alone must not be treated as a precise boundary.
These are bounded, inspected candidate alignments, not a claim of fully
automatic translation equivalence or complete-letter review.
